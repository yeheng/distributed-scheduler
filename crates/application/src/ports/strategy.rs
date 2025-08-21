use async_trait::async_trait;
use scheduler_domain::entities::{Task, WorkerInfo};
use scheduler_errors::SchedulerResult;

/// Interface for task dispatch strategies
#[async_trait]
pub trait TaskDispatchStrategy: Send + Sync {
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>>;
    fn name(&self) -> &str;
}
