use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tracing::{debug, error, info, warn};

use scheduler_core::{SchedulerError, SchedulerResult, traits::{MessageQueue, scheduler::TaskControlService}};
use scheduler_domain::entities::{Message, TaskRun, TaskRunStatus, TaskStatus, TaskControlAction, TaskControlMessage};
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};

#[derive(Debug, Default, Clone)]
pub struct TaskStatusSummary {
    pub pending: usize,
    pub dispatched: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

impl TaskStatusSummary {
    pub fn total(&self) -> usize {
        self.pending
            + self.dispatched
            + self.running
            + self.completed
            + self.failed
            + self.cancelled
    }
    pub fn active(&self) -> usize {
        self.pending + self.dispatched + self.running
    }
    pub fn finished(&self) -> usize {
        self.completed + self.failed + self.cancelled
    }
}

pub struct TaskController {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    message_queue: Arc<dyn MessageQueue>,
    control_queue_name: String,
}

impl TaskController {
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
        message_queue: Arc<dyn MessageQueue>,
        control_queue_name: String,
    ) -> Self {
        Self {
            task_repo,
            task_run_repo,
            message_queue,
            control_queue_name,
        }
    }
    async fn send_control_message(
        &self,
        task_run_id: i64,
        action: TaskControlAction,
        requester: &str,
    ) -> SchedulerResult<()> {
        let control_message = TaskControlMessage {
            task_run_id,
            action,
            requester: requester.to_string(),
            timestamp: Utc::now(),
        };

        let message = Message::task_control(control_message);
        self.message_queue
            .publish_message(&self.control_queue_name, &message)
            .await?;

        debug!(
            "发送任务控制消息: 任务运行 {} 执行 {:?} 操作",
            task_run_id, action
        );

        Ok(())
    }
    fn can_perform_control_action(
        &self,
        task_run: &TaskRun,
        action: TaskControlAction,
    ) -> SchedulerResult<bool> {
        use TaskControlAction::*;
        use TaskRunStatus::*;

        let can_perform = match (action, &task_run.status) {
            (Pause, Running) => true,
            (Resume, _) => {
                warn!("恢复操作可能需要TaskRunStatus::Paused状态支持");
                false
            }
            (Cancel, Pending | Dispatched | Running) => true,
            (Restart, Failed | Completed | Timeout) => true,
            _ => false,
        };

        if !can_perform {
            debug!(
                "任务运行 {} 当前状态 {:?} 不允许执行 {:?} 操作",
                task_run.id, task_run.status, action
            );
        }

        Ok(can_perform)
    }
    async fn create_task_run_internal(&self, task_id: i64) -> SchedulerResult<TaskRun> {
        let task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;
        if !task.is_active() {
            return Err(SchedulerError::Internal(format!(
                "Task {task_id} is not active"
            )));
        }

        let now = Utc::now();
        let mut task_run = TaskRun::new(task_id, now);
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
}

#[async_trait]
impl TaskControlService for TaskController {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
        info!("手动触发任务: {}", task_id);
        let task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;

        if !task.is_active() {
            return Err(SchedulerError::Internal(format!(
                "Task {task_id} is not active"
            )));
        }
        let running_runs = self.task_run_repo.get_by_task_id(task_id).await?;
        let has_running = running_runs.iter().any(|run| run.is_running());

        if has_running {
            warn!("任务 {} 已有正在运行的实例，跳过手动触发", task_id);
            return Err(SchedulerError::Internal(format!(
                "Task {task_id} already has running instances"
            )));
        }
        let task_run = self.create_task_run_internal(task_id).await?;

        info!(
            "成功手动触发任务 {}，创建了运行实例 {}",
            task_id, task_run.id
        );

        Ok(task_run)
    }
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()> {
        info!("暂停任务: {}", task_id);
        let _task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;
        let running_runs = self.task_run_repo.get_by_task_id(task_id).await?;
        let running_runs: Vec<_> = running_runs
            .into_iter()
            .filter(|run| run.is_running())
            .collect();

        if running_runs.is_empty() {
            warn!("任务 {} 没有正在运行的实例，无需暂停", task_id);
            return Ok(());
        }
        for task_run in running_runs {
            if self.can_perform_control_action(&task_run, TaskControlAction::Pause)? {
                self.send_control_message(task_run.id, TaskControlAction::Pause, "system")
                    .await?;

                info!("向任务运行实例 {} 发送暂停信号", task_run.id);
            }
        }
        self.task_repo
            .batch_update_status(&[task_id], TaskStatus::Inactive)
            .await?;

        info!("成功暂停任务 {} 及其所有运行实例", task_id);

        Ok(())
    }
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()> {
        info!("恢复任务: {}", task_id);
        let _task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;
        self.task_repo
            .batch_update_status(&[task_id], TaskStatus::Active)
            .await?;

        info!("成功恢复任务 {}，任务将在下次调度时重新开始", task_id);

        Ok(())
    }
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun> {
        info!("重启任务运行实例: {}", task_run_id);
        let task_run = self
            .task_run_repo
            .get_by_id(task_run_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskRunNotFound { id: task_run_id })?;
        if !self.can_perform_control_action(&task_run, TaskControlAction::Restart)? {
            return Err(SchedulerError::Internal(format!(
                "Task run {} with status {:?} cannot be restarted",
                task_run_id, task_run.status
            )));
        }
        let new_task_run = self.create_task_run_internal(task_run.task_id).await?;

        info!(
            "成功重启任务运行实例 {}，创建了新的运行实例 {}",
            task_run_id, new_task_run.id
        );

        Ok(new_task_run)
    }
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()> {
        info!("中止任务运行实例: {}", task_run_id);
        let task_run = self
            .task_run_repo
            .get_by_id(task_run_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskRunNotFound { id: task_run_id })?;
        if !self.can_perform_control_action(&task_run, TaskControlAction::Cancel)? {
            warn!(
                "任务运行实例 {} 当前状态 {:?} 不允许中止操作",
                task_run_id, task_run.status
            );
            return Ok(()); // 不是错误，只是无操作
        }
        self.send_control_message(task_run_id, TaskControlAction::Cancel, "system")
            .await?;
        self.task_run_repo
            .update_status(task_run_id, TaskRunStatus::Failed, None)
            .await?;

        info!("成功中止任务运行实例 {}", task_run_id);

        Ok(())
    }

    async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize> {
        info!("取消任务 {} 的所有运行实例", task_id);

        let running_runs = self.task_run_repo.get_by_task_id(task_id).await?;
        let active_runs: Vec<_> = running_runs
            .into_iter()
            .filter(|run| {
                matches!(
                    run.status,
                    TaskRunStatus::Pending | TaskRunStatus::Dispatched | TaskRunStatus::Running
                )
            })
            .collect();

        let cancel_count = active_runs.len();
        for task_run in active_runs {
            if let Err(e) = self.abort_task_run(task_run.id).await {
                error!("取消任务运行实例 {} 时出错: {}", task_run.id, e);
            }
        }

        info!("成功取消任务 {} 的 {} 个运行实例", task_id, cancel_count);

        Ok(cancel_count)
    }

    async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool> {
        let task_runs = self.task_run_repo.get_by_task_id(task_id).await?;
        let has_running = task_runs.iter().any(|run| run.is_running());
        Ok(has_running)
    }

    async fn get_recent_executions(
        &self,
        task_id: i64,
        limit: usize,
    ) -> SchedulerResult<Vec<TaskRun>> {
        let recent_runs = self
            .task_run_repo
            .get_recent_runs(task_id, limit as i64)
            .await?;
        Ok(recent_runs)
    }
}

impl TaskController {
    pub async fn get_task_status_summary(
        &self,
        task_id: i64,
    ) -> SchedulerResult<TaskStatusSummary> {
        let task_runs = self.task_run_repo.get_by_task_id(task_id).await?;

        let mut summary = TaskStatusSummary::default();

        for run in task_runs {
            match run.status {
                TaskRunStatus::Pending => summary.pending += 1,
                TaskRunStatus::Dispatched => summary.dispatched += 1,
                TaskRunStatus::Running => summary.running += 1,
                TaskRunStatus::Completed => summary.completed += 1,
                TaskRunStatus::Failed => summary.failed += 1,
                TaskRunStatus::Timeout => summary.cancelled += 1,
            }
        }

        Ok(summary)
    }
}
