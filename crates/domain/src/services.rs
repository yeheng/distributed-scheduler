
use crate::{
    models::{Task, TaskRun, TaskRunStatus},
    repositories::TaskRunRepository,
};
use async_trait::async_trait;
use scheduler_core::{SchedulerError, SchedulerResult};

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
        let task_run = TaskRun {
            id: 0, // 数据库生成
            task_id: task.id,
            worker_id: "".to_string(), // 稍后分配
            status: TaskRunStatus::Pending,
            started_at: chrono::Utc::now(),
            completed_at: None,
            error_message: None,
            result: None,
            retry_count: 0,
        };

        self.task_run_repo.create(&task_run).await
    }

    async fn cancel_task_run(&self, task_run_id: i64) -> SchedulerResult<()> {
        if let Some(mut task_run) = self.task_run_repo.find_by_id(task_run_id).await? {
            task_run.status = TaskRunStatus::Cancelled;
            task_run.completed_at = Some(chrono::Utc::now());
            self.task_run_repo.update(&task_run).await?;
        }
        Ok(())
    }

    async fn retry_failed_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun> {
        if let Some(failed_run) = self.task_run_repo.find_by_id(task_run_id).await? {
            let retry_run = TaskRun {
                id: 0,
                task_id: failed_run.task_id,
                worker_id: "".to_string(),
                status: TaskRunStatus::Pending,
                started_at: chrono::Utc::now(),
                completed_at: None,
                error_message: None,
                result: None,
                retry_count: failed_run.retry_count + 1,
            };

            self.task_run_repo.create(&retry_run).await
        } else {
            Err(SchedulerError::TaskRunNotFound { id: task_run_id })
        }
    }
}
