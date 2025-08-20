//! Circuit breaker implementation for resilience
//!
//! This module provides a circuit breaker implementation that uses configuration
//! from the scheduler-config crate to improve system resilience.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use scheduler_config::{CircuitBreakerConfig, CircuitState};
use scheduler_errors::SchedulerError;
use scheduler_errors::SchedulerResult;

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub consecutive_failures: usize,
    pub consecutive_successes: usize,
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub last_state_change: Instant,
    pub current_recovery_timeout: Duration,
}

impl CircuitBreakerStats {
    pub fn new(config: &CircuitBreakerConfig) -> Self {
        let now = Instant::now();
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            total_calls: 0,
            successful_calls: 0,
            failed_calls: 0,
            last_state_change: now,
            current_recovery_timeout: config.recovery_timeout,
        }
    }

    pub fn failure_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.failed_calls as f64 / self.total_calls as f64
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.successful_calls as f64 / self.total_calls as f64
        }
    }
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    stats: Arc<RwLock<CircuitBreakerStats>>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        let stats = CircuitBreakerStats::new(&config);
        Self {
            config,
            stats: Arc::new(RwLock::new(stats)),
        }
    }

    pub async fn execute<F, Fut, T>(&self, operation: F) -> SchedulerResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = SchedulerResult<T>>,
    {
        if !self.should_allow_call().await? {
            return Err(SchedulerError::Internal(
                "Circuit breaker is open - calls are blocked".to_string(),
            ));
        }

        let result = tokio::time::timeout(self.config.call_timeout, operation()).await;

        match result {
            Ok(Ok(result)) => {
                self.record_success().await?;
                Ok(result)
            }
            Ok(Err(error)) => {
                self.record_failure().await?;
                Err(error)
            }
            Err(_) => {
                self.record_failure().await?;
                Err(SchedulerError::Internal("Operation timed out".to_string()))
            }
        }
    }

    async fn should_allow_call(&self) -> SchedulerResult<bool> {
        let stats = self.stats.read().await;

        let result = match stats.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if Instant::now().duration_since(stats.last_state_change)
                    > stats.current_recovery_timeout
                {
                    drop(stats);
                    self.transition_to_half_open().await?;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        };
        Ok(result)
    }

    async fn record_success(&self) -> SchedulerResult<()> {
        let mut stats = self.stats.write().await;

        stats.total_calls += 1;
        stats.successful_calls += 1;
        stats.consecutive_successes += 1;
        stats.consecutive_failures = 0;

        if stats.state == CircuitState::HalfOpen
            && stats.consecutive_successes >= self.config.success_threshold
        {
            stats.state = CircuitState::Closed;
            stats.last_state_change = Instant::now();
            stats.current_recovery_timeout = self.config.recovery_timeout;
        }

        Ok(())
    }

    async fn record_failure(&self) -> SchedulerResult<()> {
        let mut stats = self.stats.write().await;

        stats.total_calls += 1;
        stats.failed_calls += 1;
        stats.consecutive_failures += 1;
        stats.consecutive_successes = 0;

        if stats.state == CircuitState::Closed
            && stats.consecutive_failures >= self.config.failure_threshold
        {
            stats.state = CircuitState::Open;
            stats.last_state_change = Instant::now();
            stats.current_recovery_timeout = self.config.recovery_timeout;
        } else if stats.state == CircuitState::HalfOpen {
            stats.state = CircuitState::Open;
            stats.last_state_change = Instant::now();
            stats.current_recovery_timeout = std::cmp::min(
                Duration::from_millis(
                    (stats.current_recovery_timeout.as_millis() as f64
                        * self.config.backoff_multiplier) as u64,
                ),
                self.config.max_recovery_timeout,
            );
        }

        Ok(())
    }

    async fn transition_to_half_open(&self) -> SchedulerResult<()> {
        let mut stats = self.stats.write().await;
        stats.state = CircuitState::HalfOpen;
        stats.last_state_change = Instant::now();
        stats.consecutive_successes = 0;
        Ok(())
    }

    pub async fn get_state(&self) -> SchedulerResult<CircuitState> {
        Ok(self.stats.read().await.state.clone())
    }

    pub async fn get_stats(&self) -> SchedulerResult<CircuitBreakerStats> {
        Ok(self.stats.read().await.clone())
    }

    pub async fn reset(&self) -> SchedulerResult<()> {
        let mut stats = self.stats.write().await;
        *stats = CircuitBreakerStats::new(&self.config);
        Ok(())
    }

    pub async fn force_open(&self) -> SchedulerResult<()> {
        let mut stats = self.stats.write().await;
        stats.state = CircuitState::Open;
        stats.last_state_change = Instant::now();
        Ok(())
    }

    pub async fn force_close(&self) -> SchedulerResult<()> {
        let mut stats = self.stats.write().await;
        stats.state = CircuitState::Closed;
        stats.last_state_change = Instant::now();
        stats.consecutive_failures = 0;
        stats.current_recovery_timeout = self.config.recovery_timeout;
        Ok(())
    }
}

impl Clone for CircuitBreaker {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            stats: Arc::clone(&self.stats),
        }
    }
}

/// Circuit breaker middleware for enhanced error reporting
pub struct CircuitBreakerMiddleware {
    circuit_breaker: Arc<CircuitBreaker>,
    component_name: String,
}

impl CircuitBreakerMiddleware {
    pub fn new(circuit_breaker: Arc<CircuitBreaker>, component_name: String) -> Self {
        Self {
            circuit_breaker,
            component_name,
        }
    }

    pub async fn execute<F, Fut, T>(&self, operation_name: &str, operation: F) -> SchedulerResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = SchedulerResult<T>>,
    {
        match self.circuit_breaker.execute(operation).await {
            Ok(result) => Ok(result),
            Err(error) => {
                let state = self.circuit_breaker.get_state().await;
                let enhanced_error = SchedulerError::Internal(format!(
                    "Circuit breaker error in {} operation {}: {}. State: {:?}",
                    self.component_name, operation_name, error, state
                ));
                Err(enhanced_error)
            }
        }
    }

    pub async fn get_stats(&self) -> SchedulerResult<CircuitBreakerStats> {
        self.circuit_breaker.get_stats().await
    }

    pub async fn reset(&self) -> SchedulerResult<()> {
        self.circuit_breaker.reset().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let cb = CircuitBreaker::new();
        assert_eq!(cb.get_state().await.unwrap(), CircuitState::Closed);

        let result = cb.execute(|| async { Ok::<(), SchedulerError>(()) }).await;
        assert!(result.is_ok());
        assert_eq!(cb.get_state().await.unwrap(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_open_state() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        for _ in 0..3 {
            let result: SchedulerResult<()> = cb
                .execute(|| async { Err(SchedulerError::Internal("Test error".to_string())) })
                .await;
            assert!(result.is_err());
        }

        assert_eq!(cb.get_state().await.unwrap(), CircuitState::Open);

        let result = cb.execute(|| async { Ok::<(), SchedulerError>(()) }).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circuit breaker is open"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            success_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        for _ in 0..2 {
            let _: SchedulerResult<()> = cb
                .execute(|| async { Err(SchedulerError::Internal("Test error".to_string())) })
                .await;
        }

        assert_eq!(cb.get_state().await.unwrap(), CircuitState::Open);
        tokio::time::sleep(Duration::from_millis(150)).await;

        for _ in 0..2 {
            let result = cb.execute(|| async { Ok::<(), SchedulerError>(()) }).await;
            assert!(result.is_ok());
        }

        assert_eq!(cb.get_state().await.unwrap(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_middleware() {
        let cb = Arc::new(CircuitBreaker::new());
        let middleware = CircuitBreakerMiddleware::new(cb, "test_component".to_string());

        let result = middleware
            .execute("test_operation", || async {
                Ok::<String, SchedulerError>("success".to_string())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        let result: SchedulerResult<String> = middleware
            .execute("test_operation", || async {
                Err(SchedulerError::Internal("Test error".to_string()))
            })
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circuit breaker error"));
    }
}
