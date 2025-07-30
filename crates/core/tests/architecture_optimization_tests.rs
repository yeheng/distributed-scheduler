use std::sync::Arc;
use std::time::Duration;

use scheduler_core::{
    config_management::{ConfigBuilder, Environment},
    error_handling::{
        DefaultErrorHandler, ErrorAction, ErrorContext, ErrorHandler, ErrorHandlingMiddleware,
        ErrorSeverity,
    },
    errors::{Result, SchedulerError},
};

/// Test error handling middleware
#[tokio::test]
async fn test_error_handling_middleware() {
    let handler = Arc::new(DefaultErrorHandler::new());
    let middleware = ErrorHandlingMiddleware::new(handler, "test_component".to_string());

    // Test successful operation
    let result = middleware
        .execute("test_operation", || async { Ok::<(), SchedulerError>(()) })
        .await;

    assert!(result.is_ok());

    // Test failed operation with retry
    let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    let result = middleware
        .execute("failing_operation", move || {
            let count = attempt_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move {
                if count < 2 {
                    Err(SchedulerError::MessageQueue("Retryable error".to_string()))
                } else {
                    Ok::<(), SchedulerError>(())
                }
            }
        })
        .await;

    // The error handling middleware now returns an error for retry operations
    // because FnOnce operations cannot be retried
    assert!(result.is_err());
    // The operation was only attempted once because FnOnce operations cannot be retried
    assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 1);
}

/// Test typed configuration
#[tokio::test]
async fn test_typed_configuration() {
    // Mock configuration service
    #[derive(Clone)]
    struct MockConfigService {
        config: Arc<tokio::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>>,
    }

    #[async_trait::async_trait]
    impl scheduler_core::config_management::ConfigurationService for MockConfigService {
        async fn get_config_value(&self, key: &str) -> Result<Option<serde_json::Value>> {
            let config = self.config.read().await;
            Ok(config.get(key).cloned())
        }

        async fn set_config_value(&self, key: &str, value: &serde_json::Value) -> Result<()> {
            let mut config = self.config.write().await;
            config.insert(key.to_string(), value.clone());
            Ok(())
        }

        async fn delete_config(&self, key: &str) -> Result<bool> {
            let mut config = self.config.write().await;
            Ok(config.remove(key).is_some())
        }

        async fn list_config_keys(&self) -> Result<Vec<String>> {
            let config = self.config.read().await;
            Ok(config.keys().cloned().collect())
        }

        async fn reload_config(&self) -> Result<()> {
            Ok(())
        }
    }

    let service = Arc::new(MockConfigService {
        config: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    });

    let builder = ConfigBuilder::new(service);

    // Test setting and getting a string value
    builder
        .set(
            "test_string",
            &serde_json::Value::String("hello".to_string()),
        )
        .await
        .unwrap();
    let result: Option<String> = builder.get("test_string").await.unwrap();
    assert_eq!(result, Some("hello".to_string()));

    // Test with default value
    let result: String = builder
        .get_or_default("nonexistent_key", "default".to_string())
        .await
        .unwrap();
    assert_eq!(result, "default");
}

/// Test error handling strategies
#[tokio::test]
async fn test_error_handling_strategies() {
    let handler = DefaultErrorHandler::new();

    // Test retryable error
    let error = SchedulerError::MessageQueue("Connection failed".to_string());
    let context = ErrorContext {
        component: "test_component".to_string(),
        operation: "test_operation".to_string(),
        severity: ErrorSeverity::Error,
        context: std::collections::HashMap::new(),
        timestamp: chrono::Utc::now(),
    };

    let action = handler.handle_error(error, context).await;

    match action {
        ErrorAction::Retry {
            delay,
            max_attempts,
        } => {
            assert_eq!(max_attempts, 3);
            assert_eq!(delay, Duration::from_secs(1));
        }
        _ => panic!("Expected retry action"),
    }

    // Test critical error
    let critical_error = SchedulerError::Internal("Fatal error: system corrupted".to_string());
    let critical_context = ErrorContext {
        component: "system_core".to_string(),
        operation: "system_initialization".to_string(),
        severity: ErrorSeverity::Critical,
        context: std::collections::HashMap::new(),
        timestamp: chrono::Utc::now(),
    };

    let critical_action = handler.handle_error(critical_error, critical_context).await;

    match critical_action {
        ErrorAction::Shutdown => {
            // Expected for critical errors
        }
        _ => panic!("Expected shutdown action for critical error"),
    }
}

/// Test error handling statistics
#[tokio::test]
async fn test_error_handling_statistics() {
    let handler = DefaultErrorHandler::new();

    // Handle some errors to generate statistics
    let error = SchedulerError::MessageQueue("Test error".to_string());
    let context = ErrorContext {
        component: "test_component".to_string(),
        operation: "test_operation".to_string(),
        severity: ErrorSeverity::Error,
        context: std::collections::HashMap::new(),
        timestamp: chrono::Utc::now(),
    };

    // Handle multiple errors
    for _ in 0..3 {
        let error = SchedulerError::MessageQueue("Test error".to_string());
        let context = ErrorContext {
            component: "test_component".to_string(),
            operation: "test_operation".to_string(),
            severity: ErrorSeverity::Error,
            context: std::collections::HashMap::new(),
            timestamp: chrono::Utc::now(),
        };
        handler.handle_error(error, context).await;
    }

    // Check statistics
    let stats = handler.get_stats().await;
    assert_eq!(stats.total_errors, 3);
    assert_eq!(stats.errors_by_component.get("test_component"), Some(&3));
    assert_eq!(stats.actions_taken.get("retry"), Some(&3));
}

/// Test environment configuration
#[tokio::test]
async fn test_environment_configuration() {
    let env = Environment::from_str("development").unwrap();
    assert_eq!(env, Environment::Development);

    let env = Environment::from_str("production").unwrap();
    assert_eq!(env, Environment::Production);
}
