//! Type-safe configuration wrapper
//!
//! This module provides a type-safe wrapper for configuration values.

use serde_json::Value;
use std::sync::Arc;

use super::UnifiedConfigManager;
use crate::{errors::SchedulerError, SchedulerResult};

/// Type-safe configuration wrapper
#[derive(Debug)]
pub struct TypedConfig<T> {
    manager: Arc<UnifiedConfigManager>,
    key: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TypedConfig<T>
where
    T: serde::de::DeserializeOwned + serde::Serialize + Send + Sync + Clone,
{
    /// Create new typed configuration wrapper
    pub fn new(manager: Arc<UnifiedConfigManager>, key: String) -> Self {
        Self {
            manager,
            key,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get configuration value with type safety
    pub async fn get(&self) -> SchedulerResult<Option<T>> {
        match self.manager.get(&self.key).await {
            Ok(value) => Ok(Some(value)),
            Err(SchedulerError::Configuration(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get configuration value with default
    pub async fn get_or_default(&self, default: T) -> SchedulerResult<T> {
        Ok(self.manager.get_or_default(&self.key, default).await)
    }

    /// Set configuration value with type safety
    pub async fn set(&self, value: &T) -> SchedulerResult<()> {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!(
                "Failed to serialize config '{}': {}",
                self.key, e
            ))
        })?;
        self.manager.set(&self.key, json_value).await
    }

    /// Get configuration key
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Check if configuration value exists
    pub async fn exists(&self) -> SchedulerResult<bool> {
        match self.manager.get::<Value>(&self.key).await {
            Ok(_) => Ok(true),
            Err(SchedulerError::Configuration(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

/// Builder for creating typed configuration wrappers
pub struct TypedConfigBuilder<T> {
    manager: Option<Arc<UnifiedConfigManager>>,
    key: Option<String>,
    default: Option<T>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TypedConfigBuilder<T>
where
    T: serde::de::DeserializeOwned + serde::Serialize + Send + Sync + Clone,
{
    /// Create a new typed config builder
    pub fn new() -> Self {
        Self {
            manager: None,
            key: None,
            default: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Set the configuration manager
    pub fn manager(mut self, manager: Arc<UnifiedConfigManager>) -> Self {
        self.manager = Some(manager);
        self
    }

    /// Set the configuration key
    pub fn key(mut self, key: String) -> Self {
        self.key = Some(key);
        self
    }

    /// Set the default value
    pub fn default_value(mut self, default: T) -> Self {
        self.default = Some(default);
        self
    }

    /// Build the typed configuration
    pub fn build(self) -> Result<TypedConfig<T>, String> {
        let manager = self.manager.ok_or("Configuration manager is required")?;
        let key = self.key.ok_or("Configuration key is required")?;

        Ok(TypedConfig::new(manager, key))
    }
}

impl<T> Default for TypedConfigBuilder<T>
where
    T: serde::de::DeserializeOwned + serde::Serialize + Send + Sync + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::manager::sources::ConfigSource;
    use serde_json::json;

    #[tokio::test]
    async fn test_typed_config() {
        let manager = Arc::new(
            UnifiedConfigManager::new().add_source(ConfigSource::Memory {
                data: json!({"database": {"url": "postgresql://localhost/test"}}),
            }),
        );

        manager.load().await.unwrap();

        let typed_config = TypedConfig::<String>::new(manager, "database.url".to_string());
        let result = typed_config.get().await.unwrap();

        assert!(result.is_some());
        if let Some(value) = result {
            assert_eq!(value, "postgresql://localhost/test");
        }
    }

    #[tokio::test]
    async fn test_typed_config_with_default() {
        let manager = Arc::new(
            UnifiedConfigManager::new().add_source(ConfigSource::Memory {
                data: json!({"app": {"name": "test"}}),
            }),
        );

        manager.load().await.unwrap();

        let typed_config = TypedConfig::<u32>::new(manager, "app.port".to_string());
        let result = typed_config.get_or_default(8080).await.unwrap();

        assert_eq!(result, 8080);
    }

    #[tokio::test]
    async fn test_typed_config_set() {
        let manager = Arc::new(
            UnifiedConfigManager::new().add_source(ConfigSource::Memory { data: json!({}) }),
        );

        manager.load().await.unwrap();

        let typed_config = TypedConfig::<String>::new(manager, "test.key".to_string());
        typed_config.set(&"test_value".to_string()).await.unwrap();

        let result = typed_config.get().await.unwrap();
        assert_eq!(result, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_typed_config_exists() {
        let manager = Arc::new(
            UnifiedConfigManager::new().add_source(ConfigSource::Memory {
                data: json!({"existing": "value"}),
            }),
        );

        manager.load().await.unwrap();

        let typed_config = TypedConfig::<String>::new(manager.clone(), "existing".to_string());
        assert!(typed_config.exists().await.unwrap());

        let typed_config = TypedConfig::<String>::new(manager, "non_existing".to_string());
        assert!(!typed_config.exists().await.unwrap());
    }

    #[test]
    fn test_typed_config_builder() {
        let manager = Arc::new(UnifiedConfigManager::new());
        let typed_config = TypedConfigBuilder::<String>::new()
            .manager(manager)
            .key("test.key".to_string())
            .build();

        assert!(typed_config.is_ok());
    }

    #[test]
    fn test_typed_config_builder_missing_manager() {
        let result = TypedConfigBuilder::<String>::new()
            .key("test.key".to_string())
            .build();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Configuration manager is required");
    }

    #[test]
    fn test_typed_config_builder_missing_key() {
        let manager = Arc::new(UnifiedConfigManager::new());
        let result = TypedConfigBuilder::<String>::new().manager(manager).build();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Configuration key is required");
    }
}
