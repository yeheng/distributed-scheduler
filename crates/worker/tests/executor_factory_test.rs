#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use scheduler_config::models::{ExecutorConfig, ExecutorInstanceConfig, ExecutorFactoryConfig};
    use scheduler_worker::ExecutorFactory;
    use scheduler_foundation::traits::ExecutorRegistry;

    #[tokio::test]
    async fn test_executor_factory_creation() {
        let config = ExecutorConfig::default();

        let _factory = ExecutorFactory::new(config);
        // Test that we can create an executor factory without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_executor_factory_disabled() {
        let mut config = ExecutorConfig::default();
        config.enabled = false;

        let factory = ExecutorFactory::new(config);
        let result = factory.initialize().await;
        
        // Should return Ok when disabled
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_executor_factory_with_custom_config() {
        let mut executors = HashMap::new();
        executors.insert(
            "shell".to_string(),
            ExecutorInstanceConfig {
                executor_type: "shell".to_string(),
                config: None,
                supported_task_types: vec!["shell".to_string()],
                priority: 100,
                max_concurrent_tasks: Some(5),
                timeout_seconds: Some(300),
                retry_config: None,
            },
        );

        let config = ExecutorConfig {
            enabled: true,
            executors,
            default_executor: Some("shell".to_string()),
            executor_factory: ExecutorFactoryConfig::default(),
        };

        let factory = ExecutorFactory::new(config);
        let result = factory.initialize().await;
        
        // Should initialize successfully
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_executor_factory_list_executors() {
        let config = ExecutorConfig::default();
        let factory = Arc::new(ExecutorFactory::new(config));
        factory.initialize().await.unwrap();
        
        let executor_list = factory.list_executors().await;
        assert!(!executor_list.is_empty());
        assert!(executor_list.contains(&"shell".to_string()));
    }

    #[tokio::test]
    async fn test_executor_factory_contains() {
        let config = ExecutorConfig::default();
        let factory = Arc::new(ExecutorFactory::new(config));
        factory.initialize().await.unwrap();
        
        assert!(factory.contains("shell").await);
        assert!(factory.contains("http").await);
        assert!(!factory.contains("nonexistent").await);
    }

    #[tokio::test]
    async fn test_executor_factory_get_executor() {
        let config = ExecutorConfig::default();
        let factory = Arc::new(ExecutorFactory::new(config));
        factory.initialize().await.unwrap();
        
        let executor = factory.get_executor("shell").await;
        assert!(executor.is_some());
        
        let non_existent = factory.get_executor("nonexistent").await;
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_executor_factory_default_config() {
        let config = ExecutorConfig::default();
        let _factory = ExecutorFactory::new(config);
        
        // Test that the factory was created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_executor_factory_multiple_executors() {
        let config = ExecutorConfig::default();
        let factory = Arc::new(ExecutorFactory::new(config));
        factory.initialize().await.unwrap();
        
        let executor_list = factory.list_executors().await;
        assert!(executor_list.len() >= 2);
        assert!(executor_list.contains(&"shell".to_string()));
        assert!(executor_list.contains(&"http".to_string()));
    }
}