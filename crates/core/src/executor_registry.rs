use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{
    traits::{ExecutorRegistry, ExecutorStatus, TaskExecutor},
    SchedulerResult,
};

pub struct DefaultExecutorRegistry {
    executors: Arc<RwLock<HashMap<String, Arc<dyn TaskExecutor>>>>,
}

impl DefaultExecutorRegistry {
    pub fn new() -> Self {
        Self {
            executors: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub async fn register_batch(
        &mut self,
        executors: Vec<(String, Arc<dyn TaskExecutor>)>,
    ) -> SchedulerResult<()> {
        let mut registry = self.executors.write().await;
        for (name, executor) in executors {
            registry.insert(name, executor);
        }
        Ok(())
    }
    pub async fn get_executor_info(&self, name: &str) -> SchedulerResult<Option<ExecutorInfo>> {
        let registry = self.executors.read().await;
        if let Some(executor) = registry.get(name) {
            let status = executor.get_status().await?;
            Ok(Some(ExecutorInfo {
                name: executor.name().to_string(),
                version: executor.version().to_string(),
                description: executor.description().to_string(),
                supported_task_types: executor.supported_task_types(),
                status,
            }))
        } else {
            Ok(None)
        }
    }
    pub async fn get_all_executor_info(&self) -> SchedulerResult<Vec<ExecutorInfo>> {
        let registry = self.executors.read().await;
        let mut infos = Vec::new();

        for executor in registry.values() {
            let status = executor
                .get_status()
                .await
                .unwrap_or_else(|_| ExecutorStatus {
                    name: executor.name().to_string(),
                    version: executor.version().to_string(),
                    healthy: false,
                    running_tasks: 0,
                    supported_task_types: executor.supported_task_types(),
                    last_health_check: chrono::Utc::now(),
                    metadata: std::collections::HashMap::new(),
                });

            infos.push(ExecutorInfo {
                name: executor.name().to_string(),
                version: executor.version().to_string(),
                description: executor.description().to_string(),
                supported_task_types: executor.supported_task_types(),
                status,
            });
        }

        Ok(infos)
    }
    pub async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>> {
        let registry = self.executors.read().await;
        let mut results = HashMap::new();

        for (name, executor) in registry.iter() {
            let is_healthy = executor.health_check().await.unwrap_or(false);
            results.insert(name.clone(), is_healthy);
        }

        Ok(results)
    }
}

impl Default for DefaultExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutorRegistry for DefaultExecutorRegistry {
    async fn register(
        &mut self,
        name: String,
        executor: Arc<dyn TaskExecutor>,
    ) -> SchedulerResult<()> {
        let mut registry = self.executors.write().await;
        registry.insert(name, executor);
        Ok(())
    }

    async fn get(&self, name: &str) -> Option<Arc<dyn TaskExecutor>> {
        let registry = self.executors.read().await;
        registry.get(name).cloned()
    }

    async fn list_executors(&self) -> Vec<String> {
        let registry = self.executors.read().await;
        registry.keys().cloned().collect()
    }

    async fn unregister(&mut self, name: &str) -> SchedulerResult<bool> {
        let mut registry = self.executors.write().await;
        Ok(registry.remove(name).is_some())
    }

    async fn clear(&mut self) {
        let mut registry = self.executors.write().await;
        registry.clear();
    }

    async fn contains(&self, name: &str) -> bool {
        let registry = self.executors.read().await;
        registry.contains_key(name)
    }

    async fn count(&self) -> usize {
        let registry = self.executors.read().await;
        registry.len()
    }

    async fn get_all_status(&self) -> SchedulerResult<HashMap<String, ExecutorStatus>> {
        let registry = self.executors.read().await;
        let mut statuses = HashMap::new();

        for (name, executor) in registry.iter() {
            match executor.get_status().await {
                Ok(status) => {
                    statuses.insert(name.clone(), status);
                }
                Err(e) => {
                    eprintln!("Failed to get status for executor '{name}': {e}");
                    statuses.insert(
                        name.clone(),
                        ExecutorStatus {
                            name: executor.name().to_string(),
                            version: executor.version().to_string(),
                            healthy: false,
                            running_tasks: 0,
                            supported_task_types: executor.supported_task_types(),
                            last_health_check: chrono::Utc::now(),
                            metadata: std::collections::HashMap::new(),
                        },
                    );
                }
            }
        }

        Ok(statuses)
    }

    async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>> {
        let registry = self.executors.read().await;
        let mut results = HashMap::new();

        for (name, executor) in registry.iter() {
            let is_healthy = executor.health_check().await.unwrap_or(false);
            results.insert(name.clone(), is_healthy);
        }

        Ok(results)
    }

    async fn get_by_task_type(
        &self,
        task_type: &str,
    ) -> SchedulerResult<Vec<Arc<dyn TaskExecutor>>> {
        let registry = self.executors.read().await;
        let mut matching_executors = Vec::new();

        for executor in registry.values() {
            if executor.supports_task_type(task_type) {
                matching_executors.push(Arc::clone(executor));
            }
        }

        Ok(matching_executors)
    }
}

#[derive(Debug, Clone)]
pub struct ExecutorInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub supported_task_types: Vec<String>,
    pub status: ExecutorStatus,
}
