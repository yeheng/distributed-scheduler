
use crate::{
    entities::{Task, TaskRun, TaskRunStatus},
    repositories::TaskRunRepository,
};
use scheduler_errors::{SchedulerError, SchedulerResult};
use async_trait::async_trait;

#[async_trait]
pub trait TaskSchedulingService: Send + Sync {
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<TaskRun>;
    async fn cancel_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn retry_failed_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
}

pub struct DefaultTaskSchedulingService<TRR>
where
    TRR: TaskRunRepository,
{
    task_run_repo: TRR,
}

impl<TRR> DefaultTaskSchedulingService<TRR>
where
    TRR: TaskRunRepository,
{
    pub fn new(task_run_repo: TRR) -> Self {
        Self { task_run_repo }
    }
}

#[async_trait]
impl<TRR> TaskSchedulingService for DefaultTaskSchedulingService<TRR>
where
    TRR: TaskRunRepository + 'static,
{
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<TaskRun> {
        let now = chrono::Utc::now();
        let task_run = TaskRun {
            id: 0, // 数据库生成
            task_id: task.id,
            status: TaskRunStatus::Pending,
            worker_id: None, // 稍后分配
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: now,
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: now,
        };

        self.task_run_repo.create(&task_run).await
    }

    async fn cancel_task_run(&self, task_run_id: i64) -> SchedulerResult<()> {
        if let Some(mut task_run) = self.task_run_repo.get_by_id(task_run_id).await? {
            task_run.status = TaskRunStatus::Failed;
            task_run.completed_at = Some(chrono::Utc::now());
            task_run.error_message = Some("Cancelled by user".to_string());
            self.task_run_repo.update(&task_run).await?;
        }
        Ok(())
    }

    async fn retry_failed_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun> {
        if let Some(failed_run) = self.task_run_repo.get_by_id(task_run_id).await? {
            let now = chrono::Utc::now();
            let retry_run = TaskRun {
                id: 0,
                task_id: failed_run.task_id,
                status: TaskRunStatus::Pending,
                worker_id: None,
                retry_count: failed_run.retry_count + 1,
                shard_index: failed_run.shard_index,
                shard_total: failed_run.shard_total,
                scheduled_at: now,
                started_at: None,
                completed_at: None,
                result: None,
                error_message: None,
                created_at: now,
            };

            self.task_run_repo.create(&retry_run).await
        } else {
            Err(SchedulerError::TaskRunNotFound { id: task_run_id })
        }
    }
}
