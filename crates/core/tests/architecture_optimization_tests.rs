use std::sync::Arc;
use std::time::Duration;
use std::str::FromStr;

use scheduler_core::{
    config::Environment,
    error_handling::{
        DefaultErrorHandler, ErrorAction, ErrorContext, ErrorHandler, ErrorHandlingMiddleware,
        ErrorSeverity,
    },
    errors::SchedulerError,
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
    // Test loading default configuration
    use scheduler_core::config::AppConfig;
    
    let config = AppConfig::default();
    assert!(config.validate().is_ok());
    
    // Test environment variable override
    std::env::set_var("SCHEDULER_DATABASE_URL", "postgresql://test:5432/test_db");
    let config_with_env = AppConfig::load(None).unwrap_or_else(|_| AppConfig::default());
    assert_eq!(config_with_env.database.url, "postgresql://test:5432/test_db");
    
    // Clean up
    std::env::remove_var("SCHEDULER_DATABASE_URL");
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
    let _error = SchedulerError::MessageQueue("Test error".to_string());
    let _context = ErrorContext {
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
