// Scheduler service implementation placeholder
use crate::interfaces::TaskSchedulerService;
use scheduler_core::SchedulerResult;
use scheduler_domain::entities::{Task, TaskRun};

pub struct SchedulerService;

#[async_trait::async_trait]
impl TaskSchedulerService for SchedulerService {
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>> {
        todo!("Implementation will be moved from core")
    }

    async fn check_dependencies(&self, _task: &Task) -> SchedulerResult<bool> {
        todo!("Implementation will be moved from core")
    }

    async fn create_task_run(&self, _task: &Task) -> SchedulerResult<TaskRun> {
        todo!("Implementation will be moved from core")
    }

    async fn dispatch_to_queue(&self, _task_run: &TaskRun) -> SchedulerResult<()> {
        todo!("Implementation will be moved from core")
    }
}
