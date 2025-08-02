//! 领域仓储抽象
//! 
//! 定义数据访问的抽象接口，遵循依赖倒置原则

use async_trait::async_trait;
use crate::entities::{Task, TaskRun, Worker};
use scheduler_core::SchedulerResult;

/// 任务仓储抽象
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, task: &Task) -> SchedulerResult<Task>;
    async fn find_by_id(&self, id: i64) -> SchedulerResult<Option<Task>>;
    async fn find_all(&self) -> SchedulerResult<Vec<Task>>;
    async fn update(&self, task: &Task) -> SchedulerResult<Task>;
    async fn delete(&self, id: i64) -> SchedulerResult<bool>;
    async fn find_ready_for_execution(&self) -> SchedulerResult<Vec<Task>>;
}

/// 任务执行仓储抽象
#[async_trait]
pub trait TaskRunRepository: Send + Sync {
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun>;
    async fn find_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>>;
    async fn find_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>>;
    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun>;
    async fn delete(&self, id: i64) -> SchedulerResult<bool>;
    async fn find_running(&self) -> SchedulerResult<Vec<TaskRun>>;
}

/// Worker仓储抽象
#[async_trait]
pub trait WorkerRepository: Send + Sync {
    async fn register(&self, worker: &Worker) -> SchedulerResult<Worker>;
    async fn find_by_id(&self, id: &str) -> SchedulerResult<Option<Worker>>;
    async fn find_all(&self) -> SchedulerResult<Vec<Worker>>;
    async fn update(&self, worker: &Worker) -> SchedulerResult<Worker>;
    async fn unregister(&self, id: &str) -> SchedulerResult<bool>;
    async fn find_available(&self) -> SchedulerResult<Vec<Worker>>;
}