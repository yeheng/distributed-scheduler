use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use scheduler_config::models::ExecutorConfig;
use scheduler_foundation::{
    traits::{ExecutorRegistry, TaskExecutor},
    SchedulerError, SchedulerResult,
};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::executors::{HttpExecutor, MockTaskExecutor, ShellExecutor};

/// Factory for creating executors based on configuration
pub struct ExecutorFactory {
    config: ExecutorConfig,
    registry: Arc<RwLock<HashMap<String, Arc<dyn TaskExecutor>>>>,
}

impl ExecutorFactory {
    /// Create a new executor factory from configuration
    pub fn new(config: ExecutorConfig) -> Self {
        Self {
            config,
            registry: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize executors based on configuration
    pub async fn initialize(&self) -> SchedulerResult<()> {
        if !self.config.enabled {
            info!("Executor system is disabled");
            return Ok(());
        }

        info!("Initializing {} executors", self.config.executors.len());

        let mut registry = self.registry.write().await;

        for (name, executor_config) in &self.config.executors {
            info!("Creating executor '{}' of type '{}'", name, executor_config.executor_type);
            
            let executor = self.create_executor(name, executor_config).await
                .with_context(|| format!("Failed to create executor '{}'", name))?;
            
            registry.insert(name.to_string(), executor);
            
            info!("Executor '{}' initialized successfully", name);
        }

        // Validate that default executor exists
        if let Some(ref default_executor) = self.config.default_executor {
            if !registry.contains_key(default_executor) {
                return Err(SchedulerError::config_error(format!(
                    "Default executor '{}' not found",
                    default_executor
                )));
            }
        }

        info!("All executors initialized successfully");
        Ok(())
    }

    /// Create a single executor based on its configuration
    async fn create_executor(
        &self,
        name: &str,
        config: &scheduler_config::models::ExecutorInstanceConfig,
    ) -> SchedulerResult<Arc<dyn TaskExecutor>> {
        match config.executor_type.as_str() {
            "shell" => {
                let executor = ShellExecutor::new();
                info!("Created Shell executor: {}", name);
                Ok(Arc::new(executor))
            }
            "http" => {
                let executor = HttpExecutor::new();
                info!("Created HTTP executor: {}", name);
                Ok(Arc::new(executor))
            }
            "mock" => {
                let should_succeed = config.config
                    .as_ref()
                    .and_then(|c| c.get("should_succeed").and_then(|v| v.as_bool()))
                    .unwrap_or(true);
                
                let execution_time_ms = config.config
                    .as_ref()
                    .and_then(|c| c.get("execution_time_ms").and_then(|v| v.as_u64()))
                    .unwrap_or(100);
                
                let executor = MockTaskExecutor::new(name.to_string(), should_succeed, execution_time_ms);
                info!("Created Mock executor: {} (success={}, time={}ms)", name, should_succeed, execution_time_ms);
                Ok(Arc::new(executor))
            }
            executor_type => {
                Err(SchedulerError::config_error(format!(
                    "Unknown executor type: {}",
                    executor_type
                )))
            }
        }
    }

    /// Get an executor by name
    pub async fn get_executor(&self, name: &str) -> Option<Arc<dyn TaskExecutor>> {
        let registry = self.registry.read().await;
        registry.get(name).cloned()
    }

    /// Get the default executor
    pub async fn get_default_executor(&self) -> Option<Arc<dyn TaskExecutor>> {
        if let Some(ref default_name) = self.config.default_executor {
            self.get_executor(default_name).await
        } else {
            warn!("No default executor configured");
            None
        }
    }

    /// Get executor by task type
    pub async fn get_executor_by_task_type(&self, task_type: &str) -> Option<Arc<dyn TaskExecutor>> {
        let registry = self.registry.read().await;
        
        // Find executors that support the given task type
        for (name, executor_config) in &self.config.executors {
            if executor_config.supported_task_types.contains(&task_type.to_string()) {
                if let Some(executor) = registry.get(name) {
                    return Some(Arc::clone(executor));
                }
            }
        }
        
        None
    }

    /// Get all executor names
    pub async fn list_executors(&self) -> Vec<String> {
        let registry = self.registry.read().await;
        registry.keys().cloned().collect()
    }

    /// Get executor configuration
    pub fn get_config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Reload configuration and reinitialize executors
    pub async fn reload_config(&mut self, new_config: ExecutorConfig) -> SchedulerResult<()> {
        info!("Reloading executor configuration");
        
        // Clear existing executors
        {
            let mut registry = self.registry.write().await;
            registry.clear();
        }
        
        // Update configuration
        self.config = new_config;
        
        // Reinitialize executors
        self.initialize().await?;
        
        info!("Executor configuration reloaded successfully");
        Ok(())
    }

    /// Get executor status for all executors
    pub async fn get_all_executor_status(&self) -> SchedulerResult<HashMap<String, scheduler_foundation::traits::ExecutorStatus>> {
        let registry = self.registry.read().await;
        let mut statuses = HashMap::new();

        for (name, executor) in registry.iter() {
            match executor.get_status().await {
                Ok(status) => {
                    statuses.insert(name.clone(), status);
                }
                Err(e) => {
                    error!("Failed to get status for executor '{}': {}", name, e);
                    statuses.insert(
                        name.clone(),
                        scheduler_foundation::traits::ExecutorStatus {
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

    /// Perform health check on all executors
    pub async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>> {
        let registry = self.registry.read().await;
        let mut results = HashMap::new();

        for (name, executor) in registry.iter() {
            let is_healthy = match executor.health_check().await {
                Ok(healthy) => healthy,
                Err(e) => {
                    error!("Health check failed for executor '{}': {}", name, e);
                    false
                }
            };
            results.insert(name.clone(), is_healthy);
        }

        Ok(results)
    }

    /// Warm up all executors
    pub async fn warm_up_all(&self) -> SchedulerResult<()> {
        let registry = self.registry.read().await;
        
        for (name, executor) in registry.iter() {
            if let Err(e) = executor.warm_up().await {
                warn!("Warm up failed for executor '{}': {}", name, e);
            } else {
                info!("Executor '{}' warmed up successfully", name);
            }
        }
        
        Ok(())
    }

    /// Clean up all executors
    pub async fn cleanup_all(&self) -> SchedulerResult<()> {
        let registry = self.registry.read().await;
        
        for (name, executor) in registry.iter() {
            if let Err(e) = executor.cleanup().await {
                error!("Cleanup failed for executor '{}': {}", name, e);
            } else {
                info!("Executor '{}' cleaned up successfully", name);
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl ExecutorRegistry for ExecutorFactory {
    async fn register(
        &mut self,
        name: String,
        executor: Arc<dyn TaskExecutor>,
    ) -> SchedulerResult<()> {
        let mut registry = self.registry.write().await;
        registry.insert(name, executor);
        Ok(())
    }

    async fn get(&self, name: &str) -> Option<Arc<dyn TaskExecutor>> {
        self.get_executor(name).await
    }

    async fn list_executors(&self) -> Vec<String> {
        self.list_executors().await
    }

    async fn unregister(&mut self, name: &str) -> SchedulerResult<bool> {
        let mut registry = self.registry.write().await;
        Ok(registry.remove(name).is_some())
    }

    async fn clear(&mut self) {
        let mut registry = self.registry.write().await;
        registry.clear();
    }

    async fn contains(&self, name: &str) -> bool {
        let registry = self.registry.read().await;
        registry.contains_key(name)
    }

    async fn count(&self) -> usize {
        let registry = self.registry.read().await;
        registry.len()
    }

    async fn get_all_status(&self) -> SchedulerResult<HashMap<String, scheduler_foundation::traits::ExecutorStatus>> {
        self.get_all_executor_status().await
    }

    async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>> {
        self.health_check_all().await
    }

    async fn get_by_task_type(
        &self,
        task_type: &str,
    ) -> SchedulerResult<Vec<Arc<dyn TaskExecutor>>> {
        let mut matching_executors = Vec::new();
        
        if let Some(executor) = self.get_executor_by_task_type(task_type).await {
            matching_executors.push(executor);
        }
        
        Ok(matching_executors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_config::models::{ExecutorInstanceConfig, RetryConfig};

    #[tokio::test]
    async fn test_executor_factory_creation() {
        let config = ExecutorConfig::default();
        let factory = ExecutorFactory::new(config);
        
        assert_eq!(factory.list_executors().await.len(), 0);
    }

    #[tokio::test]
    async fn test_executor_factory_initialization() {
        let config = ExecutorConfig::default();
        let factory = ExecutorFactory::new(config);
        
        let result = factory.initialize().await;
        assert!(result.is_ok());
        
        assert_eq!(factory.list_executors().await.len(), 2);
        assert!(factory.contains("shell").await);
        assert!(factory.contains("http").await);
    }

    #[tokio::test]
    async fn test_get_executor() {
        let config = ExecutorConfig::default();
        let factory = ExecutorFactory::new(config);
        factory.initialize().await.unwrap();
        
        let shell_executor = factory.get_executor("shell").await;
        assert!(shell_executor.is_some());
        
        let nonexistent = factory.get_executor("nonexistent").await;
        assert!(nonexistent.is_none());
    }

    #[tokio::test]
    async fn test_get_default_executor() {
        let config = ExecutorConfig::default();
        let factory = ExecutorFactory::new(config);
        factory.initialize().await.unwrap();
        
        let default_executor = factory.get_default_executor().await;
        assert!(default_executor.is_some());
        
        let executor = default_executor.unwrap();
        assert_eq!(executor.name(), "shell");
    }

    #[tokio::test]
    async fn test_get_executor_by_task_type() {
        let config = ExecutorConfig::default();
        let factory = ExecutorFactory::new(config);
        factory.initialize().await.unwrap();
        
        let shell_executor = factory.get_executor_by_task_type("shell").await;
        assert!(shell_executor.is_some());
        
        let http_executor = factory.get_executor_by_task_type("http").await;
        assert!(http_executor.is_some());
        
        let nonexistent = factory.get_executor_by_task_type("nonexistent").await;
        assert!(nonexistent.is_none());
    }

    #[tokio::test]
    async fn test_executor_factory_with_mock() {
        let mut config = ExecutorConfig::default();
        
        config.executors.insert(
            "test_mock".to_string(),
            ExecutorInstanceConfig {
                executor_type: "mock".to_string(),
                config: Some(serde_json::json!({
                    "should_succeed": true,
                    "execution_time_ms": 50
                })),
                supported_task_types: vec!["test".to_string()],
                priority: 50,
                max_concurrent_tasks: Some(5),
                timeout_seconds: Some(30),
                retry_config: Some(RetryConfig::default()),
            },
        );
        
        let factory = ExecutorFactory::new(config);
        factory.initialize().await.unwrap();
        
        let mock_executor = factory.get_executor("test_mock").await;
        assert!(mock_executor.is_some());
        
        let executor = mock_executor.unwrap();
        assert_eq!(executor.name(), "test_mock");
        assert_eq!(executor.supported_task_types(), vec!["test_mock".to_string()]);
    }
}