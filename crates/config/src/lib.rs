// 新的模块结构（推荐使用）
pub mod core;
pub mod security_new;
pub mod validation_new;

// 现有模块（保持向后兼容）
pub mod circuit_breaker;
pub mod environment;
pub mod models;
pub mod security;

// Re-export commonly used types

// 新的简化API（推荐使用）
pub use core::constants;
pub use security_new::{Encryptor, SimpleSecretManager, SecretType, SecretStatus};
pub use validation_new::{BasicConfigValidator, ValidationUtils, ConfigValidator};

// 现有API（保持向后兼容）
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
