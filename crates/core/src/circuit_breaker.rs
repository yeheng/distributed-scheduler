use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{SchedulerError, SchedulerResult};

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Circuit is closed - normal operation
    Closed,
    /// Circuit is open - calls are blocked
    Open,
    /// Circuit is half-open - testing if service has recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: usize,
    /// Timeout for half-open state
    pub recovery_timeout: Duration,
    /// Number of successful calls in half-open before closing circuit
    pub success_threshold: usize,
    /// Maximum call timeout
    pub call_timeout: Duration,
    /// Exponential backoff multiplier for recovery timeout
    pub backoff_multiplier: f64,
    /// Maximum recovery timeout
    pub max_recovery_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 3,
            call_timeout: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            max_recovery_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    /// Current state
    pub state: CircuitState,
    /// Number of consecutive failures
    pub consecutive_failures: usize,
    /// Number of consecutive successes
    pub consecutive_successes: usize,
    /// Total calls made
    pub total_calls: u64,
    /// Total successful calls
    pub successful_calls: u64,
    /// Total failed calls
    pub failed_calls: u64,
    /// Last state change time
    pub last_state_change: Instant,
    /// Current recovery timeout
    pub current_recovery_timeout: Duration,
}

/// Circuit breaker statistics for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableCircuitBreakerStats {
    /// Current state
    pub state: CircuitState,
    /// Number of consecutive failures
    pub consecutive_failures: usize,
    /// Number of consecutive successes
    pub consecutive_successes: usize,
    /// Total calls made
    pub total_calls: u64,
    /// Total successful calls
    pub successful_calls: u64,
    /// Total failed calls
    pub failed_calls: u64,
    /// Last state change time (as timestamp since epoch)
    pub last_state_change_timestamp: i64,
    /// Current recovery timeout
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

    /// Calculate failure rate
    pub fn failure_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.failed_calls as f64 / self.total_calls as f64
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.successful_calls as f64 / self.total_calls as f64
        }
    }

    /// Convert to serializable format
    pub fn to_serializable(&self) -> SerializableCircuitBreakerStats {
        SerializableCircuitBreakerStats {
            state: self.state.clone(),
            consecutive_failures: self.consecutive_failures,
            consecutive_successes: self.consecutive_successes,
            total_calls: self.total_calls,
            successful_calls: self.successful_calls,
            failed_calls: self.failed_calls,
            last_state_change_timestamp: self.last_state_change.elapsed().as_secs() as i64,
            current_recovery_timeout: self.current_recovery_timeout,
        }
    }

    /// Create from serializable format
    pub fn from_serializable(serializable: SerializableCircuitBreakerStats) -> Self {
        Self {
            state: serializable.state,
            consecutive_failures: serializable.consecutive_failures,
            consecutive_successes: serializable.consecutive_successes,
            total_calls: serializable.total_calls,
            successful_calls: serializable.successful_calls,
            failed_calls: serializable.failed_calls,
            last_state_change: Instant::now()
                - Duration::from_secs(serializable.last_state_change_timestamp as u64),
            current_recovery_timeout: serializable.current_recovery_timeout,
        }
    }
}

/// Circuit breaker - Provides fault tolerance and service protection
/// Follows Circuit Breaker pattern to prevent cascading failures
pub struct CircuitBreaker {
    /// Circuit breaker configuration
    config: CircuitBreakerConfig,
    /// Current statistics
    stats: Arc<RwLock<CircuitBreakerStats>>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreaker {
    /// Create new circuit breaker with default configuration
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create new circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        let stats = CircuitBreakerStats::new(&config);
        Self {
            config,
            stats: Arc::new(RwLock::new(stats)),
        }
    }

    /// Execute operation with circuit breaker protection
    pub async fn execute<F, Fut, T>(&self, operation: F) -> SchedulerResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = SchedulerResult<T>>,
    {
        // Check if we should allow the call
        if !self.should_allow_call().await {
            return Err(SchedulerError::Internal(
                "Circuit breaker is open - calls are blocked".to_string(),
            ));
        }

        // Execute the operation with timeout
        let result = tokio::time::timeout(self.config.call_timeout, operation()).await;

        match result {
            Ok(Ok(result)) => {
                self.record_success().await;
                Ok(result)
            }
            Ok(Err(error)) => {
                self.record_failure().await;
                Err(error)
            }
            Err(_) => {
                self.record_failure().await;
                Err(SchedulerError::Internal("Operation timed out".to_string()))
            }
        }
    }

    /// Check if call should be allowed based on circuit state
    async fn should_allow_call(&self) -> bool {
        let stats = self.stats.read().await;

        match stats.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if recovery timeout has passed
                if Instant::now().duration_since(stats.last_state_change)
                    > stats.current_recovery_timeout
                {
                    drop(stats);
                    self.transition_to_half_open().await;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record successful call
    async fn record_success(&self) {
        let mut stats = self.stats.write().await;

        stats.total_calls += 1;
        stats.successful_calls += 1;
        stats.consecutive_successes += 1;
        stats.consecutive_failures = 0;

        // Check if we should close the circuit
        if stats.state == CircuitState::HalfOpen
            && stats.consecutive_successes >= self.config.success_threshold
        {
            stats.state = CircuitState::Closed;
            stats.last_state_change = Instant::now();
            stats.current_recovery_timeout = self.config.recovery_timeout; // Reset timeout
        }
    }

    /// Record failed call
    async fn record_failure(&self) {
        let mut stats = self.stats.write().await;

        stats.total_calls += 1;
        stats.failed_calls += 1;
        stats.consecutive_failures += 1;
        stats.consecutive_successes = 0;

        // Check if we should open the circuit
        if stats.state == CircuitState::Closed
            && stats.consecutive_failures >= self.config.failure_threshold
        {
            stats.state = CircuitState::Open;
            stats.last_state_change = Instant::now();
            stats.current_recovery_timeout = self.config.recovery_timeout;
        } else if stats.state == CircuitState::HalfOpen {
            // In half-open state, any failure immediately opens the circuit
            stats.state = CircuitState::Open;
            stats.last_state_change = Instant::now();
            // Increase recovery timeout with backoff
            stats.current_recovery_timeout = std::cmp::min(
                Duration::from_millis(
                    (stats.current_recovery_timeout.as_millis() as f64
                        * self.config.backoff_multiplier) as u64,
                ),
                self.config.max_recovery_timeout,
            );
        }
    }

    /// Transition to half-open state
    async fn transition_to_half_open(&self) {
        let mut stats = self.stats.write().await;
        stats.state = CircuitState::HalfOpen;
        stats.last_state_change = Instant::now();
        stats.consecutive_successes = 0;
    }

    /// Get current circuit state
    pub async fn get_state(&self) -> CircuitState {
        self.stats.read().await.state.clone()
    }

    /// Get circuit breaker statistics
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        self.stats.read().await.clone()
    }

    /// Reset circuit breaker to closed state
    pub async fn reset(&self) {
        let mut stats = self.stats.write().await;
        *stats = CircuitBreakerStats::new(&self.config);
    }

    /// Force open circuit (for testing or maintenance)
    pub async fn force_open(&self) {
        let mut stats = self.stats.write().await;
        stats.state = CircuitState::Open;
        stats.last_state_change = Instant::now();
    }

    /// Force close circuit (for testing or recovery)
    pub async fn force_close(&self) {
        let mut stats = self.stats.write().await;
        stats.state = CircuitState::Closed;
        stats.last_state_change = Instant::now();
        stats.consecutive_failures = 0;
        stats.current_recovery_timeout = self.config.recovery_timeout;
    }
}

/// Circuit breaker middleware - Provides automatic circuit breaking for operations
pub struct CircuitBreakerMiddleware {
    circuit_breaker: Arc<CircuitBreaker>,
    component_name: String,
}

impl CircuitBreakerMiddleware {
    /// Create new circuit breaker middleware
    pub fn new(circuit_breaker: Arc<CircuitBreaker>, component_name: String) -> Self {
        Self {
            circuit_breaker,
            component_name,
        }
    }

    /// Execute operation with circuit breaker protection
    pub async fn execute<F, Fut, T>(&self, operation_name: &str, operation: F) -> SchedulerResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = SchedulerResult<T>>,
    {
        match self.circuit_breaker.execute(operation).await {
            Ok(result) => Ok(result),
            Err(error) => {
                // Enhance error with circuit breaker context
                let state = self.circuit_breaker.get_state().await;
                let enhanced_error = SchedulerError::Internal(format!(
                    "Circuit breaker error in {} operation {}: {}. State: {:?}",
                    self.component_name, operation_name, error, state
                ));
                Err(enhanced_error)
            }
        }
    }

    /// Get circuit breaker statistics
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        self.circuit_breaker.get_stats().await
    }

    /// Reset circuit breaker
    pub async fn reset(&self) -> SchedulerResult<()> {
        self.circuit_breaker.reset().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let cb = CircuitBreaker::new();

        // Initially in closed state
        assert_eq!(cb.get_state().await, CircuitState::Closed);

        // Successful call should keep it closed
        let result = cb.execute(|| async { Ok::<(), SchedulerError>(()) }).await;
        assert!(result.is_ok());
        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_open_state() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Fail 3 times to open circuit
        for _ in 0..3 {
            let result: SchedulerResult<()> = cb
                .execute(|| async { Err(SchedulerError::Internal("Test error".to_string())) })
                .await;
            assert!(result.is_err());
        }

        // Circuit should be open
        assert_eq!(cb.get_state().await, CircuitState::Open);

        // Further calls should be blocked
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

        // Fail 2 times to open circuit
        for _ in 0..2 {
            let _: SchedulerResult<()> = cb
                .execute(|| async { Err(SchedulerError::Internal("Test error".to_string())) })
                .await;
        }

        assert_eq!(cb.get_state().await, CircuitState::Open);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Successful calls should close the circuit
        for _ in 0..2 {
            let result = cb.execute(|| async { Ok::<(), SchedulerError>(()) }).await;
            assert!(result.is_ok());
        }

        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_timeout() {
        let config = CircuitBreakerConfig {
            call_timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Operation that times out
        let result = cb
            .execute(|| async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<(), SchedulerError>(())
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));

        // Should count as failure
        let stats = cb.get_stats().await;
        assert_eq!(stats.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_middleware() {
        let cb = Arc::new(CircuitBreaker::new());
        let middleware = CircuitBreakerMiddleware::new(cb, "test_component".to_string());

        // Successful operation
        let result = middleware
            .execute("test_operation", || async {
                Ok::<String, SchedulerError>("success".to_string())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        // Failed operation
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

    #[tokio::test]
    async fn test_circuit_breaker_exponential_backoff() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            max_recovery_timeout: Duration::from_millis(400),
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // First failure
        let _: SchedulerResult<()> = cb
            .execute(|| async { Err(SchedulerError::Internal("Test error".to_string())) })
            .await;

        assert_eq!(cb.get_state().await, CircuitState::Open);
        let stats1 = cb.get_stats().await;
        assert_eq!(stats1.current_recovery_timeout, Duration::from_millis(100));

        // Wait and fail again in half-open
        tokio::time::sleep(Duration::from_millis(150)).await;
        let _: SchedulerResult<()> = cb
            .execute(|| async { Err(SchedulerError::Internal("Test error".to_string())) })
            .await;

        let stats2 = cb.get_stats().await;
        assert_eq!(stats2.current_recovery_timeout, Duration::from_millis(200)); // 100 * 2

        // Wait and fail again
        tokio::time::sleep(Duration::from_millis(250)).await;
        let _: SchedulerResult<()> = cb
            .execute(|| async { Err(SchedulerError::Internal("Test error".to_string())) })
            .await;

        let stats3 = cb.get_stats().await;
        assert_eq!(stats3.current_recovery_timeout, Duration::from_millis(400)); // 200 * 2

        // Should be capped at max
        tokio::time::sleep(Duration::from_millis(450)).await;
        let _: SchedulerResult<()> = cb
            .execute(|| async { Err(SchedulerError::Internal("Test error".to_string())) })
            .await;

        let stats4 = cb.get_stats().await;
        assert_eq!(stats4.current_recovery_timeout, Duration::from_millis(400));
        // Still 400 (max)
    }
}
