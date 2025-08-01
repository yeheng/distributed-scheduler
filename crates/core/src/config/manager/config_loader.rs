//! Configuration loader - Handles loading and merging configuration from sources
//!
//! This component is responsible only for loading configuration data from various sources
//! and merging them according to precedence rules.

use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{
    sources::{ConfigMerger, ConfigSource},
    validation::{ConfigValidationError, ConfigValidator},
};
use crate::{errors::SchedulerError, SchedulerResult};

/// Configuration loader - Handles loading and merging configuration from sources
/// Follows SRP: Only responsible for loading and merging configuration data
pub struct ConfigLoader {
    /// Configuration sources
    sources: Vec<ConfigSource>,

    /// Configuration validators
    validators: Vec<Box<dyn ConfigValidator>>,

    /// Current configuration data
    config: Arc<RwLock<Value>>,
}

impl ConfigLoader {
    /// Create a new configuration loader
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            validators: Vec::new(),
            config: Arc::new(RwLock::new(Value::Object(serde_json::Map::new()))),
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

    /// Load configuration from all sources
    pub async fn load(&self) -> SchedulerResult<Value> {
        let mut merged_config = Value::Object(serde_json::Map::new());

        // Load from all sources (later sources override earlier ones)
        for source in &self.sources {
            let source_config = source.load().await?;
            merged_config = ConfigMerger::merge(merged_config, source_config);
        }

        // Validate configuration
        self.validate_config(&merged_config).await?;

        // Update configuration
        {
            let mut config = self.config.write().await;
            *config = merged_config.clone();
        }

        Ok(merged_config)
    }

    /// Get current configuration
    pub async fn get_config(&self) -> Value {
        self.config.read().await.clone()
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

        Ok(())
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
                if let Some(stripped) = key.strip_prefix(prefix) {
                    let remaining_key = stripped.trim_start_matches('.');
                    if !remaining_key.is_empty() {
                        result_map.insert(remaining_key.to_string(), value.clone());
                    }
                }
            }
            result = Value::Object(result_map);
        }

        Ok(result)
    }

    /// Validate configuration
    async fn validate_config(&self, config: &Value) -> Result<(), ConfigValidationError> {
        for validator in &self.validators {
            validator.validate(config).await?;
        }
        Ok(())
    }

    /// Get configuration as JSON
    pub async fn to_json(&self) -> SchedulerResult<String> {
        let config = self.config.read().await;
        serde_json::to_string(&*config)
            .map_err(|e| SchedulerError::Configuration(format!("Failed to serialize config: {e}")))
    }

    /// Get sources
    pub fn sources(&self) -> &[ConfigSource] {
        &self.sources
    }

    /// Get validators
    pub fn validators(&self) -> &[Box<dyn ConfigValidator>] {
        &self.validators
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}
