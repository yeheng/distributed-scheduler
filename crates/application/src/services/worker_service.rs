// Worker service implementation placeholder
use crate::interfaces::WorkerServiceTrait;
use scheduler_core::{models::TaskStatusUpdate, SchedulerResult};
use scheduler_domain::entities::TaskRun;

pub struct WorkerService;

#[async_trait::async_trait]
impl WorkerServiceTrait for WorkerService {
    async fn start(&self) -> SchedulerResult<()> {
        todo!("Implementation will be moved from core")
    }

    async fn stop(&self) -> SchedulerResult<()> {
        todo!("Implementation will be moved from core")
    }

    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()> {
        todo!("Implementation will be moved from core")
    }

    async fn send_status_update(&self, _update: TaskStatusUpdate) -> SchedulerResult<()> {
        todo!("Implementation will be moved from core")
    }

    async fn get_current_task_count(&self) -> i32 {
        todo!("Implementation will be moved from core")
    }

    async fn can_accept_task(&self, _task_type: &str) -> bool {
        todo!("Implementation will be moved from core")
    }

    async fn cancel_task(&self, _task_run_id: i64) -> SchedulerResult<()> {
        todo!("Implementation will be moved from core")
    }

    async fn get_running_tasks(&self) -> Vec<TaskRun> {
        todo!("Implementation will be moved from core")
    }

    async fn is_task_running(&self, _task_run_id: i64) -> bool {
        todo!("Implementation will be moved from core")
    }

    async fn send_heartbeat(&self) -> SchedulerResult<()> {
        todo!("Implementation will be moved from core")
    }
}
