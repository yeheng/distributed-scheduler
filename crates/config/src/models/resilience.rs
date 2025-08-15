use crate::circuit_breaker::CircuitBreakerConfig;
use crate::validation::ConfigValidator;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceConfig {
    pub database_circuit_breaker: CircuitBreakerConfig,
    pub message_queue_circuit_breaker: CircuitBreakerConfig,
    pub external_service_circuit_breaker: CircuitBreakerConfig,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            database_circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 5,
                recovery_timeout: std::time::Duration::from_secs(60),
                success_threshold: 3,
                call_timeout: std::time::Duration::from_secs(30),
                backoff_multiplier: 2.0,
                max_recovery_timeout: std::time::Duration::from_secs(300),
            },
            message_queue_circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 3,
                recovery_timeout: std::time::Duration::from_secs(30),
                success_threshold: 2,
                call_timeout: std::time::Duration::from_secs(10),
                backoff_multiplier: 1.5,
                max_recovery_timeout: std::time::Duration::from_secs(180),
            },
            external_service_circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 3,
                recovery_timeout: std::time::Duration::from_secs(45),
                success_threshold: 3,
                call_timeout: std::time::Duration::from_secs(15),
                backoff_multiplier: 2.0,
                max_recovery_timeout: std::time::Duration::from_secs(240),
            },
        }
    }
}

impl ConfigValidator for ResilienceConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        self.database_circuit_breaker.validate()?;
        self.message_queue_circuit_breaker.validate()?;
        self.external_service_circuit_breaker.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resilience_config_default() {
        let config = ResilienceConfig::default();

        assert_eq!(config.database_circuit_breaker.failure_threshold, 5);
        assert_eq!(
            config.database_circuit_breaker.recovery_timeout,
            std::time::Duration::from_secs(60)
        );

        assert_eq!(config.message_queue_circuit_breaker.failure_threshold, 3);
        assert_eq!(
            config.message_queue_circuit_breaker.recovery_timeout,
            std::time::Duration::from_secs(30)
        );

        assert_eq!(config.external_service_circuit_breaker.failure_threshold, 3);
        assert_eq!(
            config.external_service_circuit_breaker.recovery_timeout,
            std::time::Duration::from_secs(45)
        );
    }

    #[test]
    fn test_resilience_config_validation() {
        let config = ResilienceConfig::default();
        assert!(config.validate().is_ok());

        // Test with invalid database circuit breaker
        let mut invalid_config = config.clone();
        invalid_config.database_circuit_breaker.failure_threshold = 0;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_resilience_config_serialization() {
        let config = ResilienceConfig::default();

        // Test that the config can be serialized and deserialized
        let serialized = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: ResilienceConfig =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(
            config.database_circuit_breaker.failure_threshold,
            deserialized.database_circuit_breaker.failure_threshold
        );
        assert_eq!(
            config.message_queue_circuit_breaker.failure_threshold,
            deserialized.message_queue_circuit_breaker.failure_threshold
        );
    }
}
