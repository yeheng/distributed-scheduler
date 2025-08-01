//! Main configuration manager implementation
//!
//! This module provides the core UnifiedConfigManager that coordinates
//! all configuration sources, validation, and reloading strategies.

use std::fs;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::RwLock;

use super::{
    metadata::ConfigMetadata,
    reloading::ReloadStrategy,
    sources::{ConfigMerger, ConfigSource},
    validation::{ConfigValidationError, ConfigValidator},
};
use crate::{errors::SchedulerError, SchedulerResult};

/// Configuration service trait
#[async_trait]
pub trait ConfigurationService: Send + Sync {
    /// Get configuration value
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<Value>>;

    /// Set configuration value
    async fn set_config_value(&self, key: &str, value: &Value) -> SchedulerResult<()>;

    /// Delete configuration
    async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;

    /// List all configuration keys
    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;

    /// Reload configuration
    async fn reload_config(&self) -> SchedulerResult<()>;
}

/// Unified configuration manager
///
/// This is the core component that coordinates all configuration sources,
/// validation, and reloading strategies.
pub struct UnifiedConfigManager {
    /// Current configuration data
    config: Arc<RwLock<Value>>,

    /// Configuration metadata
    metadata: Arc<RwLock<ConfigMetadata>>,

    /// Configuration sources
    sources: Vec<ConfigSource>,

    /// Configuration validators
    validators: Vec<Box<dyn ConfigValidator>>,

    /// Reload strategy
    reload_strategy: ReloadStrategy,

    /// Configuration change callbacks
    callbacks: Vec<Box<dyn Fn(&Value) + Send + Sync>>,
}

impl UnifiedConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(Value::Object(serde_json::Map::new()))),
            metadata: Arc::new(RwLock::new(ConfigMetadata::default())),
            sources: Vec::new(),
            validators: Vec::new(),
            reload_strategy: ReloadStrategy::default(),
            callbacks: Vec::new(),
        }
    }

    /// Add a configuration source
    pub fn add_source(mut self, source: ConfigSource) -> Self {
        self.sources.push(source);
        self
    }

    /// Add a configuration validator
    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.validators.push(validator);
        self
    }

    /// Set reload strategy
    pub fn with_reload_strategy(mut self, strategy: ReloadStrategy) -> Self {
        self.reload_strategy = strategy;
        self
    }

    /// Add a configuration change callback
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&Value) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    /// Load configuration from all sources
    pub async fn load(&self) -> SchedulerResult<()> {
        let mut merged_config = Value::Object(serde_json::Map::new());

        // Load from all sources (later sources override earlier ones)
        for source in &self.sources {
            let source_config = source.load().await?;
            merged_config = ConfigMerger::merge(merged_config, source_config);
        }

        // Validate configuration
        let validation_result = self.validate_config(&merged_config).await;

        // Update configuration and metadata
        {
            let mut config = self.config.write().await;
            *config = merged_config.clone();
        }

        {
            let mut metadata = self.metadata.write().await;
            metadata.is_valid = validation_result.is_ok();
            metadata.validation_errors = validation_result
                .err()
                .map(|e| vec![e.to_string()])
                .unwrap_or_default();
            metadata.update_timestamp();
        }

        // Start periodic reload if needed
        if matches!(self.reload_strategy, ReloadStrategy::Periodic { .. }) {
            self.start_periodic_reload().await;
        }

        // Call callbacks
        let config = self.config.read().await;
        for callback in &self.callbacks {
            callback(&config);
        }

        Ok(())
    }

    /// Validate configuration
    async fn validate_config(&self, config: &Value) -> Result<(), ConfigValidationError> {
        for validator in &self.validators {
            validator.validate(config).await?;
        }
        Ok(())
    }

    /// Start periodic reload
    async fn start_periodic_reload(&self) {
        if let ReloadStrategy::Periodic { interval_seconds } = self.reload_strategy {
            let _config = self.config.clone();
            let metadata = self.metadata.clone();
            let sources = self.sources.clone();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

                loop {
                    interval.tick().await;

                    // Check all sources for changes
                    for source in &sources {
                        if let ConfigSource::File { path } = source {
                            if let Ok(file_metadata) = fs::metadata(path) {
                                if let Ok(modified) = file_metadata.modified() {
                                    let modified_secs = modified
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();

                                    let current_metadata = metadata.read().await;
                                    if modified_secs > current_metadata.last_modified {
                                        // Update timestamp
                                        let mut metadata_guard = metadata.write().await;
                                        metadata_guard.update_timestamp();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    /// Get configuration value
    pub async fn get<T>(&self, key: &str) -> SchedulerResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let config = self.config.read().await;

        // Try nested traversal first
        let keys: Vec<&str> = key.split('.').collect();
        let mut current = &*config;
        let mut found_nested = true;

        for k in &keys {
            match current {
                Value::Object(map) => {
                    if let Some(value) = map.get(*k) {
                        current = value;
                    } else {
                        found_nested = false;
                        break;
                    }
                }
                _ => {
                    found_nested = false;
                    break;
                }
            }
        }

        if found_nested {
            return T::deserialize(current).map_err(|e| {
                SchedulerError::Configuration(format!(
                    "Failed to deserialize value for key {key}: {e}"
                ))
            });
        }

        // If nested traversal failed, try flat key lookup
        if let Value::Object(map) = &*config {
            if let Some(value) = map.get(key) {
                return T::deserialize(value).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to deserialize value for key {key}: {e}"
                    ))
                });
            }
        }

        Err(SchedulerError::Configuration(format!(
            "Key not found: {key}"
        )))
    }

    /// Get configuration value with default
    pub async fn get_or_default<T>(&self, key: &str, default: T) -> T
    where
        T: serde::de::DeserializeOwned + Clone,
    {
        match self.get(key).await {
            Ok(value) => value,
            Err(_) => default,
        }
    }

    /// Get configuration metadata
    pub async fn get_metadata(&self) -> ConfigMetadata {
        self.metadata.read().await.clone()
    }

    /// Check if configuration is valid
    pub async fn is_valid(&self) -> bool {
        self.metadata.read().await.is_valid()
    }

    /// Get validation errors
    pub async fn get_validation_errors(&self) -> Vec<String> {
        self.metadata.read().await.validation_errors().to_vec()
    }

    /// Manual reload
    pub async fn reload(&self) -> SchedulerResult<()> {
        self.load().await
    }

    /// Get current configuration as JSON
    pub async fn to_json(&self) -> SchedulerResult<String> {
        let config = self.config.read().await;
        serde_json::to_string(&*config)
            .map_err(|e| SchedulerError::Configuration(format!("Failed to serialize config: {e}")))
    }

    /// Get configuration subset
    pub async fn get_subset(&self, prefix: &str) -> SchedulerResult<Value> {
        let config = self.config.read().await;

        let mut result = Value::Object(serde_json::Map::new());

        if let Value::Object(map) = &*config {
            // Check if there's a direct nested object with this prefix
            if let Some(nested_value) = map.get(prefix) {
                return Ok(nested_value.clone());
            }

            // Collect all keys that start with the prefix
            let mut result_map = serde_json::Map::new();
            for (key, value) in map {
                if key.starts_with(prefix) {
                    let remaining_key = key[prefix.len()..].trim_start_matches('.');
                    if !remaining_key.is_empty() {
                        result_map.insert(remaining_key.to_string(), value.clone());
                    }
                }
            }
            result = Value::Object(result_map);
        }

        Ok(result)
    }

    /// Update configuration value
    pub async fn set(&self, key: &str, value: Value) -> SchedulerResult<()> {
        let mut config = self.config.write().await;

        let keys: Vec<&str> = key.split('.').collect();
        let mut current = &mut *config;

        // Navigate to the parent object
        for (i, k) in keys.iter().enumerate() {
            if i == keys.len() - 1 {
                // Last key - set the value
                if let Value::Object(map) = current {
                    map.insert(k.to_string(), value);
                }
                break;
            }

            match current {
                Value::Object(map) => {
                    if !map.contains_key(*k) {
                        map.insert(k.to_string(), Value::Object(serde_json::Map::new()));
                    }
                    current = map.get_mut(*k).unwrap();
                }
                _ => {
                    return Err(SchedulerError::Configuration(format!(
                        "Cannot traverse into non-object value at key: {k}"
                    )));
                }
            }
        }

        // Update metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.update_timestamp();
        }

        // Call callbacks
        for callback in &self.callbacks {
            callback(&config);
        }

        Ok(())
    }

    // Accessor methods for builder
    pub(crate) fn sources(&self) -> &[ConfigSource] {
        &self.sources
    }

    pub(crate) fn validators(&self) -> &[Box<dyn ConfigValidator>] {
        &self.validators
    }

    pub(crate) fn reload_strategy(&self) -> &ReloadStrategy {
        &self.reload_strategy
    }
}

impl std::fmt::Debug for UnifiedConfigManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedConfigManager")
            .field("sources", &self.sources)
            .field("validators", &self.validators.len())
            .field("reload_strategy", &self.reload_strategy)
            .field("callbacks", &self.callbacks.len())
            .finish()
    }
}

#[async_trait]
impl ConfigurationService for UnifiedConfigManager {
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<Value>> {
        match self.get::<Value>(key).await {
            Ok(value) => Ok(Some(value)),
            Err(SchedulerError::Configuration(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn set_config_value(&self, key: &str, value: &Value) -> SchedulerResult<()> {
        self.set(key, value.clone()).await
    }

    async fn delete_config(&self, _key: &str) -> SchedulerResult<bool> {
        // This is a simplified implementation
        // In a real implementation, you would need to navigate the JSON structure
        Ok(false)
    }

    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>> {
        // This is a simplified implementation
        // In a real implementation, you would traverse the JSON structure
        Ok(vec![])
    }

    async fn reload_config(&self) -> SchedulerResult<()> {
        self.reload().await
    }
}

impl Default for UnifiedConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::manager::sources::ConfigSource;

    #[tokio::test]
    async fn test_unified_config_manager() {
        let manager = UnifiedConfigManager::new().add_source(ConfigSource::Memory {
            data: Value::Object(
                serde_json::json!({
                    "server": {
                        "host": "localhost",
                        "port": 8080
                    }
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        });

        manager.load().await.unwrap();

        let host: String = manager.get("server.host").await.unwrap();
        assert_eq!(host, "localhost");

        let port: u16 = manager.get("server.port").await.unwrap();
        assert_eq!(port, 8080);
    }

    #[tokio::test]
    async fn test_config_service() {
        let manager = UnifiedConfigManager::new().add_source(ConfigSource::Memory {
            data: Value::Object(
                serde_json::json!({
                    "test": "value"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        });

        manager.load().await.unwrap();

        let value = manager.get_config_value("test").await.unwrap();
        assert_eq!(value, Some(Value::String("value".to_string())));
    }
}
