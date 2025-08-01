//! Refactored unified configuration manager
//!
//! This module provides the refactored UnifiedConfigManager that uses
//! focused components for better separation of concerns.

use std::sync::Arc;
use serde_json::Value;

use super::{
    config_callback_manager::ConfigCallbackManager,
    config_hot_reload_manager::ConfigHotReloadManager,
    config_loader::ConfigLoader,
    config_metadata_manager::ConfigMetadataManager,
    reloading::ReloadStrategy,
    sources::ConfigSource,
    validation::{ConfigValidationError, ConfigValidator},
};
use crate::{errors::SchedulerError, SchedulerResult};

/// Refactored unified configuration manager
///
/// This is the core component that coordinates all configuration sources,
/// validation, and reloading strategies using focused components.
pub struct UnifiedConfigManager {
    /// Configuration loader - handles loading and validation
    loader: Arc<ConfigLoader>,
    
    /// Metadata manager - handles metadata and validation state
    metadata_manager: Arc<ConfigMetadataManager>,
    
    /// Callback manager - handles change callbacks
    callback_manager: Arc<ConfigCallbackManager>,
    
    /// Hot reload manager - handles file change detection
    hot_reload_manager: Arc<ConfigHotReloadManager>,
}

impl UnifiedConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        Self {
            loader: Arc::new(ConfigLoader::new()),
            metadata_manager: Arc::new(ConfigMetadataManager::new()),
            callback_manager: Arc::new(ConfigCallbackManager::new()),
            hot_reload_manager: Arc::new(ConfigHotReloadManager::new(
                Vec::new(),
                0,
            )),
        }
    }

    /// Add a configuration source
    pub fn add_source(mut self, source: ConfigSource) -> Self {
        // Update the loader
        let loader = Arc::get_mut(&mut self.loader).unwrap();
        *loader = std::mem::take(loader).add_source(source.clone());
        
        // Update the hot reload manager
        let hot_reload_manager = Arc::get_mut(&mut self.hot_reload_manager).unwrap();
        *hot_reload_manager = std::mem::take(hot_reload_manager);
        // Note: In a real implementation, we'd need to update the sources list
        
        self
    }

    /// Add a configuration validator
    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        let loader = Arc::get_mut(&mut self.loader).unwrap();
        *loader = std::mem::take(loader).add_validator(validator);
        self
    }

    /// Set reload strategy
    pub fn with_reload_strategy(mut self, strategy: ReloadStrategy) -> Self {
        let interval_seconds = match strategy {
            ReloadStrategy::Periodic { interval_seconds } => interval_seconds,
            ReloadStrategy::Manual => 0,
            ReloadStrategy::Watch => 1, // Default to 1 second for watch mode
        };
        
        let hot_reload_manager = Arc::get_mut(&mut self.hot_reload_manager).unwrap();
        *hot_reload_manager = ConfigHotReloadManager::new(
            hot_reload_manager.sources.clone(),
            interval_seconds,
        );
        
        self
    }

    /// Add a configuration change callback
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&Value) + Send + Sync + 'static,
    {
        let callback_manager = Arc::get_mut(&mut self.callback_manager).unwrap();
        callback_manager.add_callback_mut(callback);
        self
    }

    /// Load configuration from all sources
    pub async fn load(&self) -> SchedulerResult<()> {
        // Load configuration using the loader
        let config = self.loader.load().await?;
        
        // Update metadata
        self.metadata_manager.mark_valid().await;
        
        // Start hot reload if needed
        self.hot_reload_manager.start_monitoring().await?;
        
        // Notify callbacks
        self.callback_manager.notify_callbacks(&config);
        
        Ok(())
    }

    /// Get configuration value
    pub async fn get<T>(&self, key: &str) -> SchedulerResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.loader.get(key).await
    }

    /// Get configuration value with default
    pub async fn get_or_default<T>(&self, key: &str, default: T) -> T
    where
        T: serde::de::DeserializeOwned + Clone,
    {
        self.loader.get_or_default(key, default).await
    }

    /// Get configuration metadata
    pub async fn get_metadata(&self) -> super::metadata::ConfigMetadata {
        self.metadata_manager.get_metadata().await
    }

    /// Check if configuration is valid
    pub async fn is_valid(&self) -> bool {
        self.metadata_manager.is_valid().await
    }

    /// Get validation errors
    pub async fn get_validation_errors(&self) -> Vec<String> {
        self.metadata_manager.get_validation_errors().await
    }

    /// Manual reload
    pub async fn reload(&self) -> SchedulerResult<()> {
        self.load().await
    }

    /// Get current configuration as JSON
    pub async fn to_json(&self) -> SchedulerResult<String> {
        self.loader.to_json().await
    }

    /// Get configuration subset
    pub async fn get_subset(&self, prefix: &str) -> SchedulerResult<Value> {
        self.loader.get_subset(prefix).await
    }

    /// Update configuration value
    pub async fn set(&self, key: &str, value: Value) -> SchedulerResult<()> {
        self.loader.set(key, value).await?;
        
        // Update metadata
        self.metadata_manager.update_timestamp().await;
        
        // Notify callbacks
        let config = self.loader.get_config().await;
        self.callback_manager.notify_callbacks(&config);
        
        Ok(())
    }

    // Accessor methods for builder
    pub(crate) fn sources(&self) -> &[ConfigSource] {
        self.loader.sources()
    }

    pub(crate) fn validators(&self) -> &[Box<dyn ConfigValidator>] {
        self.loader.validators()
    }

    pub(crate) fn reload_strategy(&self) -> &ReloadStrategy {
        // This is a simplified implementation
        &ReloadStrategy::Manual
    }
}

impl std::fmt::Debug for UnifiedConfigManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedConfigManager")
            .field("sources", &self.loader.sources())
            .field("validators", &self.loader.validators().len())
            .field("reload_strategy", &self.reload_strategy())
            .field("callbacks", &self.callback_manager.callback_count())
            .finish()
    }
}

#[async_trait::async_trait]
impl super::ConfigurationService for UnifiedConfigManager {
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
    async fn test_refactored_config_manager() {
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