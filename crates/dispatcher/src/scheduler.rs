use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tracing::{debug, error, info, warn};

use scheduler_core::{
    models::{Message, Task, TaskExecutionMessage, TaskRun, TaskRunStatus},
    traits::{MessageQueue, TaskRepository, TaskRunRepository, TaskSchedulerService},
    SchedulerError, SchedulerResult,
};
use scheduler_infrastructure::{MetricsCollector, StructuredLogger, TaskTracer};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::cron_utils::CronScheduler;
use crate::dependency_checker::{DependencyCheckService, DependencyChecker};

/// 任务调度器实现
pub struct TaskScheduler {
    pub task_repo: Arc<dyn TaskRepository>,
    pub task_run_repo: Arc<dyn TaskRunRepository>,
    pub message_queue: Arc<dyn MessageQueue>,
    pub task_queue_name: String,
    pub dependency_checker: DependencyChecker,
    pub metrics: Arc<MetricsCollector>,
}

impl TaskScheduler {
    /// 创建新的任务调度器
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
        message_queue: Arc<dyn MessageQueue>,
        task_queue_name: String,
        metrics: Arc<MetricsCollector>,
    ) -> Self {
        let dependency_checker = DependencyChecker::new(task_repo.clone(), task_run_repo.clone());

        Self {
            task_repo,
            task_run_repo,
            message_queue,
            task_queue_name,
            dependency_checker,
            metrics,
        }
    }

    /// 检查任务是否应该被调度
    pub async fn should_schedule_task(&self, task: &Task) -> SchedulerResult<bool> {
        // 检查任务是否处于活跃状态
        if !task.is_active() {
            debug!("任务 {} 不处于活跃状态，跳过调度", task.name);
            return Ok(false);
        }

        // 验证CRON表达式
        let cron_scheduler = CronScheduler::new(&task.schedule)?;

        // 获取任务的最近执行记录
        let recent_runs = self.task_run_repo.get_recent_runs(task.id, 1).await?;
        let last_run_time = recent_runs.first().map(|run| run.scheduled_at);

        // 检查是否到达调度时间
        let now = Utc::now();
        let should_trigger = cron_scheduler.should_trigger(last_run_time, now);

        if should_trigger {
            debug!("任务 {} 到达调度时间", task.name);

            // 检查任务是否过期（可选的警告）
            if cron_scheduler.is_task_overdue(last_run_time, now, 5) {
                warn!("任务 {} 可能已过期，预期执行时间已过去超过5分钟", task.name);
            }

            // 记录频率信息用于调试
            debug!(
                "任务 {} 的执行频率: {}",
                task.name,
                cron_scheduler.get_frequency_description()
            );
        } else {
            // 记录下次执行时间
            if let Some(time_until) = cron_scheduler.time_until_next_execution(now) {
                debug!(
                    "任务 {} 下次执行还需等待: {}分钟",
                    task.name,
                    time_until.num_minutes()
                );
            }
        }

        Ok(should_trigger)
    }

    /// 检查任务是否有正在运行的实例
    async fn has_running_instance(&self, task_id: i64) -> SchedulerResult<bool> {
        let running_runs = self.task_run_repo.get_by_task_id(task_id).await?;
        let has_running = running_runs.iter().any(|run| run.is_running());

        if has_running {
            debug!("任务 {} 有正在运行的实例，跳过调度", task_id);
        }

        Ok(has_running)
    }

    /// 创建任务执行消息
    fn create_task_execution_message(
        &self,
        task: &Task,
        task_run: &TaskRun,
    ) -> TaskExecutionMessage {
        TaskExecutionMessage {
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

#[async_trait]
impl TaskSchedulerService for TaskScheduler {
    /// 扫描并调度任务
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>> {
        let span = tracing::info_span!("scan_and_schedule");
        let _guard = span.enter();
        let start_time = std::time::Instant::now();
        info!("开始扫描需要调度的任务");

        // 获取所有活跃任务
        let active_tasks = self.task_repo.get_active_tasks().await?;
        let mut scheduled_runs = Vec::new();

        for task in active_tasks {
            let task_span = TaskTracer::schedule_task_span(task.id, &task.name, &task.task_type);
            let _task_guard = task_span.enter();

            match self.schedule_task_if_needed(&task).await {
                Ok(Some(task_run)) => {
                    StructuredLogger::log_task_scheduled(
                        task.id,
                        &task.name,
                        &task.task_type,
                        task_run.scheduled_at,
                    );
                    scheduled_runs.push(task_run);
                }
                Ok(None) => {
                    // 任务不需要调度，这是正常情况
                }
                Err(e) => {
                    StructuredLogger::log_system_error("dispatcher", "schedule_task", &e);
                    TaskTracer::record_error(&e);
                    // 继续处理其他任务，不因为单个任务失败而停止整个调度过程
                }
            }
        }

        // Record scheduling metrics
        let duration = start_time.elapsed().as_secs_f64();
        self.metrics.record_scheduling_duration(duration);

        info!("本次调度完成，共调度了 {} 个任务", scheduled_runs.len());
        Ok(scheduled_runs)
    }

    /// 检查任务依赖
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool> {
        let span = TaskTracer::dependency_check_span(task.id, &task.name);
        let _guard = span.enter();
        let check_result = self.dependency_checker.check_dependencies(task).await?;

        StructuredLogger::log_dependency_check(
            task.id,
            &task.name,
            check_result.can_execute,
            check_result.reason.as_deref(),
        );

        if !check_result.can_execute {
            if let Some(reason) = &check_result.reason {
                tracing::Span::current().set_attribute("dependency.check.result", "failed");
                tracing::Span::current().set_attribute("dependency.check.reason", reason.clone());
            }
        } else {
            tracing::Span::current().set_attribute("dependency.check.result", "passed");
        }

        Ok(check_result.can_execute)
    }

    /// 创建任务运行实例
    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun> {
        let now = Utc::now();
        let mut task_run = TaskRun::new(task.id, now);

        // 如果任务配置了分片，设置分片信息
        if let Some(shard_config) = &task.shard_config {
            if shard_config.enabled {
                // 这里简化处理，实际实现中可能需要更复杂的分片逻辑
                task_run.shard_index = Some(0);
                task_run.shard_total = Some(shard_config.shard_count);
            }
        }

        // 保存到数据库
        let created_run = self.task_run_repo.create(&task_run).await?;

        info!("为任务 {} 创建了新的执行实例 {}", task.name, created_run.id);

        Ok(created_run)
    }

    /// 分发任务到消息队列
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        let span = tracing::info_span!("dispatch_to_queue", task_run.id = task_run.id);
        let _guard = span.enter();
        let _start_time = std::time::Instant::now();

        // 获取任务信息
        let task = self
            .task_repo
            .get_by_id(task_run.task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound {
                id: task_run.task_id,
            })?;

        // 创建任务执行消息
        let task_execution = self.create_task_execution_message(&task, task_run);
        let message = Message::task_execution(task_execution);

        // 发送到消息队列
        let mq_start = std::time::Instant::now();
        {
            let mq_span = TaskTracer::message_queue_span("publish", &self.task_queue_name);
            let _mq_guard = mq_span.enter();
            self.message_queue
                .publish_message(&self.task_queue_name, &message)
                .await?;
        }

        // Record message queue operation metrics
        let mq_duration = mq_start.elapsed().as_secs_f64();
        self.metrics
            .record_message_queue_operation("publish", mq_duration);

        // 更新任务运行状态为已分发
        let db_start = std::time::Instant::now();
        {
            let db_span = TaskTracer::database_span("update", "task_runs");
            let _db_guard = db_span.enter();
            self.task_run_repo
                .update_status(task_run.id, TaskRunStatus::Dispatched, None)
                .await?;
        }

        // Record database operation metrics
        let db_duration = db_start.elapsed().as_secs_f64();
        self.metrics
            .record_database_operation("update_task_run_status", db_duration);

        info!("任务运行实例 {} 已分发到消息队列", task_run.id);

        Ok(())
    }
}

impl TaskScheduler {
    /// 如果需要的话调度任务
    async fn schedule_task_if_needed(&self, task: &Task) -> SchedulerResult<Option<TaskRun>> {
        // 检查是否应该调度
        if !self.should_schedule_task(task).await? {
            return Ok(None);
        }

        // 检查是否有正在运行的实例
        if self.has_running_instance(task.id).await? {
            return Ok(None);
        }

        // 检查依赖关系
        if !self.check_dependencies(task).await? {
            return Ok(None);
        }

        // 创建任务运行实例
        let task_run = self.create_task_run(task).await?;

        // 分发到消息队列
        self.dispatch_to_queue(&task_run).await?;

        Ok(Some(task_run))
    }

    /// 检测并处理过期任务
    pub async fn detect_overdue_tasks(
        &self,
        grace_period_minutes: i64,
    ) -> SchedulerResult<Vec<Task>> {
        info!("开始检测过期任务，宽限期: {}分钟", grace_period_minutes);

        let active_tasks = self.task_repo.get_active_tasks().await?;
        let mut overdue_tasks = Vec::new();
        let now = Utc::now();

        for task in active_tasks {
            // 验证CRON表达式
            if let Ok(cron_scheduler) = CronScheduler::new(&task.schedule) {
                // 获取最近的执行记录
                let recent_runs = self.task_run_repo.get_recent_runs(task.id, 1).await?;
                let last_run_time = recent_runs.first().map(|run| run.scheduled_at);

                // 检查是否过期
                if cron_scheduler.is_task_overdue(last_run_time, now, grace_period_minutes) {
                    warn!(
                        "检测到过期任务: {} (ID: {}), 上次执行: {:?}",
                        task.name,
                        task.id,
                        last_run_time.map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    );
                    overdue_tasks.push(task);
                }
            } else {
                warn!("任务 {} 的CRON表达式无效: {}", task.name, task.schedule);
            }
        }

        info!("检测完成，发现 {} 个过期任务", overdue_tasks.len());
        Ok(overdue_tasks)
    }

    /// 获取任务的下次执行时间
    pub async fn get_next_execution_time(
        &self,
        task_id: i64,
    ) -> SchedulerResult<Option<DateTime<Utc>>> {
        let task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;

        let cron_scheduler = CronScheduler::new(&task.schedule)?;
        let now = Utc::now();

        Ok(cron_scheduler.next_execution_time(now))
    }

    /// 验证任务的CRON表达式
    pub async fn validate_task_schedule(&self, task_id: i64) -> SchedulerResult<bool> {
        let task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;

        match CronScheduler::validate_cron_expression(&task.schedule) {
            Ok(_) => {
                debug!("任务 {} 的CRON表达式有效: {}", task.name, task.schedule);
                Ok(true)
            }
            Err(e) => {
                error!(
                    "任务 {} 的CRON表达式无效: {} - {}",
                    task.name, task.schedule, e
                );
                Ok(false)
            }
        }
    }
}
