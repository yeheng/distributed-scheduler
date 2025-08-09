use crate::{
    models::{WorkerInfo, WorkerStatus},
    SchedulerResult,
};
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait WorkerRegistrationService: Send + Sync {
    async fn register_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()>;
    async fn unregister_worker(&self, worker_id: &str) -> SchedulerResult<()>;
    async fn update_worker_status(
        &self,
        worker_id: &str,
        status: WorkerStatus,
    ) -> SchedulerResult<()>;
}

#[async_trait]
pub trait WorkerQueryService: Send + Sync {
    async fn get_active_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn get_worker_details(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;
    async fn get_worker_load_stats(&self) -> SchedulerResult<HashMap<String, WorkerLoadStats>>;
}

#[async_trait]
pub trait WorkerHealthService: Send + Sync {
    async fn check_worker_health(&self, worker_id: &str) -> SchedulerResult<bool>;
    async fn process_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_data: &WorkerHeartbeat,
    ) -> SchedulerResult<()>;
}

#[async_trait]
pub trait WorkerSelectionService: Send + Sync {
    async fn select_best_worker(&self, task_type: &str) -> SchedulerResult<Option<String>>;
    async fn select_workers_for_batch(
        &self,
        task_type: &str,
        count: usize,
    ) -> SchedulerResult<Vec<String>>;
}

#[derive(Debug, Clone)]
pub struct WorkerLoadStats {
    pub worker_id: String,
    pub current_task_count: i32,
    pub max_concurrent_tasks: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct WorkerHeartbeat {
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
