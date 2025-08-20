//! Timeout handling utilities for async operations
//!
//! This module provides standardized timeout handling for all async operations
//! including database calls, message queue operations, and external service calls.

use scheduler_errors::SchedulerError;
use scheduler_errors::SchedulerResult;
use std::future::Future;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, instrument};

/// Default timeout values for different operation types
pub struct TimeoutConfig {
    /// Database operations timeout
    pub database_timeout: Duration,
    /// Message queue operations timeout
    pub message_queue_timeout: Duration,
    /// External API calls timeout
    pub external_api_timeout: Duration,
    /// Internal service calls timeout
    pub internal_service_timeout: Duration,
    /// Long-running operations timeout (migrations, bulk operations)
    pub long_running_timeout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            database_timeout: Duration::from_secs(30),
            message_queue_timeout: Duration::from_secs(10),
            external_api_timeout: Duration::from_secs(15),
            internal_service_timeout: Duration::from_secs(5),
            long_running_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Timeout handler utility for async operations
pub struct TimeoutHandler {
    config: TimeoutConfig,
}

impl TimeoutHandler {
    pub fn new(config: TimeoutConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(TimeoutConfig::default())
    }

    /// Execute database operation with timeout
    #[instrument(skip(self, operation, operation_name))]
    pub async fn database_operation<F, T>(
        &self,
        operation: F,
        operation_name: &str,
    ) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        self.execute_with_timeout(
            operation,
            self.config.database_timeout,
            "数据库",
            operation_name,
        )
        .await
    }

    /// Execute message queue operation with timeout
    #[instrument(skip(self, operation, operation_name))]
    pub async fn message_queue_operation<F, T>(
        &self,
        operation: F,
        operation_name: &str,
    ) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        self.execute_with_timeout(
            operation,
            self.config.message_queue_timeout,
            "消息队列",
            operation_name,
        )
        .await
    }

    /// Execute external API call with timeout
    #[instrument(skip(self, operation, operation_name))]
    pub async fn external_api_operation<F, T>(
        &self,
        operation: F,
        operation_name: &str,
    ) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        self.execute_with_timeout(
            operation,
            self.config.external_api_timeout,
            "外部API",
            operation_name,
        )
        .await
    }

    /// Execute internal service call with timeout
    #[instrument(skip(self, operation, operation_name))]
    pub async fn internal_service_operation<F, T>(
        &self,
        operation: F,
        operation_name: &str,
    ) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        self.execute_with_timeout(
            operation,
            self.config.internal_service_timeout,
            "内部服务",
            operation_name,
        )
        .await
    }

    /// Execute long-running operation with timeout (migrations, bulk operations)
    #[instrument(skip(self, operation, operation_name))]
    pub async fn long_running_operation<F, T>(
        &self,
        operation: F,
        operation_name: &str,
    ) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        self.execute_with_timeout(
            operation,
            self.config.long_running_timeout,
            "长时间运行",
            operation_name,
        )
        .await
    }

    /// Execute operation with custom timeout
    #[instrument(skip(self, operation, operation_name))]
    pub async fn custom_timeout_operation<F, T>(
        &self,
        operation: F,
        timeout_duration: Duration,
        operation_type: &str,
        operation_name: &str,
    ) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        self.execute_with_timeout(operation, timeout_duration, operation_type, operation_name)
            .await
    }

    /// Internal method to execute operation with timeout
    async fn execute_with_timeout<F, T>(
        &self,
        operation: F,
        timeout_duration: Duration,
        operation_type: &str,
        operation_name: &str,
    ) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        match timeout(timeout_duration, operation).await {
            Ok(result) => result,
            Err(_) => {
                let error_msg = format!(
                    "{operation_type}操作 '{operation_name}' 超时 (超时时间: {timeout_duration:?})"
                );
                error!("{}", error_msg);
                Err(SchedulerError::timeout_error(error_msg))
            }
        }
    }
}

/// Convenience functions for common timeout operations
pub struct TimeoutUtils;

impl TimeoutUtils {
    /// Execute database operation with default timeout
    #[instrument(skip(operation, operation_name))]
    pub async fn database<F, T>(operation: F, operation_name: &str) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        let handler = TimeoutHandler::with_default_config();
        handler.database_operation(operation, operation_name).await
    }

    /// Execute message queue operation with default timeout
    #[instrument(skip(operation, operation_name))]
    pub async fn message_queue<F, T>(operation: F, operation_name: &str) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        let handler = TimeoutHandler::with_default_config();
        handler
            .message_queue_operation(operation, operation_name)
            .await
    }

    /// Execute external API operation with default timeout
    #[instrument(skip(operation, operation_name))]
    pub async fn external_api<F, T>(operation: F, operation_name: &str) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        let handler = TimeoutHandler::with_default_config();
        handler
            .external_api_operation(operation, operation_name)
            .await
    }

    /// Execute with custom timeout duration
    #[instrument(skip(operation, operation_name))]
    pub async fn custom<F, T>(
        operation: F,
        timeout_duration: Duration,
        operation_name: &str,
    ) -> SchedulerResult<T>
    where
        F: Future<Output = SchedulerResult<T>>,
    {
        match timeout(timeout_duration, operation).await {
            Ok(result) => result,
            Err(_) => {
                let error_msg =
                    format!("操作 '{operation_name}' 超时 (超时时间: {timeout_duration:?})");
                error!("{}", error_msg);
                Err(SchedulerError::timeout_error(error_msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_timeout_handler_success() {
        let handler = TimeoutHandler::with_default_config();

        let result = handler
            .database_operation(async { Ok("success") }, "test_operation")
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_timeout_handler_timeout() {
        let mut config = TimeoutConfig::default();
        config.database_timeout = Duration::from_millis(100);
        let handler = TimeoutHandler::new(config);

        let result = handler
            .database_operation(
                async {
                    sleep(Duration::from_millis(200)).await;
                    Ok("should_timeout")
                },
                "slow_operation",
            )
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("超时"));
    }

    #[tokio::test]
    async fn test_timeout_utils_database() {
        let result = TimeoutUtils::database(async { Ok(42) }, "test_db_op").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout_utils_custom_timeout() {
        let result =
            TimeoutUtils::custom(async { Ok("custom") }, Duration::from_secs(1), "custom_op").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "custom");
    }

    #[tokio::test]
    async fn test_timeout_utils_custom_timeout_failure() {
        let result = TimeoutUtils::custom(
            async {
                sleep(Duration::from_millis(200)).await;
                Ok("should_timeout")
            },
            Duration::from_millis(100),
            "slow_custom_op",
        )
        .await;
        assert!(result.is_err());
    }
}
