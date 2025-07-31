use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::SchedulerResult;

use super::errors::SchedulerError;

// Simple logging macros for now - in production you'd use proper logging
macro_rules! error {
    ($($arg:tt)*) => {
        eprintln!("[ERROR] {}", format!($($arg)*));
    };
}

macro_rules! warn {
    ($($arg:tt)*) => {
        eprintln!("[WARN] {}", format!($($arg)*));
    };
}

macro_rules! info {
    ($($arg:tt)*) => {
        eprintln!("[INFO] {}", format!($($arg)*));
    };
}

macro_rules! debug {
    ($($arg:tt)*) => {
        eprintln!("[DEBUG] {}", format!($($arg)*));
    };
}

/// Error handling strategy - Defines how to handle different types of errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorAction {
    /// Retry the operation after a delay
    Retry { delay: Duration, max_attempts: u32 },
    /// Escalate to higher level handler
    Escalate,
    /// Log the error and continue
    LogAndContinue,
    /// Shutdown the service
    Shutdown,
}

/// Error context - Provides information about where and when error occurred
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Component where error occurred
    pub component: String,
    /// Operation that failed
    pub operation: String,
    /// Error severity level
    pub severity: ErrorSeverity,
    /// Additional context data
    pub context: HashMap<String, String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Informational - doesn't affect functionality
    Info,
    /// Warning - might affect functionality but system continues
    Warning,
    /// Error - affects functionality but system can continue
    Error,
    /// Critical - system cannot continue operating
    Critical,
}

/// Error handler trait - Defines interface for error handling
#[async_trait]
pub trait ErrorHandler: Send + Sync {
    /// Handle an error with context and return action to take
    async fn handle_error(&self, error: SchedulerError, context: ErrorContext) -> ErrorAction;

    /// Get error handling statistics
    async fn get_stats(&self) -> ErrorHandlingStats;
}

/// Error handling statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingStats {
    /// Total errors handled
    pub total_errors: u64,
    /// Errors by type
    pub errors_by_type: HashMap<String, u64>,
    /// Errors by component
    pub errors_by_component: HashMap<String, u64>,
    /// Actions taken
    pub actions_taken: HashMap<String, u64>,
    /// Average resolution time (milliseconds)
    pub avg_resolution_time_ms: f64,
}

/// Default error handler - Provides sensible default error handling
pub struct DefaultErrorHandler {
    stats: Arc<tokio::sync::RwLock<ErrorHandlingStats>>,
    retry_config: RetryConfig,
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Default retry delay
    pub default_delay: Duration,
    /// Maximum retry delay
    pub max_delay: Duration,
    /// Default maximum retry attempts
    pub default_max_attempts: u32,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            default_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            default_max_attempts: 3,
            backoff_multiplier: 2.0,
        }
    }
}

impl Default for DefaultErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultErrorHandler {
    /// Create new default error handler
    pub fn new() -> Self {
        Self {
            stats: Arc::new(tokio::sync::RwLock::new(ErrorHandlingStats {
                total_errors: 0,
                errors_by_type: HashMap::new(),
                errors_by_component: HashMap::new(),
                actions_taken: HashMap::new(),
                avg_resolution_time_ms: 0.0,
            })),
            retry_config: RetryConfig::default(),
        }
    }

    /// Create with custom retry configuration
    pub fn with_retry_config(retry_config: RetryConfig) -> Self {
        Self {
            stats: Arc::new(tokio::sync::RwLock::new(ErrorHandlingStats {
                total_errors: 0,
                errors_by_type: HashMap::new(),
                errors_by_component: HashMap::new(),
                actions_taken: HashMap::new(),
                avg_resolution_time_ms: 0.0,
            })),
            retry_config,
        }
    }

    /// Update statistics
    async fn update_stats(&self, error_type: &str, component: &str, action: &str) {
        let mut stats = self.stats.write().await;
        stats.total_errors += 1;
        *stats
            .errors_by_type
            .entry(error_type.to_string())
            .or_insert(0) += 1;
        *stats
            .errors_by_component
            .entry(component.to_string())
            .or_insert(0) += 1;
        *stats.actions_taken.entry(action.to_string()).or_insert(0) += 1;
    }

    /// Determine error action based on error type and context
    fn determine_action(&self, error: &SchedulerError, context: &ErrorContext) -> ErrorAction {
        match error {
            // Retryable errors
            SchedulerError::MessageQueue(_) | SchedulerError::Database(_) => {
                if context.severity == ErrorSeverity::Error {
                    ErrorAction::Retry {
                        delay: self.retry_config.default_delay,
                        max_attempts: self.retry_config.default_max_attempts,
                    }
                } else {
                    ErrorAction::LogAndContinue
                }
            }

            // Critical errors
            SchedulerError::Internal(msg) if msg.contains("fatal") || msg.contains("critical") => {
                ErrorAction::Shutdown
            }

            // Configuration errors
            SchedulerError::Configuration(_) => ErrorAction::LogAndContinue,

            // Task not found errors
            SchedulerError::TaskNotFound { .. } => ErrorAction::LogAndContinue,

            // Default case
            _ => match context.severity {
                ErrorSeverity::Critical => ErrorAction::Shutdown,
                ErrorSeverity::Error => ErrorAction::Escalate,
                ErrorSeverity::Warning => ErrorAction::LogAndContinue,
                ErrorSeverity::Info => ErrorAction::LogAndContinue,
            },
        }
    }
}

#[async_trait]
impl ErrorHandler for DefaultErrorHandler {
    async fn handle_error(&self, error: SchedulerError, context: ErrorContext) -> ErrorAction {
        let _start_time = std::time::Instant::now();

        // Log the error with context
        match context.severity {
            ErrorSeverity::Critical => {
                error!(
                    "Critical error in {}: {} - {}. Context: {:?}",
                    context.component, context.operation, error, context.context
                );
            }
            ErrorSeverity::Error => {
                error!(
                    "Error in {}: {} - {}",
                    context.component, context.operation, error
                );
            }
            ErrorSeverity::Warning => {
                warn!(
                    "Warning in {}: {} - {}",
                    context.component, context.operation, error
                );
            }
            ErrorSeverity::Info => {
                debug!(
                    "Info in {}: {} - {}",
                    context.component, context.operation, error
                );
            }
        }

        // Determine action
        let action = self.determine_action(&error, &context);

        // Update statistics
        let action_name = match &action {
            ErrorAction::Retry { .. } => "retry",
            ErrorAction::Escalate => "escalate",
            ErrorAction::LogAndContinue => "log_and_continue",
            ErrorAction::Shutdown => "shutdown",
        };

        self.update_stats(&error.to_string(), &context.component, action_name)
            .await;

        info!(
            "Error handled in {} operation {}: {} - {}",
            context.component, context.operation, action_name, error
        );

        action
    }

    async fn get_stats(&self) -> ErrorHandlingStats {
        self.stats.read().await.clone()
    }
}

/// Error handling middleware - Provides automatic error handling for async operations
pub struct ErrorHandlingMiddleware {
    handler: Arc<dyn ErrorHandler>,
    component: String,
}

impl ErrorHandlingMiddleware {
    /// Create new middleware for a component
    pub fn new(handler: Arc<dyn ErrorHandler>, component: String) -> Self {
        Self { handler, component }
    }

    /// Execute operation with automatic error handling
    pub async fn execute<F, Fut, T>(&self, operation: &str, f: F) -> SchedulerResult<T>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = SchedulerResult<T>> + Send,
    {
        let context = ErrorContext {
            component: self.component.clone(),
            operation: operation.to_string(),
            severity: ErrorSeverity::Error,
            context: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        match f().await {
            Ok(result) => Ok(result),
            Err(error) => {
                let action = self.handler.handle_error(error, context.clone()).await;

                match action {
                    ErrorAction::Retry {
                        delay,
                        max_attempts,
                    } => {
                        warn!(
                            "Retry requested for operation {} but operation is FnOnce - logging and continuing. Delay: {}ms, Max attempts: {}",
                            operation, delay.as_millis(), max_attempts
                        );
                        // For FnOnce operations, we can't retry the original operation
                        // In a real implementation, you'd design the operation to be retryable
                        // For now, we'll just log and return a similar error
                        Err(SchedulerError::Internal(
                            "Retry operation failed - FnOnce operation cannot be retried"
                                .to_string(),
                        ))
                    }
                    ErrorAction::Escalate => Err(SchedulerError::Internal(
                        "Error escalated to higher level".to_string(),
                    )),
                    ErrorAction::LogAndContinue => {
                        // For non-critical operations, we might want to return a default value
                        // This depends on the specific operation
                        Err(SchedulerError::Internal(
                            "Error logged and continuing".to_string(),
                        ))
                    }
                    ErrorAction::Shutdown => {
                        error!("Shutdown requested");
                        Err(SchedulerError::Internal(
                            "Service shutdown requested".to_string(),
                        ))
                    }
                }
            }
        }
    }
}

/// Convenience macro for creating error context
#[macro_export]
macro_rules! error_context {
    (component: $component:expr, operation: $operation:expr, severity: $severity:expr) => {
        ErrorContext {
            component: $component.to_string(),
            operation: $operation.to_string(),
            severity: $severity,
            context: std::collections::HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    };
    (component: $component:expr, operation: $operation:expr, severity: $severity:expr, $($key:expr => $value:expr),*) => {
        {
            let mut context = std::collections::HashMap::new();
            $(
                context.insert($key.to_string(), $value.to_string());
            )*
            ErrorContext {
                component: $component.to_string(),
                operation: $operation.to_string(),
                severity: $severity,
                context,
                timestamp: chrono::Utc::now(),
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_error_handler() {
        let handler = DefaultErrorHandler::new();

        // Test with a retryable error
        let error = SchedulerError::MessageQueue("Test error".to_string());
        let context = error_context!(
            component: "test_component",
            operation: "test_operation",
            severity: ErrorSeverity::Error
        );

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
    }

    #[tokio::test]
    async fn test_error_handling_middleware() {
        let handler = Arc::new(DefaultErrorHandler::new());
        let middleware = ErrorHandlingMiddleware::new(handler, "test_component".to_string());

        // Test successful operation
        let result = middleware
            .execute("test_operation", || async { Ok::<(), SchedulerError>(()) })
            .await;

        assert!(result.is_ok());

        // Test failed operation with retry - note that FnOnce operations cannot be retried
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

        // The operation should fail because FnOnce operations cannot be retried
        assert!(result.is_err());
        // The operation should only be called once
        assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_error_context_macro() {
        let context = error_context!(
            component: "test_component",
            operation: "test_operation",
            severity: ErrorSeverity::Error
        );

        assert_eq!(context.component, "test_component");
        assert_eq!(context.operation, "test_operation");
        assert_eq!(context.severity, ErrorSeverity::Error);

        let context_with_data = error_context!(
            component: "test_component",
            operation: "test_operation",
            severity: ErrorSeverity::Error,
            "task_id" => "123",
            "worker_id" => "worker_1"
        );

        assert_eq!(
            context_with_data.context.get("task_id"),
            Some(&"123".to_string())
        );
        assert_eq!(
            context_with_data.context.get("worker_id"),
            Some(&"worker_1".to_string())
        );
    }
}
