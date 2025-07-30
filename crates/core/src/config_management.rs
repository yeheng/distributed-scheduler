use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use super::errors::{Result, SchedulerError};

/// Type-safe configuration wrapper
/// Provides compile-time type safety for configuration values
pub struct TypedConfig<T> {
    service: Arc<dyn ConfigurationService>,
    key: String,
    _phantom: PhantomData<T>,
}

impl<T> TypedConfig<T>
where
    T: Serialize + DeserializeOwned + Send + Sync,
{
    /// Create new typed configuration wrapper
    pub fn new(service: Arc<dyn ConfigurationService>, key: String) -> Self {
        Self {
            service,
            key,
            _phantom: PhantomData,
        }
    }

    /// Get configuration value with type safety
    pub async fn get(&self) -> Result<Option<T>> {
        match self.service.get_config_value(&self.key).await? {
            Some(value) => {
                let typed_value: T = serde_json::from_value(value).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to deserialize config '{}': {}",
                        self.key, e
                    ))
                })?;
                Ok(Some(typed_value))
            }
            None => Ok(None),
        }
    }

    /// Get configuration value with default
    pub async fn get_or_default(&self, default: T) -> Result<T> {
        match self.get().await? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }

    /// Set configuration value with type safety
    pub async fn set(&self, value: &T) -> Result<()> {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!(
                "Failed to serialize config '{}': {}",
                self.key, e
            ))
        })?;
        self.service.set_config_value(&self.key, &json_value).await
    }

    /// Get configuration key
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Configuration service trait
#[async_trait]
pub trait ConfigurationService: Send + Sync {
    async fn get_config_value(&self, key: &str) -> Result<Option<Value>>;
    async fn set_config_value(&self, key: &str, value: &Value) -> Result<()>;
    async fn delete_config(&self, key: &str) -> Result<bool>;
    async fn list_config_keys(&self) -> Result<Vec<String>>;
    async fn reload_config(&self) -> Result<()>;
}

/// Configuration builder - Fluent interface for configuration management
pub struct ConfigBuilder {
    service: Arc<dyn ConfigurationService>,
}

impl ConfigBuilder {
    /// Create new configuration builder
    pub fn new(service: Arc<dyn ConfigurationService>) -> Self {
        Self { service }
    }

    /// Create typed configuration for a specific key
    pub fn typed<T>(&self, key: &str) -> TypedConfig<T>
    where
        T: Serialize + DeserializeOwned + Send + Sync,
    {
        TypedConfig::new(Arc::clone(&self.service), key.to_string())
    }

    /// Get configuration value
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send + Sync,
    {
        // Direct implementation to avoid type constraints
        match self.service.get_config_value(key).await? {
            Some(value) => {
                let typed_value: T = serde_json::from_value(value).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to deserialize config '{key}': {e}"
                    ))
                })?;
                Ok(Some(typed_value))
            }
            None => Ok(None),
        }
    }

    /// Get configuration value with default
    pub async fn get_or_default<T>(&self, key: &str, default: T) -> Result<T>
    where
        T: DeserializeOwned + Send + Sync,
    {
        match self.get::<T>(key).await? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }

    /// Set configuration value
    pub async fn set<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!("Failed to serialize config '{key}': {e}"))
        })?;
        self.service.set_config_value(key, &json_value).await
    }

    /// Delete configuration
    pub async fn delete(&self, key: &str) -> Result<bool> {
        self.service.delete_config(key).await
    }

    /// List all configuration keys
    pub async fn list_keys(&self) -> Result<Vec<String>> {
        self.service.list_config_keys().await
    }

    /// Reload configuration
    pub async fn reload(&self) -> Result<()> {
        self.service.reload_config().await
    }
}

/// Environment-specific configuration
#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

impl Environment {
    /// Parse environment from string
    pub fn from_str(env: &str) -> Result<Self> {
        match env.to_lowercase().as_str() {
            "development" | "dev" => Ok(Environment::Development),
            "testing" | "test" => Ok(Environment::Testing),
            "staging" | "stage" => Ok(Environment::Staging),
            "production" | "prod" => Ok(Environment::Production),
            _ => Err(SchedulerError::Configuration(format!(
                "Invalid environment: {env}"
            ))),
        }
    }

    /// Get current environment from environment variable
    pub fn current() -> Result<Self> {
        std::env::var("APP_ENV")
            .map(|s| Self::from_str(&s))
            .unwrap_or(Ok(Environment::Development))
    }
}

impl Environment {
    fn _as_str(&self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Testing => "testing",
            Environment::Staging => "staging",
            Environment::Production => "production",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde::Deserialize;
    use std::collections::HashMap;

    // Mock configuration service for testing
    struct MockConfigService {
        config: std::sync::RwLock<HashMap<String, Value>>,
    }

    impl MockConfigService {
        fn new() -> Self {
            Self {
                config: std::sync::RwLock::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl ConfigurationService for MockConfigService {
        async fn get_config_value(&self, key: &str) -> Result<Option<Value>> {
            let config = self.config.read().unwrap();
            Ok(config.get(key).cloned())
        }

        async fn set_config_value(&self, key: &str, value: &Value) -> Result<()> {
            let mut config = self.config.write().unwrap();
            config.insert(key.to_string(), value.clone());
            Ok(())
        }

        async fn delete_config(&self, key: &str) -> Result<bool> {
            let mut config = self.config.write().unwrap();
            Ok(config.remove(key).is_some())
        }

        async fn list_config_keys(&self) -> Result<Vec<String>> {
            let config = self.config.read().unwrap();
            Ok(config.keys().cloned().collect())
        }

        async fn reload_config(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_typed_config() {
        let service = Arc::new(MockConfigService::new());
        let typed_config = TypedConfig::<String>::new(service, "test_key".to_string());

        // Test get with default
        let result = typed_config
            .get_or_default("default".to_string())
            .await
            .unwrap();
        assert_eq!(result, "default");

        // Test set and get
        typed_config.set(&"test_value".to_string()).await.unwrap();
        let result = typed_config.get().await.unwrap();
        assert_eq!(result, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_config_builder() {
        let service = Arc::new(MockConfigService::new());
        let builder = ConfigBuilder::new(service);

        // Test fluent interface
        builder
            .set("test_key", &Value::String("test_value".to_string()))
            .await
            .unwrap();
        let result: Option<String> = builder.get("test_key").await.unwrap();
        assert_eq!(result, Some("test_value".to_string()));
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct DatabaseConfig {
        url: String,
        max_connections: u32,
    }

    #[tokio::test]
    async fn test_complex_config_type() {
        let service = Arc::new(MockConfigService::new());
        let typed_config = TypedConfig::<DatabaseConfig>::new(service, "database".to_string());

        let config = DatabaseConfig {
            url: "postgresql://localhost/test".to_string(),
            max_connections: 10,
        };

        typed_config.set(&config).await.unwrap();
        let result = typed_config.get().await.unwrap();

        assert_eq!(result, Some(config));
    }
}
