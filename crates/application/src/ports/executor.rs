use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

use scheduler_domain::entities::{Task, TaskResult, TaskRun};
use scheduler_errors::SchedulerResult;

/// Interface for task execution
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute_task(&self, context: &TaskExecutionContext) -> SchedulerResult<TaskResult>;

    fn supports_task_type(&self, task_type: &str) -> bool;
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> &str;
    fn supported_task_types(&self) -> Vec<String>;
    async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn is_running(&self, task_run_id: i64) -> SchedulerResult<bool>;
    async fn get_status(&self) -> SchedulerResult<ExecutorStatus>;
    async fn health_check(&self) -> SchedulerResult<bool>;
    async fn warm_up(&self) -> SchedulerResult<()>;
    async fn cleanup(&self) -> SchedulerResult<()>;
}

/// Interface for executor registry management
#[async_trait]
pub trait ExecutorRegistry: Send + Sync {
    async fn register(
        &mut self,
        name: String,
        executor: Arc<dyn TaskExecutor>,
    ) -> SchedulerResult<()>;
    async fn get(&self, name: &str) -> Option<Arc<dyn TaskExecutor>>;
    async fn list_executors(&self) -> Vec<String>;
    async fn unregister(&mut self, name: &str) -> SchedulerResult<bool>;
    async fn clear(&mut self);
    async fn contains(&self, name: &str) -> bool;
    async fn count(&self) -> usize;
    async fn get_all_status(&self) -> SchedulerResult<HashMap<String, ExecutorStatus>>;
    async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>>;
    async fn get_by_task_type(
        &self,
        task_type: &str,
    ) -> SchedulerResult<Vec<Arc<dyn TaskExecutor>>>;
}

// Data structures
#[derive(Debug, Clone)]
pub struct TaskExecutionContext {
    pub task_run: TaskRun,
    pub task_type: String,
    pub parameters: HashMap<String, Value>,
    pub timeout_seconds: u64,
    pub environment: HashMap<String, String>,
    pub working_directory: Option<String>,
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_percent: Option<f64>,
    pub max_disk_mb: Option<u64>,
    pub max_network_kbps: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ExecutorStatus {
    pub name: String,
    pub version: String,
    pub healthy: bool,
    pub running_tasks: i32,
    pub supported_task_types: Vec<String>,
    pub last_health_check: DateTime<Utc>,
    pub metadata: HashMap<String, Value>,
}