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
            let is_healthy = match executor.health_check().await {
                Ok(healthy) => healthy,
                Err(e) => {
                    eprintln!("Health check failed for executor '{name}': {e}");
                    false
                }
            };
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
        if let Some(executor) = registry.remove(name) {
            // Call cleanup on the executor before removing it
            if let Err(e) = executor.cleanup().await {
                eprintln!("Warning: Cleanup failed for executor '{name}': {e}");
                // Continue with unregistration even if cleanup fails
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn clear(&mut self) {
        let mut registry = self.executors.write().await;

        // Call cleanup on all executors before clearing
        for (name, executor) in registry.drain() {
            if let Err(e) = executor.cleanup().await {
                eprintln!("Warning: Cleanup failed for executor '{name}': {e}");
                // Continue with clearing even if cleanup fails
            }
        }
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
            let is_healthy = match executor.health_check().await {
                Ok(healthy) => healthy,
                Err(e) => {
                    eprintln!("Health check failed for executor '{name}': {e}");
                    false
                }
            };
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

impl DefaultExecutorRegistry {
    /// Gracefully shutdown all executors by calling their cleanup methods
    pub async fn shutdown(&mut self) -> SchedulerResult<()> {
        let registry = self.executors.read().await;
        let executor_names: Vec<String> = registry.keys().cloned().collect();
        drop(registry); // Release read lock

        println!(
            "Shutting down {} executors gracefully...",
            executor_names.len()
        );

        for name in executor_names {
            let mut registry = self.executors.write().await;
            if let Some(executor) = registry.remove(&name) {
                drop(registry); // Release write lock during cleanup

                println!("Cleaning up executor: {name}");
                if let Err(e) = executor.cleanup().await {
                    eprintln!("Warning: Cleanup failed for executor '{name}': {e}");
                }
            }
        }

        println!("All executors have been shut down");
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    // Mock executor for testing
    #[derive(Debug, Clone)]
    struct MockTaskExecutor {
        name: String,
        version: String,
        description: String,
        supported_types: Vec<String>,
        is_healthy: bool,
    }

    impl MockTaskExecutor {
        fn new(name: &str, version: &str, description: &str, supported_types: Vec<String>) -> Self {
            Self {
                name: name.to_string(),
                version: version.to_string(),
                description: description.to_string(),
                supported_types,
                is_healthy: true,
            }
        }

        fn set_healthy(&mut self, healthy: bool) {
            self.is_healthy = healthy;
        }
    }

    #[async_trait]
    impl TaskExecutor for MockTaskExecutor {
        async fn execute_task(&self, _context: &crate::TaskExecutionContext) -> SchedulerResult<crate::TaskResult> {
            Ok(crate::TaskResult {
                success: true,
                output: Some("Mock execution completed".to_string()),
                error_message: None,
                exit_code: Some(0),
                execution_time_ms: 100,
            })
        }

        fn supports_task_type(&self, task_type: &str) -> bool {
            self.supported_types.contains(&task_type.to_string())
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> &str {
            &self.version
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn supported_task_types(&self) -> Vec<String> {
            self.supported_types.clone()
        }

        async fn cancel(&self, _task_run_id: i64) -> SchedulerResult<()> {
            Ok(())
        }

        async fn is_running(&self, _task_run_id: i64) -> SchedulerResult<bool> {
            Ok(false)
        }

        async fn get_status(&self) -> SchedulerResult<ExecutorStatus> {
            Ok(ExecutorStatus {
                name: self.name.clone(),
                version: self.version.clone(),
                healthy: self.is_healthy,
                running_tasks: 0,
                supported_task_types: self.supported_types.clone(),
                last_health_check: Utc::now(),
                metadata: HashMap::new(),
            })
        }

        async fn health_check(&self) -> SchedulerResult<bool> {
            Ok(self.is_healthy)
        }

        async fn warm_up(&self) -> SchedulerResult<()> {
            Ok(())
        }

        async fn cleanup(&self) -> SchedulerResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_default_executor_registry_creation() {
        let registry = DefaultExecutorRegistry::new();
        assert_eq!(registry.count().await, 0);
        assert!(registry.list_executors().await.is_empty());
    }

    #[tokio::test]
    async fn test_register_executor() {
        let mut registry = DefaultExecutorRegistry::new();
        let executor = Arc::new(MockTaskExecutor::new(
            "test_executor",
            "1.0.0",
            "Test executor",
            vec!["test_type".to_string()],
        ));

        let result = registry.register("test_executor".to_string(), executor).await;
        assert!(result.is_ok());
        assert_eq!(registry.count().await, 1);
        assert!(registry.contains("test_executor").await);
    }

    #[tokio::test]
    async fn test_get_executor() {
        let mut registry = DefaultExecutorRegistry::new();
        let executor = Arc::new(MockTaskExecutor::new(
            "test_executor",
            "1.0.0",
            "Test executor",
            vec!["test_type".to_string()],
        ));

        registry.register("test_executor".to_string(), executor.clone()).await.unwrap();

        let retrieved = registry.get("test_executor").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "test_executor");

        let non_existent = registry.get("non_existent").await;
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_unregister_executor() {
        let mut registry = DefaultExecutorRegistry::new();
        let executor = Arc::new(MockTaskExecutor::new(
            "test_executor",
            "1.0.0",
            "Test executor",
            vec!["test_type".to_string()],
        ));

        registry.register("test_executor".to_string(), executor).await.unwrap();
        assert_eq!(registry.count().await, 1);

        let result = registry.unregister("test_executor").await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(registry.count().await, 0);
        assert!(!registry.contains("test_executor").await);
    }

    #[tokio::test]
    async fn test_unregister_nonexistent_executor() {
        let mut registry = DefaultExecutorRegistry::new();
        let result = registry.unregister("non_existent").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_clear_executors() {
        let mut registry = DefaultExecutorRegistry::new();
        
        // Add multiple executors
        for i in 0..3 {
            let executor = Arc::new(MockTaskExecutor::new(
                &format!("executor_{}", i),
                "1.0.0",
                "Test executor",
                vec!["test_type".to_string()],
            ));
            registry.register(format!("executor_{}", i), executor).await.unwrap();
        }

        assert_eq!(registry.count().await, 3);
        
        registry.clear().await;
        assert_eq!(registry.count().await, 0);
        assert!(registry.list_executors().await.is_empty());
    }

    #[tokio::test]
    async fn test_list_executors() {
        let mut registry = DefaultExecutorRegistry::new();
        
        // Add executors in specific order
        let names = vec!["executor_a", "executor_b", "executor_c"];
        for name in &names {
            let executor = Arc::new(MockTaskExecutor::new(
                name,
                "1.0.0",
                "Test executor",
                vec!["test_type".to_string()],
            ));
            registry.register(name.to_string(), executor).await.unwrap();
        }

        let listed = registry.list_executors().await;
        assert_eq!(listed.len(), 3);
        assert!(listed.contains(&"executor_a".to_string()));
        assert!(listed.contains(&"executor_b".to_string()));
        assert!(listed.contains(&"executor_c".to_string()));
    }

    #[tokio::test]
    async fn test_get_all_status() {
        let mut registry = DefaultExecutorRegistry::new();
        
        let executor1 = Arc::new(MockTaskExecutor::new(
            "executor1",
            "1.0.0",
            "Test executor 1",
            vec!["type1".to_string()],
        ));
        
        let executor2 = Arc::new(MockTaskExecutor::new(
            "executor2",
            "2.0.0",
            "Test executor 2",
            vec!["type2".to_string()],
        ));

        registry.register("executor1".to_string(), executor1).await.unwrap();
        registry.register("executor2".to_string(), executor2).await.unwrap();

        let statuses = registry.get_all_status().await.unwrap();
        assert_eq!(statuses.len(), 2);
        assert!(statuses.contains_key("executor1"));
        assert!(statuses.contains_key("executor2"));
    }

    #[tokio::test]
    async fn test_health_check_all() {
        let mut registry = DefaultExecutorRegistry::new();
        
        let healthy_executor = Arc::new(MockTaskExecutor::new(
            "healthy",
            "1.0.0",
            "Healthy executor",
            vec!["test_type".to_string()],
        ));
        
        let unhealthy_executor = Arc::new(MockTaskExecutor::new(
            "unhealthy",
            "1.0.0",
            "Unhealthy executor",
            vec!["test_type".to_string()],
        ));

        registry.register("healthy".to_string(), healthy_executor).await.unwrap();
        registry.register("unhealthy".to_string(), unhealthy_executor).await.unwrap();

        let health_results = registry.health_check_all().await.unwrap();
        assert_eq!(health_results.len(), 2);
        assert!(health_results.get("healthy").unwrap());
        assert!(health_results.get("unhealthy").unwrap());
    }

    #[tokio::test]
    async fn test_get_by_task_type() {
        let mut registry = DefaultExecutorRegistry::new();
        
        let executor1 = Arc::new(MockTaskExecutor::new(
            "executor1",
            "1.0.0",
            "Test executor 1",
            vec!["type1".to_string(), "common".to_string()],
        ));
        
        let executor2 = Arc::new(MockTaskExecutor::new(
            "executor2",
            "1.0.0",
            "Test executor 2",
            vec!["type2".to_string(), "common".to_string()],
        ));

        let executor3 = Arc::new(MockTaskExecutor::new(
            "executor3",
            "1.0.0",
            "Test executor 3",
            vec!["type3".to_string()],
        ));

        registry.register("executor1".to_string(), executor1).await.unwrap();
        registry.register("executor2".to_string(), executor2).await.unwrap();
        registry.register("executor3".to_string(), executor3).await.unwrap();

        // Test common type (should match 2 executors)
        let common_executors = registry.get_by_task_type("common").await.unwrap();
        assert_eq!(common_executors.len(), 2);

        // Test specific type (should match 1 executor)
        let type1_executors = registry.get_by_task_type("type1").await.unwrap();
        assert_eq!(type1_executors.len(), 1);

        // Test non-existent type (should match 0 executors)
        let non_existent = registry.get_by_task_type("non_existent").await.unwrap();
        assert_eq!(non_existent.len(), 0);
    }

    #[tokio::test]
    async fn test_register_batch() {
        let mut registry = DefaultExecutorRegistry::new();
        
        let executors = vec![
            ("batch1".to_string(), Arc::new(MockTaskExecutor::new(
                "batch1",
                "1.0.0",
                "Batch executor 1",
                vec!["test_type".to_string()],
            )) as Arc<dyn TaskExecutor>),
            ("batch2".to_string(), Arc::new(MockTaskExecutor::new(
                "batch2",
                "1.0.0",
                "Batch executor 2",
                vec!["test_type".to_string()],
            )) as Arc<dyn TaskExecutor>),
            ("batch3".to_string(), Arc::new(MockTaskExecutor::new(
                "batch3",
                "1.0.0",
                "Batch executor 3",
                vec!["test_type".to_string()],
            )) as Arc<dyn TaskExecutor>),
        ];

        let result = registry.register_batch(executors).await;
        assert!(result.is_ok());
        assert_eq!(registry.count().await, 3);
        assert!(registry.contains("batch1").await);
        assert!(registry.contains("batch2").await);
        assert!(registry.contains("batch3").await);
    }

    #[tokio::test]
    async fn test_get_executor_info() {
        let mut registry = DefaultExecutorRegistry::new();
        let executor = Arc::new(MockTaskExecutor::new(
            "test_executor",
            "1.0.0",
            "Test executor for info",
            vec!["test_type".to_string()],
        ));

        registry.register("test_executor".to_string(), executor).await.unwrap();

        let info = registry.get_executor_info("test_executor").await.unwrap();
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.name, "test_executor");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.description, "Test executor for info");
        assert_eq!(info.supported_task_types, vec!["test_type".to_string()]);

        let non_existent = registry.get_executor_info("non_existent").await.unwrap();
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_get_all_executor_info() {
        let mut registry = DefaultExecutorRegistry::new();
        
        for i in 0..3 {
            let executor = Arc::new(MockTaskExecutor::new(
                &format!("executor_{}", i),
                "1.0.0",
                &format!("Test executor {}", i),
                vec!["test_type".to_string()],
            ));
            registry.register(format!("executor_{}", i), executor).await.unwrap();
        }

        let infos = registry.get_all_executor_info().await.unwrap();
        assert_eq!(infos.len(), 3);
        
        // Check that all executors are represented
        let names: Vec<String> = infos.iter().map(|info| info.name.clone()).collect();
        assert!(names.contains(&"executor_0".to_string()));
        assert!(names.contains(&"executor_1".to_string()));
        assert!(names.contains(&"executor_2".to_string()));
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut registry = DefaultExecutorRegistry::new();
        
        // Add some executors
        for i in 0..3 {
            let executor = Arc::new(MockTaskExecutor::new(
                &format!("executor_{}", i),
                "1.0.0",
                "Test executor",
                vec!["test_type".to_string()],
            ));
            registry.register(format!("executor_{}", i), executor).await.unwrap();
        }

        assert_eq!(registry.count().await, 3);
        
        // Shutdown should remove all executors
        let result = registry.shutdown().await;
        assert!(result.is_ok());
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_default_trait_implementation() {
        let registry = DefaultExecutorRegistry::default();
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_executor_info_clone() {
        let status = ExecutorStatus {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            healthy: true,
            running_tasks: 1,
            supported_task_types: vec!["test".to_string()],
            last_health_check: Utc::now(),
            metadata: HashMap::new(),
        };

        let info = ExecutorInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            supported_task_types: vec!["test".to_string()],
            status: status.clone(),
        };

        let cloned_info = info.clone();
        assert_eq!(info.name, cloned_info.name);
        assert_eq!(info.version, cloned_info.version);
        assert_eq!(info.status.healthy, cloned_info.status.healthy);
    }

    #[tokio::test]
    async fn test_error_handling_in_get_all_status() {
        let mut registry = DefaultExecutorRegistry::new();
        
        // Create a mock executor that is unhealthy
        let mut failing_executor = MockTaskExecutor::new(
            "failing",
            "1.0.0",
            "Failing executor",
            vec!["test_type".to_string()],
        );
        failing_executor.set_healthy(false);
        
        let failing_executor = Arc::new(failing_executor);

        registry.register("failing".to_string(), failing_executor).await.unwrap();

        // Should return unhealthy status
        let statuses = registry.get_all_status().await.unwrap();
        assert_eq!(statuses.len(), 1);
        assert!(!statuses.get("failing").unwrap().healthy);
    }
}
