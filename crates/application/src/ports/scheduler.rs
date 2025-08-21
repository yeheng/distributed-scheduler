use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use scheduler_errors::SchedulerResult;
use scheduler_domain::entities::{
    Task, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus,
};
use scheduler_domain::events::TaskStatusUpdate;

/// Service interface for task control operations
#[async_trait]
pub trait TaskControlService: Send + Sync {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize>;
    async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool>;
    async fn get_recent_executions(
        &self,
        task_id: i64,
        limit: usize,
    ) -> SchedulerResult<Vec<TaskRun>>;
}

/// Service interface for task scheduling operations
#[async_trait]
pub trait TaskSchedulerService: Send + Sync {
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>>;
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool>;
    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun>;
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()>;
    async fn start(&self) -> SchedulerResult<()>;
    async fn stop(&self) -> SchedulerResult<()>;
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<()>;
    async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()>;
    async fn is_running(&self) -> bool;
    async fn get_stats(&self) -> SchedulerResult<SchedulerStats>;
    async fn reload_config(&self) -> SchedulerResult<()>;
}

/// Service interface for task dispatch operations
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
    async fn redispatch_failed_tasks(&self) -> SchedulerResult<usize>;
    async fn get_dispatch_stats(&self) -> SchedulerResult<DispatchStats>;
}

/// Service interface for worker management operations
#[async_trait]
pub trait WorkerManagementService: Send + Sync {
    async fn register_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()>;
    async fn unregister_worker(&self, worker_id: &str) -> SchedulerResult<()>;
    async fn update_worker_status(
        &self,
        worker_id: &str,
        status: WorkerStatus,
    ) -> SchedulerResult<()>;
    async fn get_active_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn get_worker_details(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;
    async fn check_worker_health(&self, worker_id: &str) -> SchedulerResult<bool>;
    async fn get_worker_load_stats(&self) -> SchedulerResult<HashMap<String, WorkerLoadStats>>;
    async fn select_best_worker(&self, task_type: &str) -> SchedulerResult<Option<String>>;
    async fn process_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_data: &WorkerHeartbeat,
    ) -> SchedulerResult<()>;
}

/// Service interface for worker operations
#[async_trait]
pub trait WorkerService: Send + Sync {
    async fn start(&self) -> SchedulerResult<()>;
    async fn stop(&self) -> SchedulerResult<()>;
    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()>;
    async fn send_status_update(
        &self,
        update: TaskStatusUpdate,
    ) -> SchedulerResult<()>;
    async fn get_current_task_count(&self) -> i32;
    async fn can_accept_task(&self, task_type: &str) -> bool;
    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn get_running_tasks(&self) -> Vec<TaskRun>;
    async fn is_task_running(&self, task_run_id: i64) -> bool;
    async fn send_heartbeat(&self) -> SchedulerResult<()>;
}

/// Service interface for state listening operations
#[async_trait]
pub trait StateListenerService: Send + Sync {
    async fn listen_for_updates(&self) -> SchedulerResult<()>;
    async fn process_status_update(
        &self,
        task_run_id: i64,
        status: TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;
}

// Data structures
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub total_tasks: i64,
    pub active_tasks: i64,
    pub running_task_runs: i64,
    pub pending_task_runs: i64,
    pub uptime_seconds: u64,
    pub last_schedule_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct DispatchStats {
    pub total_dispatched: i64,
    pub successful_dispatched: i64,
    pub failed_dispatched: i64,
    pub redispatched: i64,
    pub avg_dispatch_time_ms: f64,
}

#[derive(Debug, Clone)]
pub struct WorkerLoadStats {
    pub worker_id: String,
    pub current_task_count: i32,
    pub max_concurrent_tasks: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WorkerHeartbeat {
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: DateTime<Utc>,
}