use std::sync::Arc;
use chrono::Utc;
use tracing::{debug, error, info};

use crate::interfaces::service_interfaces::{task_services::TaskSchedulerService, SchedulerStats};
use scheduler_core::{traits::MessageQueue, SchedulerError, SchedulerResult};
use scheduler_domain::entities::{Message, Task, TaskRun};
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};

use crate::services::dependency_checker::{DependencyCheckService, DependencyCheckServiceTrait};
use crate::services::cron_utils::CronScheduler;

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
        let dependency_checker = DependencyCheckService::new(task_repo.clone(), task_run_repo.clone());

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

        // TODO: 添加并发控制检查，当前Task结构体没有max_concurrent_runs字段

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

    fn create_task_execution_message(&self, task: &Task, task_run: &TaskRun) -> scheduler_domain::entities::TaskExecutionMessage {
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
        let mut scheduled_runs = Vec::new();

        for task in active_tasks {
            // TODO: 添加tracing支持

            match self.schedule_task_if_needed(&task).await {
                Ok(Some(task_run)) => {
                    // TODO: 添加logging支持
                    scheduled_runs.push(task_run);
                }
                Ok(None) => {}
                Err(e) => {
                    error!("调度任务 {} 失败: {}", task.name, e);
                }
            }
        }
        
        let _duration = start_time.elapsed().as_secs_f64();
        // TODO: 添加metrics记录

        info!("本次调度完成，共调度了 {} 个任务", scheduled_runs.len());
        Ok(scheduled_runs)
    }

    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool> {
        let span = tracing::info_span!("check_dependencies");
        let _guard = span.enter();
        let check_result = self.dependency_checker.check_dependencies(task).await?;

        debug!("依赖检查结果: can_execute={}, reason={:?}", check_result.can_execute, check_result.reason);

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
        
        info!("任务 {} 已分发到队列 {}, 耗时: {:.3}s", task.name, self.task_queue_name, mq_duration);

        Ok(())
    }

    async fn start(&self) -> SchedulerResult<()> {
        info!("启动调度器服务");
        self.is_running.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> SchedulerResult<()> {
        info!("停止调度器服务");
        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);
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
        // TODO: 实现统计信息收集，当前Repository trait缺少一些必要的方法
        Ok(SchedulerStats {
            total_tasks: 0,
            active_tasks: 0,
            running_task_runs: 0,
            pending_task_runs: 0,
            uptime_seconds: 0,
            last_schedule_time: None,
        })
    }

    async fn reload_config(&self) -> SchedulerResult<()> {
        info!("重新加载调度器配置");
        // TODO: Implement configuration reloading
        Ok(())
    }
}
