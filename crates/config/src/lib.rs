pub mod circuit_breaker;
pub mod encrypted_config_storage;
pub mod enhanced_validation;
pub mod environment;
pub mod jwt_secret_manager;
pub mod models;
pub mod secret_manager;
pub mod security;
pub mod security_policy;
pub mod validation;

// Re-export commonly used types
pub use circuit_breaker::{CircuitBreakerConfig, CircuitState};
pub use environment::{ConfigProfile, Environment, ProfileRegistry};
pub use models::{
    ApiConfig, AppConfig, DatabaseConfig, DispatcherConfig, LogConfig, LogLevel,
    MessageQueueConfig, MessageQueueType, ObservabilityConfig, OutputFormat, RateLimitingConfig,
    RedisConfig, ResilienceConfig, WorkerConfig,
};
pub use security::{
    ConfigSecurity, SecurityEvent, SecurityEventType, SecuritySeverity, SENSITIVE_PATTERNS,
};
pub use encrypted_config_storage::{EncryptedConfigStorage, EncryptionStatus};
pub use enhanced_validation::{ConfigValidator, ValidationResult, ValidationError, ValidationWarning, ValidationScore};
pub use jwt_secret_manager::{JwtSecretManager, JwtSecret, JwtRotationPolicy, RotationStatus};
pub use secret_manager::{SecretManager, SecretEntry, SecretType, RotationPolicy, SecretValidation};

/// Configuration error type
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Configuration error enumeration
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("File error: {0}")]
    File(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Environment error: {0}")]
    Environment(String),

    #[error("Security error: {0}")]
    Security(String),
}

impl From<anyhow::Error> for ConfigError {
    fn from(err: anyhow::Error) -> Self {
        ConfigError::Configuration(err.to_string())
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(err: serde_json::Error) -> Self {
        ConfigError::Parse(err.to_string())
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::Parse(err.to_string())
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError::File(err.to_string())
    }
}

impl From<url::ParseError> for ConfigError {
    fn from(err: url::ParseError) -> Self {
        ConfigError::Parse(err.to_string())
    }
}
