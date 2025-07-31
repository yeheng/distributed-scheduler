use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use crate::{SchedulerError, SchedulerResult};

/// Configuration value wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValue {
    pub value: Value,
    pub source: ConfigSource,
    pub last_updated: std::time::SystemTime,
}

/// Configuration source tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfigSource {
    File(String),
    Environment,
    Default,
    Runtime,
}

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
    pub async fn get(&self) -> SchedulerResult<Option<T>> {
        match self.service.get_config_value(&self.key).await? {
            Some(config_value) => {
                let typed_value: T = serde_json::from_value(config_value.value).map_err(|e| {
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
    pub async fn get_or_default(&self, default: T) -> SchedulerResult<T> {
        match self.get().await? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }

    /// Set configuration value with type safety
    pub async fn set(&self, value: &T) -> SchedulerResult<()> {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!(
                "Failed to serialize config '{}': {}",
                self.key, e
            ))
        })?;
        let config_value = ConfigValue {
            value: json_value,
            source: ConfigSource::Runtime,
            last_updated: std::time::SystemTime::now(),
        };
        self.service
            .set_config_value(&self.key, &config_value)
            .await
    }

    /// Get configuration key
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Configuration service trait
#[async_trait]
pub trait ConfigurationService: Send + Sync {
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<ConfigValue>>;
    async fn set_config_value(&self, key: &str, value: &ConfigValue) -> SchedulerResult<()>;
    async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;
    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;
    async fn reload_config(&self) -> SchedulerResult<()>;
    async fn get_config_source(&self, key: &str) -> SchedulerResult<Option<ConfigSource>>;
}

/// Configuration cache for performance
pub struct ConfigCache {
    pub cache: Arc<tokio::sync::RwLock<std::collections::HashMap<String, ConfigValue>>>,
    pub ttl: std::time::Duration,
}

impl ConfigCache {
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            ttl,
        }
    }

    pub async fn get(&self, key: &str) -> Option<ConfigValue> {
        let cache = self.cache.read().await;
        cache.get(key).cloned().and_then(|config_value| {
            if config_value
                .last_updated
                .elapsed()
                .unwrap_or(std::time::Duration::ZERO)
                < self.ttl
            {
                Some(config_value)
            } else {
                None
            }
        })
    }

    pub async fn set(&self, key: String, value: ConfigValue) {
        let mut cache = self.cache.write().await;
        cache.insert(key, value);
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

impl Default for ConfigCache {
    fn default() -> Self {
        Self::new(std::time::Duration::from_secs(300)) // 5 minutes
    }
}
