#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use scheduler_foundation::{
        prelude::{ApplicationContext, DefaultExecutorRegistry},
        traits::WorkerServiceTrait,
        ServiceLocator,
    };

    use crate::{WorkerConfig, WorkerService, WorkerServiceBuilder};

    #[tokio::test]
    async fn test_worker_config_builder() {
        let config = WorkerConfig::builder(
            "test_worker".to_string(),
            "test_tasks".to_string(),
            "test_status".to_string(),
        )
        .max_concurrent_tasks(10)
        .heartbeat_interval_seconds(60)
        .poll_interval_ms(500)
        .build();

        assert_eq!(config.worker_id, "test_worker");
        assert_eq!(config.task_queue, "test_tasks");
        assert_eq!(config.status_queue, "test_status");
        assert_eq!(config.max_concurrent_tasks, 10);
        assert_eq!(config.heartbeat_interval_seconds, 60);
        assert_eq!(config.poll_interval_ms, 500);
    }

    #[tokio::test]
    async fn test_worker_service_builder() {
        let context = Arc::new(ApplicationContext::default());
        let service_locator = Arc::new(ServiceLocator::new(context));
        let executor_registry = Arc::new(DefaultExecutorRegistry::new());

        let worker_service = WorkerServiceBuilder::new(
            "test_worker".to_string(),
            service_locator,
            "test_tasks".to_string(),
            "test_status".to_string(),
        )
        .with_executor_registry(executor_registry)
        .max_concurrent_tasks(5)
        .build()
        .await;

        assert!(worker_service.is_ok());
    }

    #[tokio::test]
    async fn test_worker_service_creation() {
        let context = Arc::new(ApplicationContext::default());
        let service_locator = Arc::new(ServiceLocator::new(context));
        let executor_registry = Arc::new(DefaultExecutorRegistry::new());

        let config = WorkerConfig::builder(
            "test_worker".to_string(),
            "test_tasks".to_string(),
            "test_status".to_string(),
        )
        .build();

        let worker_service = WorkerService::new(config, service_locator, executor_registry);

        // Test basic functionality
        let task_count = worker_service.get_current_task_count().await;
        assert_eq!(task_count, 0);

        let supported_types = worker_service.get_supported_task_types().await;
        assert!(supported_types.is_empty()); // Default registry has no executors
    }
}
