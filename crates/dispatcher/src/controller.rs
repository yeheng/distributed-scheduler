use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tracing::{debug, error, info, warn};

use scheduler_core::{
    models::{Message, TaskControlAction, TaskControlMessage, TaskRun, TaskRunStatus, TaskStatus},
    traits::{MessageQueue, TaskControlService, TaskRepository, TaskRunRepository},
    SchedulerResult, SchedulerError,
};

/// 任务控制器实现
pub struct TaskController {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    message_queue: Arc<dyn MessageQueue>,
    control_queue_name: String,
}

impl TaskController {
    /// 创建新的任务控制器
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

    /// 发送任务控制消息
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

        // 发送控制消息到消息队列
        self.message_queue
            .publish_message(&self.control_queue_name, &message)
            .await?;

        debug!(
            "发送任务控制消息: 任务运行 {} 执行 {:?} 操作",
            task_run_id, action
        );

        Ok(())
    }

    /// 验证任务运行是否可以执行指定的控制操作
    fn can_perform_control_action(
        &self,
        task_run: &TaskRun,
        action: TaskControlAction,
    ) -> SchedulerResult<bool> {
        use TaskControlAction::*;
        use TaskRunStatus::*;

        let can_perform = match (action, &task_run.status) {
            // 暂停操作: 只有运行中的任务可以暂停
            (Pause, Running) => true,

            // 恢复操作: 只有已暂停的任务可以恢复 (这里假设我们有Paused状态)
            (Resume, _) => {
                // 注意: 当前TaskRunStatus可能没有Paused状态，这里需要根据实际情况调整
                warn!("恢复操作可能需要TaskRunStatus::Paused状态支持");
                false
            }

            // 取消操作: Pending, Dispatched, Running状态的任务可以取消
            (Cancel, Pending | Dispatched | Running) => true,

            // 重启操作: Failed, Completed, Timeout状态的任务可以重启
            (Restart, Failed | Completed | Timeout) => true,

            // 其他情况都不允许
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

    /// 创建任务运行实例的内部实现
    async fn create_task_run_internal(&self, task_id: i64) -> SchedulerResult<TaskRun> {
        let task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;

        // 检查任务是否处于活跃状态
        if !task.is_active() {
            return Err(SchedulerError::Internal(format!(
                "Task {task_id} is not active"
            )));
        }

        let now = Utc::now();
        let mut task_run = TaskRun::new(task_id, now);

        // 如果任务配置了分片，设置分片信息
        if let Some(shard_config) = &task.shard_config {
            if shard_config.enabled {
                task_run.shard_index = Some(0);
                task_run.shard_total = Some(shard_config.shard_count);
            }
        }

        // 保存到数据库
        let created_run = self.task_run_repo.create(&task_run).await?;

        info!("为任务 {} 创建了新的执行实例 {}", task.name, created_run.id);

        Ok(created_run)
    }
}

#[async_trait]
impl TaskControlService for TaskController {
    /// 手动触发任务
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
        info!("手动触发任务: {}", task_id);

        // 检查任务是否存在且处于活跃状态
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

        // 检查是否已有正在运行的实例
        let running_runs = self.task_run_repo.get_by_task_id(task_id).await?;
        let has_running = running_runs.iter().any(|run| run.is_running());

        if has_running {
            warn!("任务 {} 已有正在运行的实例，跳过手动触发", task_id);
            return Err(SchedulerError::Internal(format!(
                "Task {task_id} already has running instances"
            )));
        }

        // 创建任务运行实例
        let task_run = self.create_task_run_internal(task_id).await?;

        // 这里可以考虑直接调用调度器的分发逻辑，或者发送任务执行消息
        // 为了简化，这里只创建TaskRun，具体的分发逻辑由调度器处理

        info!(
            "成功手动触发任务 {}，创建了运行实例 {}",
            task_id, task_run.id
        );

        Ok(task_run)
    }

    /// 暂停任务
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()> {
        info!("暂停任务: {}", task_id);

        // 检查任务是否存在
        let _task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;

        // 获取任务的所有正在运行的实例
        let running_runs = self.task_run_repo.get_by_task_id(task_id).await?;
        let running_runs: Vec<_> = running_runs
            .into_iter()
            .filter(|run| run.is_running())
            .collect();

        if running_runs.is_empty() {
            warn!("任务 {} 没有正在运行的实例，无需暂停", task_id);
            return Ok(());
        }

        // 向所有正在运行的实例发送暂停信号
        for task_run in running_runs {
            if self.can_perform_control_action(&task_run, TaskControlAction::Pause)? {
                self.send_control_message(task_run.id, TaskControlAction::Pause, "system")
                    .await?;

                info!("向任务运行实例 {} 发送暂停信号", task_run.id);
            }
        }

        // 将任务状态设置为不活跃，防止新的实例被调度
        self.task_repo
            .batch_update_status(&[task_id], TaskStatus::Inactive)
            .await?;

        info!("成功暂停任务 {} 及其所有运行实例", task_id);

        Ok(())
    }

    /// 恢复任务
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()> {
        info!("恢复任务: {}", task_id);

        // 检查任务是否存在
        let _task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;

        // 将任务状态设置为活跃，允许新的实例被调度
        self.task_repo
            .batch_update_status(&[task_id], TaskStatus::Active)
            .await?;

        // 注意: 这里没有发送恢复信号给已暂停的任务运行实例，
        // 因为当前的TaskRunStatus可能没有Paused状态
        // 如果需要支持恢复已暂停的实例，需要扩展TaskRunStatus枚举

        info!("成功恢复任务 {}，任务将在下次调度时重新开始", task_id);

        Ok(())
    }

    /// 重启任务运行实例
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun> {
        info!("重启任务运行实例: {}", task_run_id);

        // 检查任务运行实例是否存在
        let task_run = self
            .task_run_repo
            .get_by_id(task_run_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskRunNotFound { id: task_run_id })?;

        // 验证是否可以重启
        if !self.can_perform_control_action(&task_run, TaskControlAction::Restart)? {
            return Err(SchedulerError::Internal(format!(
                "Task run {} with status {:?} cannot be restarted",
                task_run_id, task_run.status
            )));
        }

        // 创建新的任务运行实例
        let new_task_run = self.create_task_run_internal(task_run.task_id).await?;

        // 可以选择向原实例发送重启信号，或者直接让新实例开始执行
        // 这里我们选择让调度器处理新实例的分发

        info!(
            "成功重启任务运行实例 {}，创建了新的运行实例 {}",
            task_run_id, new_task_run.id
        );

        Ok(new_task_run)
    }

    /// 中止任务运行实例
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()> {
        info!("中止任务运行实例: {}", task_run_id);

        // 检查任务运行实例是否存在
        let task_run = self
            .task_run_repo
            .get_by_id(task_run_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskRunNotFound { id: task_run_id })?;

        // 验证是否可以中止
        if !self.can_perform_control_action(&task_run, TaskControlAction::Cancel)? {
            warn!(
                "任务运行实例 {} 当前状态 {:?} 不允许中止操作",
                task_run_id, task_run.status
            );
            return Ok(()); // 不是错误，只是无操作
        }

        // 发送取消信号
        self.send_control_message(task_run_id, TaskControlAction::Cancel, "system")
            .await?;

        // 直接更新数据库状态（以防Worker无法及时处理消息）
        self.task_run_repo
            .update_status(task_run_id, TaskRunStatus::Failed, None)
            .await?;

        info!("成功中止任务运行实例 {}", task_run_id);

        Ok(())
    }
}

impl TaskController {
    /// 获取任务的当前运行状态统计
    pub async fn get_task_status_summary(&self, task_id: i64) -> SchedulerResult<TaskStatusSummary> {
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

    /// 批量操作: 取消任务的所有运行实例
    pub async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize> {
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

        // 批量发送取消信号
        for task_run in active_runs {
            if let Err(e) = self.abort_task_run(task_run.id).await {
                error!("取消任务运行实例 {} 时出错: {}", task_run.id, e);
            }
        }

        info!("成功取消任务 {} 的 {} 个运行实例", task_id, cancel_count);

        Ok(cancel_count)
    }

    /// 检查任务是否有运行中的实例
    pub async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool> {
        let task_runs = self.task_run_repo.get_by_task_id(task_id).await?;
        let has_running = task_runs.iter().any(|run| run.is_running());
        Ok(has_running)
    }

    /// 获取任务的最近执行历史
    pub async fn get_recent_executions(&self, task_id: i64, limit: usize) -> SchedulerResult<Vec<TaskRun>> {
        let recent_runs = self
            .task_run_repo
            .get_recent_runs(task_id, limit as i64)
            .await?;
        Ok(recent_runs)
    }
}

/// 任务状态统计摘要
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
    /// 获取总的运行实例数量
    pub fn total(&self) -> usize {
        self.pending
            + self.dispatched
            + self.running
            + self.completed
            + self.failed
            + self.cancelled
    }

    /// 获取活跃的运行实例数量（Pending + Dispatched + Running）
    pub fn active(&self) -> usize {
        self.pending + self.dispatched + self.running
    }

    /// 获取完成的运行实例数量（Completed + Failed + Cancelled）
    pub fn finished(&self) -> usize {
        self.completed + self.failed + self.cancelled
    }
}
