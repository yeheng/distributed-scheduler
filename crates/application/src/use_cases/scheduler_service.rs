use chrono::Utc;
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::ports::service_interfaces::{task_services::TaskSchedulerService, SchedulerStats};
use scheduler_domain::entities::{Message, Task, TaskRun};
use scheduler_domain::ports::messaging::MessageQueue;
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};
use scheduler_errors::{SchedulerError, SchedulerResult};

use crate::use_cases::cron_utils::CronScheduler;
use crate::use_cases::dependency_checker::{DependencyCheckService, DependencyCheckServiceTrait};

pub struct SchedulerService {
    pub task_repo: Arc<dyn TaskRepository>,
    pub task_run_repo: Arc<dyn TaskRunRepository>,
    pub message_queue: Arc<dyn MessageQueue>,
    pub task_queue_name: String,
    pub dependency_checker: DependencyCheckService,
    // pub metrics: Arc<MetricsCollector>, // TODO: 添加metrics支持
    pub is_running: std::sync::atomic::AtomicBool,
}

impl SchedulerService {
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
        message_queue: Arc<dyn MessageQueue>,
        task_queue_name: String,
    ) -> Self {
        let dependency_checker =
            DependencyCheckService::new(task_repo.clone(), task_run_repo.clone());

        Self {
            task_repo,
            task_run_repo,
            message_queue,
            task_queue_name,
            dependency_checker,
            is_running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    async fn should_schedule_task(&self, task: &Task) -> SchedulerResult<bool> {
        if !task.is_active() {
            debug!("任务 {} 不处于活跃状态，跳过调度", task.name);
            return Ok(false);
        }

        let cron_scheduler = CronScheduler::new(&task.schedule)?;
        let now = Utc::now();

        if !cron_scheduler.should_run(now) {
            return Ok(false);
        }

        // 检查并发控制限制
        if !self.check_concurrent_limit(task).await? {
            debug!(
                "任务 {} 已达到最大并发限制，跳过本次调度",
                task.name
            );
            return Ok(false);
        }

        Ok(true)
    }

    async fn schedule_task_if_needed(&self, task: &Task) -> SchedulerResult<Option<TaskRun>> {
        if !self.should_schedule_task(task).await? {
            return Ok(None);
        }

        if !self.check_dependencies(task).await? {
            return Ok(None);
        }

        let task_run = self.create_task_run(task).await?;
        self.dispatch_to_queue(&task_run).await?;

        Ok(Some(task_run))
    }

    async fn check_concurrent_limit(&self, task: &Task) -> SchedulerResult<bool> {
        // 获取正在运行的任务实例数量
        let running_runs = self.task_run_repo.get_running_runs().await?;
        let running_count = running_runs
            .iter()
            .filter(|run| run.task_id == task.id)
            .count() as i32;

        // 使用默认并发限制（未来可以添加到Task结构体中）
        let max_concurrent_runs = task.max_retries.max(1); // 使用max_retries作为并发限制的代理
        
        debug!(
            "任务 {} 并发检查: 运行中 {}/{}",
            task.name, running_count, max_concurrent_runs
        );

        Ok(running_count < max_concurrent_runs)
    }

    fn create_task_execution_message(
        &self,
        task: &Task,
        task_run: &TaskRun,
    ) -> scheduler_domain::entities::TaskExecutionMessage {
        scheduler_domain::entities::TaskExecutionMessage {
            task_run_id: task_run.id,
            task_id: task.id,
            task_name: task.name.clone(),
            task_type: task.task_type.clone(),
            parameters: task.parameters.clone(),
            timeout_seconds: task.timeout_seconds,
            retry_count: task_run.retry_count,
            shard_index: task_run.shard_index,
            shard_total: task_run.shard_total,
        }
    }
}

#[async_trait::async_trait]
impl TaskSchedulerService for SchedulerService {
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>> {
        let span = tracing::info_span!("scan_and_schedule");
        let _guard = span.enter();
        let start_time = std::time::Instant::now();
        info!("开始扫描需要调度的任务");

        let active_tasks = self.task_repo.get_active_tasks().await?;
        let task_count = active_tasks.len();

        // 配置并发参数
        let concurrency_limit = 10; // 限制并发数，避免资源耗尽
        info!(
            "发现 {} 个活跃任务，开始并发调度（并发限制: {}）",
            task_count, concurrency_limit
        );

        // 使用 futures::stream 进行并发处理
        let scheduled_runs: Vec<TaskRun> = stream::iter(active_tasks)
            .map(|task| {
                let scheduler_service = self;
                async move {
                    match scheduler_service.schedule_task_if_needed(&task).await {
                        Ok(Some(task_run)) => {
                            info!("成功调度任务: {} -> 运行实例: {}", task.name, task_run.id);
                            Some(task_run)
                        }
                        Ok(None) => {
                            debug!("任务 {} 不需要调度", task.name);
                            None
                        }
                        Err(e) => {
                            error!("调度任务 {} 失败: {}", task.name, e);
                            None
                        }
                    }
                }
            })
            .buffer_unordered(concurrency_limit) // 控制并发数
            .filter_map(|result| async { result }) // 过滤掉 None 结果
            .collect()
            .await;

        let duration = start_time.elapsed().as_secs_f64();
        let success_count = scheduled_runs.len();

        info!(
            "本次调度完成，共调度了 {}/{} 个任务，耗时 {:.3} 秒",
            success_count, task_count, duration
        );

        Ok(scheduled_runs)
    }

    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool> {
        let span = tracing::info_span!("check_dependencies");
        let _guard = span.enter();
        let check_result = self.dependency_checker.check_dependencies(task).await?;

        debug!(
            "依赖检查结果: can_execute={}, reason={:?}",
            check_result.can_execute, check_result.reason
        );

        if !check_result.can_execute {
            debug!("依赖检查失败: {:?}", check_result.reason);
        } else {
            debug!("依赖检查通过");
        }

        Ok(check_result.can_execute)
    }

    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun> {
        let now = Utc::now();
        let mut task_run = TaskRun::new(task.id, now);

        if let Some(shard_config) = &task.shard_config {
            if shard_config.enabled {
                task_run.shard_index = Some(0);
                task_run.shard_total = Some(shard_config.shard_count);
            }
        }

        let created_run = self.task_run_repo.create(&task_run).await?;

        info!("为任务 {} 创建了新的执行实例 {}", task.name, created_run.id);

        Ok(created_run)
    }

    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        let span = tracing::info_span!("dispatch_to_queue", task_run.id = task_run.id);
        let _guard = span.enter();
        let _start_time = std::time::Instant::now();

        let task = self
            .task_repo
            .get_by_id(task_run.task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound {
                id: task_run.task_id,
            })?;

        let task_execution = self.create_task_execution_message(&task, task_run);
        let message = Message::task_execution(task_execution);

        let mq_start = std::time::Instant::now();
        self.message_queue
            .publish_message(&self.task_queue_name, &message)
            .await?;
        let mq_duration = mq_start.elapsed().as_secs_f64();

        info!(
            "任务 {} 已分发到队列 {}, 耗时: {:.3}s",
            task.name, self.task_queue_name, mq_duration
        );

        Ok(())
    }

    async fn start(&self) -> SchedulerResult<()> {
        info!("启动调度器服务");
        self.is_running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> SchedulerResult<()> {
        info!("停止调度器服务");
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn schedule_task(&self, task: &Task) -> SchedulerResult<()> {
        if let Some(_task_run) = self.schedule_task_if_needed(task).await? {
            info!("任务 {} 已调度", task.name);
        }
        Ok(())
    }

    async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()> {
        for task in tasks {
            if let Err(e) = self.schedule_task(task).await {
                error!("调度任务 {} 失败: {}", task.name, e);
            }
        }
        Ok(())
    }

    async fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn get_stats(&self) -> SchedulerResult<SchedulerStats> {
        // 获取所有任务数量（使用现有的API）
        let active_tasks = self.task_repo.get_active_tasks().await?;
        let total_tasks = active_tasks.len() as i64;
        let active_tasks_count = active_tasks.len() as i64;

        // 获取任务运行统计
        let running_runs = self.task_run_repo.get_running_runs().await?;
        let running_task_runs = running_runs.len() as i64;

        let pending_runs = self.task_run_repo.get_pending_runs(None).await?;
        let pending_task_runs = pending_runs.len() as i64;

        // 计算运行时间（使用运行状态作为代理）
        let uptime_seconds = if self.is_running().await {
            // 简单计算，实际中可以存储启动时间
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        } else {
            0
        };

        Ok(SchedulerStats {
            total_tasks,
            active_tasks: active_tasks_count,
            running_task_runs,
            pending_task_runs,
            uptime_seconds,
            last_schedule_time: Some(Utc::now()), // 可以存储实际的最后调度时间
        })
    }

    async fn reload_config(&self) -> SchedulerResult<()> {
        info!("重新加载调度器配置");
        
        // 从配置文件重新加载配置
        match scheduler_config::AppConfig::load(None) {
            Ok(new_config) => {
                info!("配置重新加载成功");
                
                // 检查并更新相关配置
                // TODO: 实际项目中可以将配置传递给各个组件
                debug!(
                    "新配置加载: 调度间隔={}s, 最大并发={}",
                    new_config.dispatcher.schedule_interval_seconds,
                    new_config.dispatcher.max_concurrent_dispatches
                );
                
                Ok(())
            }
            Err(e) => {
                error!("配置重新加载失败: {}", e);
                Err(SchedulerError::Configuration(
                    format!("配置重新加载失败: {}", e)
                ))
            }
        }
    }
}
