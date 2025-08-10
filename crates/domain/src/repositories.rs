use crate::entities::{Task, TaskFilter, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_errors::SchedulerResult;

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, task: &Task) -> SchedulerResult<Task>;
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>>;
    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>>;
    async fn update(&self, task: &Task) -> SchedulerResult<()>;
    async fn delete(&self, id: i64) -> SchedulerResult<()>;
    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>>;
    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>>;
    async fn get_schedulable_tasks(
        &self,
        current_time: DateTime<Utc>,
    ) -> SchedulerResult<Vec<Task>>;
    async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool>;
    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>>;
    async fn batch_update_status(
        &self,
        task_ids: &[i64],
        status: crate::entities::TaskStatus,
    ) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskRunRepository: Send + Sync {
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun>;
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>>;
    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<()>;
    async fn delete(&self, id: i64) -> SchedulerResult<()>;
    async fn get_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>>;
    async fn get_by_worker_id(&self, worker_id: &str) -> SchedulerResult<Vec<TaskRun>>;
    async fn get_by_status(&self, status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>>;
    async fn get_pending_runs(&self, limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>>;
    async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>>;
    async fn get_timeout_runs(&self, timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>>;
    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        worker_id: Option<&str>,
    ) -> SchedulerResult<()>;
    async fn update_result(
        &self,
        id: i64,
        result: Option<&str>,
        error_message: Option<&str>,
    ) -> SchedulerResult<()>;
    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> SchedulerResult<Vec<TaskRun>>;
    async fn get_execution_stats(
        &self,
        task_id: i64,
        days: i32,
    ) -> SchedulerResult<TaskExecutionStats>;
    async fn cleanup_old_runs(&self, days: i32) -> SchedulerResult<u64>;
    async fn batch_update_status(
        &self,
        run_ids: &[i64],
        status: TaskRunStatus,
    ) -> SchedulerResult<()>;
}

#[async_trait]
pub trait WorkerRepository: Send + Sync {
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()>;
    async fn unregister(&self, worker_id: &str) -> SchedulerResult<()>;
    async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;
    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()>;
    async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn get_workers_by_task_type(&self, task_type: &str) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: DateTime<Utc>,
        current_task_count: i32,
    ) -> SchedulerResult<()>;
    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()>;
    async fn get_timeout_workers(&self, timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn cleanup_offline_workers(&self, timeout_seconds: i64) -> SchedulerResult<u64>;
    async fn get_worker_load_stats(&self) -> SchedulerResult<Vec<WorkerLoadStats>>;
    async fn batch_update_status(
        &self,
        worker_ids: &[String],
        status: WorkerStatus,
    ) -> SchedulerResult<()>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskExecutionStats {
    pub task_id: i64,
    pub total_runs: i64,
    pub successful_runs: i64,
    pub failed_runs: i64,
    pub timeout_runs: i64,
    pub average_execution_time_ms: Option<f64>,
    pub success_rate: f64,
    pub last_execution: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerLoadStats {
    pub worker_id: String,
    pub current_task_count: i32,
    pub max_concurrent_tasks: i32,
    pub load_percentage: f64,
    pub total_completed_tasks: i64,
    pub total_failed_tasks: i64,
    pub average_task_duration_ms: Option<f64>,
    pub last_heartbeat: DateTime<Utc>,
}

impl TaskExecutionStats {
    pub fn calculate_success_rate(&mut self) {
        if self.total_runs > 0 {
            self.success_rate = (self.successful_runs as f64 / self.total_runs as f64) * 100.0;
        } else {
            self.success_rate = 0.0;
        }
    }
}

impl WorkerLoadStats {
    pub fn calculate_load_percentage(&mut self) {
        if self.max_concurrent_tasks > 0 {
            self.load_percentage =
                (self.current_task_count as f64 / self.max_concurrent_tasks as f64) * 100.0;
        } else {
            self.load_percentage = 0.0;
        }
    }
}
