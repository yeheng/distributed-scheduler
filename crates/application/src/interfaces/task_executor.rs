use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use scheduler_domain::entities::TaskResult;
use scheduler_core::SchedulerResult;
use scheduler_domain::entities::TaskRun;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionContextTrait {
    pub task_run: TaskRun,
    pub task_type: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub timeout_seconds: u64,
    pub environment: HashMap<String, String>,
    pub working_directory: Option<String>,
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_percent: Option<f64>,
    pub max_disk_mb: Option<u64>,
    pub max_network_kbps: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorStatus {
    pub name: String,
    pub version: String,
    pub healthy: bool,
    pub running_tasks: i32,
    pub supported_task_types: Vec<String>,
    pub last_health_check: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute_task(
        &self,
        context: &TaskExecutionContextTrait,
    ) -> SchedulerResult<TaskResult>;
    
    async fn execute(&self, task_run: &TaskRun) -> SchedulerResult<TaskResult> {
        let task_info = task_run
            .result
            .as_ref()
            .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let parameters = task_info
            .get("parameters")
            .and_then(|p| p.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let task_type = task_info
            .get("task_type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown")
            .to_string();

        let context = TaskExecutionContextTrait {
            task_run: task_run.clone(),
            task_type,
            parameters,
            timeout_seconds: 300, // 默认5分钟
            environment: HashMap::new(),
            working_directory: None,
            resource_limits: ResourceLimits::default(),
        };

        self.execute_task(&context).await
    }
    
    fn supports_task_type(&self, task_type: &str) -> bool;
    fn name(&self) -> &str;
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn description(&self) -> &str {
        "Generic task executor"
    }
    fn supported_task_types(&self) -> Vec<String> {
        vec![]
    }
    async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn is_running(&self, task_run_id: i64) -> SchedulerResult<bool>;
    async fn get_status(&self) -> SchedulerResult<ExecutorStatus> {
        Ok(ExecutorStatus {
            name: self.name().to_string(),
            version: self.version().to_string(),
            healthy: true,
            running_tasks: 0,
            supported_task_types: self.supported_task_types(),
            last_health_check: Utc::now(),
            metadata: HashMap::new(),
        })
    }
    async fn health_check(&self) -> SchedulerResult<bool> {
        Ok(true)
    }
    async fn warm_up(&self) -> SchedulerResult<()> {
        Ok(())
    }
    async fn cleanup(&self) -> SchedulerResult<()> {
        Ok(())
    }
}

#[async_trait]
pub trait ExecutorFactory: Send + Sync {
    async fn create_executor(
        &self,
        executor_type: &str,
        config: &ExecutorConfig,
    ) -> SchedulerResult<Arc<dyn TaskExecutor>>;
    fn supported_types(&self) -> Vec<String>;
    fn validate_config(&self, executor_type: &str, config: &ExecutorConfig) -> SchedulerResult<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    pub parameters: HashMap<String, serde_json::Value>,
    pub resource_limits: ResourceLimits,
    pub enabled: bool,
    pub max_concurrent_tasks: Option<u32>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            parameters: HashMap::new(),
            resource_limits: ResourceLimits::default(),
            enabled: true,
            max_concurrent_tasks: None,
        }
    }
}

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