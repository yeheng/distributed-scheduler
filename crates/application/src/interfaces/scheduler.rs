use async_trait::async_trait;

use scheduler_domain::entities::{Task, TaskRun, WorkerInfo};
use scheduler_foundation::{SchedulerResult, TaskStatusUpdate};

#[async_trait]
pub trait TaskSchedulerService: Send + Sync {
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>>;
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool>;
    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun>;
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()>;
}

#[async_trait]
pub trait StateListenerService: Send + Sync {
    async fn listen_for_updates(&self) -> SchedulerResult<()>;
    async fn process_status_update(
        &self,
        task_run_id: i64,
        status: scheduler_domain::entities::TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskControlService: Send + Sync {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskDispatchStrategy: Send + Sync {
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>>;
    fn name(&self) -> &str;
}

#[async_trait]
pub trait WorkerServiceTrait: Send + Sync {
    async fn start(&self) -> SchedulerResult<()>;
    async fn stop(&self) -> SchedulerResult<()>;
    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()>;
    async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()>;
    async fn get_current_task_count(&self) -> i32;
    async fn can_accept_task(&self, task_type: &str) -> bool;
    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn get_running_tasks(&self) -> Vec<TaskRun>;
    async fn is_task_running(&self, task_run_id: i64) -> bool;
    async fn send_heartbeat(&self) -> SchedulerResult<()>;
}
