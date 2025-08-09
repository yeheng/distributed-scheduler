use crate::{
    models::{Task, TaskRun, TaskRunStatus},
    SchedulerResult,
};
use async_trait::async_trait;

#[async_trait]
pub trait TaskControlService: Send + Sync {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskRunManagementService: Send + Sync {
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize>;
    async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool>;
}

#[async_trait]
pub trait TaskHistoryService: Send + Sync {
    async fn get_recent_executions(
        &self,
        task_id: i64,
        limit: usize,
    ) -> SchedulerResult<Vec<TaskRun>>;
    async fn get_task_run_details(&self, task_run_id: i64) -> SchedulerResult<Option<TaskRun>>;
}

#[async_trait]
pub trait SchedulerService: Send + Sync {
    async fn start(&self) -> SchedulerResult<()>;
    async fn stop(&self) -> SchedulerResult<()>;
    async fn is_running(&self) -> bool;
}

#[async_trait]
pub trait TaskSchedulingService: Send + Sync {
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<()>;
    async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()>;
    async fn reload_config(&self) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskDispatchService: Send + Sync {
    async fn dispatch_task(&self, task_run: &TaskRun, worker_id: &str) -> SchedulerResult<()>;
    async fn dispatch_tasks(&self, dispatches: &[(TaskRun, String)]) -> SchedulerResult<()>;
    async fn handle_status_update(
        &self,
        task_run_id: i64,
        status: TaskRunStatus,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskRetryService: Send + Sync {
    async fn redispatch_failed_tasks(&self) -> SchedulerResult<usize>;
    async fn retry_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
}

#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub total_tasks: i64,
    pub active_tasks: i64,
    pub running_task_runs: i64,
    pub pending_task_runs: i64,
    pub uptime_seconds: u64,
    pub last_schedule_time: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct DispatchStats {
    pub total_dispatched: i64,
    pub successful_dispatched: i64,
    pub failed_dispatched: i64,
    pub redispatched: i64,
    pub avg_dispatch_time_ms: f64,
}
