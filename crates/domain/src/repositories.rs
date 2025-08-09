
use async_trait::async_trait;
use crate::errors::SchedulerResult;
use crate::entities::{Task, TaskRun, WorkerInfo};

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, task: &Task) -> SchedulerResult<Task>;
    async fn find_by_id(&self, id: i64) -> SchedulerResult<Option<Task>>;
    async fn find_all(&self) -> SchedulerResult<Vec<Task>>;
    async fn update(&self, task: &Task) -> SchedulerResult<Task>;
    async fn delete(&self, id: i64) -> SchedulerResult<bool>;
    async fn find_ready_for_execution(&self) -> SchedulerResult<Vec<Task>>;
}

#[async_trait]
pub trait TaskRunRepository: Send + Sync {
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun>;
    async fn find_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>>;
    async fn find_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>>;
    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun>;
    async fn delete(&self, id: i64) -> SchedulerResult<bool>;
    async fn find_running(&self) -> SchedulerResult<Vec<TaskRun>>;
}

#[async_trait]
pub trait WorkerRepository: Send + Sync {
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<WorkerInfo>;
    async fn find_by_id(&self, id: &str) -> SchedulerResult<Option<WorkerInfo>>;
    async fn find_all(&self) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<WorkerInfo>;
    async fn unregister(&self, id: &str) -> SchedulerResult<bool>;
    async fn find_available(&self) -> SchedulerResult<Vec<Worker>>;
}
