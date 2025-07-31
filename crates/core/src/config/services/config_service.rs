use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::config::core::{ConfigSource, ConfigValue};
use crate::SchedulerResult;

/// Configuration service trait
#[async_trait]
pub trait ConfigurationService: Send + Sync {
    /// Get configuration value by key
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<ConfigValue>>;

    /// Set configuration value
    async fn set_config_value(&self, key: &str, value: &ConfigValue) -> SchedulerResult<()>;

    /// Delete configuration
    async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;

    /// List all configuration keys
    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;

    /// Reload configuration
    async fn reload_config(&self) -> SchedulerResult<()>;

    /// Get configuration source
    async fn get_config_source(&self, key: &str) -> SchedulerResult<Option<ConfigSource>>;
}

/// In-memory configuration service implementation
pub struct InMemoryConfigService {
    config: Arc<tokio::sync::RwLock<HashMap<String, ConfigValue>>>,
}

impl InMemoryConfigService {
    /// Create new in-memory configuration service
    pub fn new(config: HashMap<String, ConfigValue>) -> Self {
        Self {
            config: Arc::new(tokio::sync::RwLock::new(config)),
        }
    }
}

#[async_trait]
impl ConfigurationService for InMemoryConfigService {
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<ConfigValue>> {
        let config = self.config.read().await;
        Ok(config.get(key).cloned())
    }

    async fn set_config_value(&self, key: &str, value: &ConfigValue) -> SchedulerResult<()> {
        let mut config = self.config.write().await;
        config.insert(key.to_string(), value.clone());
        Ok(())
    }

    async fn delete_config(&self, key: &str) -> SchedulerResult<bool> {
        let mut config = self.config.write().await;
        Ok(config.remove(key).is_some())
    }

    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>> {
        let config = self.config.read().await;
        Ok(config.keys().cloned().collect())
    }

    async fn reload_config(&self) -> SchedulerResult<()> {
        // In-memory config doesn't need reloading
        Ok(())
    }

    async fn get_config_source(&self, key: &str) -> SchedulerResult<Option<ConfigSource>> {
        let config = self.config.read().await;
        Ok(config.get(key).map(|v| v.source.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_config_service() {
        let mut config = HashMap::new();
        config.insert(
            "test_key".to_string(),
            ConfigValue {
                value: serde_json::Value::String("test_value".to_string()),
                source: ConfigSource::Default,
                last_updated: std::time::SystemTime::now(),
            },
        );

        let service = InMemoryConfigService::new(config);

        // Test get config value
        let value = service.get_config_value("test_key").await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().value, "test_value");

        // Test get non-existent value
        let non_existent = service.get_config_value("non_existent").await.unwrap();
        assert!(non_existent.is_none());

        // Test set config value
        let new_value = ConfigValue {
            value: serde_json::Value::String("new_value".to_string()),
            source: ConfigSource::Runtime,
            last_updated: std::time::SystemTime::now(),
        };
        service
            .set_config_value("new_key", &new_value)
            .await
            .unwrap();

        // Test list keys
        let keys = service.list_config_keys().await.unwrap();
        assert!(keys.contains(&"test_key".to_string()));
        assert!(keys.contains(&"new_key".to_string()));

        // Test delete config
        let deleted = service.delete_config("test_key").await.unwrap();
        assert!(deleted);

        let value_after_delete = service.get_config_value("test_key").await.unwrap();
        assert!(value_after_delete.is_none());

        // Test get config source
        let source = service.get_config_source("new_key").await.unwrap();
        assert!(source.is_some());
        assert_eq!(source.unwrap(), ConfigSource::Runtime);
    }
}
