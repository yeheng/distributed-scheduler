use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::{ConfigError, ConfigResult};

mod duration_serde {
    use std::time::Duration;
    use serde::{Deserialize, Deserializer, Serializer, Serialize};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: usize,
    #[serde(with = "duration_serde")]
    pub recovery_timeout: Duration,
    pub success_threshold: usize,
    #[serde(with = "duration_serde")]
    pub call_timeout: Duration,
    pub backoff_multiplier: f64,
    #[serde(with = "duration_serde")]
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

impl CircuitBreakerConfig {
    pub fn validate(&self) -> ConfigResult<()> {
        if self.failure_threshold == 0 {
            return Err(ConfigError::Validation(
                "failure_threshold must be greater than 0".to_string(),
            ));
        }
        
        if self.success_threshold == 0 {
            return Err(ConfigError::Validation(
                "success_threshold must be greater than 0".to_string(),
            ));
        }
        
        if self.backoff_multiplier <= 1.0 {
            return Err(ConfigError::Validation(
                "backoff_multiplier must be greater than 1.0".to_string(),
            ));
        }
        
        if self.recovery_timeout.is_zero() {
            return Err(ConfigError::Validation(
                "recovery_timeout must be greater than 0".to_string(),
            ));
        }
        
        if self.call_timeout.is_zero() {
            return Err(ConfigError::Validation(
                "call_timeout must be greater than 0".to_string(),
            ));
        }
        
        if self.max_recovery_timeout.is_zero() {
            return Err(ConfigError::Validation(
                "max_recovery_timeout must be greater than 0".to_string(),
            ));
        }
        
        if self.recovery_timeout > self.max_recovery_timeout {
            return Err(ConfigError::Validation(
                "recovery_timeout must be less than or equal to max_recovery_timeout".to_string(),
            ));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout, Duration::from_secs(60));
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.call_timeout, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.max_recovery_timeout, Duration::from_secs(300));
    }

    #[test]
    fn test_circuit_breaker_config_validation() {
        let config = CircuitBreakerConfig::default();
        assert!(config.validate().is_ok());

        // Test invalid failure_threshold
        let mut invalid_config = config.clone();
        invalid_config.failure_threshold = 0;
        assert!(invalid_config.validate().is_err());

        // Test invalid success_threshold
        let mut invalid_config = config.clone();
        invalid_config.success_threshold = 0;
        assert!(invalid_config.validate().is_err());

        // Test invalid backoff_multiplier
        let mut invalid_config = config.clone();
        invalid_config.backoff_multiplier = 1.0;
        assert!(invalid_config.validate().is_err());

        // Test invalid recovery_timeout
        let mut invalid_config = config.clone();
        invalid_config.recovery_timeout = Duration::from_secs(0);
        assert!(invalid_config.validate().is_err());

        // Test recovery_timeout > max_recovery_timeout
        let mut invalid_config = config.clone();
        invalid_config.recovery_timeout = Duration::from_secs(400);
        invalid_config.max_recovery_timeout = Duration::from_secs(300);
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_circuit_breaker_config_serialization() {
        let config = CircuitBreakerConfig::default();
        let serialized = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: CircuitBreakerConfig = serde_json::from_str(&serialized).expect("Failed to deserialize");
        
        assert_eq!(config.failure_threshold, deserialized.failure_threshold);
        assert_eq!(config.recovery_timeout, deserialized.recovery_timeout);
        assert_eq!(config.success_threshold, deserialized.success_threshold);
    }
}