use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use crate::{SchedulerError, SchedulerResult};

use super::{
    core::{ConfigurationService, TypedConfig, ConfigValue, ConfigCache, ConfigSource},
    validation::{ConfigValidator, ValidatorRegistry},
    loader::{MultiSourceLoader, FileConfigLoader, MergeStrategy},
    environment::ProfileRegistry,
    hot_reload::HotReloadManager,
};

/// Configuration manager - Unified configuration management
pub struct ConfigManager {
    /// Configuration service
    service: Arc<dyn ConfigurationService>,
    /// Configuration cache
    cache: ConfigCache,
    /// Hot reload manager
    hot_reload: Option<HotReloadManager>,
    /// Validator registry
    validators: ValidatorRegistry,
}

impl ConfigManager {
    /// Create new configuration manager
    pub fn new(service: Arc<dyn ConfigurationService>) -> Self {
        Self {
            service,
            cache: ConfigCache::default(),
            hot_reload: None,
            validators: ValidatorRegistry::default(),
        }
    }

    /// Create with custom cache TTL
    pub fn with_cache_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.cache = ConfigCache::new(ttl);
        self
    }

    /// Add configuration validator
    pub fn with_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.validators = self.validators.add_validator(validator);
        self
    }

    /// Enable hot reload
    pub fn with_hot_reload(mut self, hot_reload: HotReloadManager) -> Self {
        self.hot_reload = Some(hot_reload);
        self
    }

    /// Get typed configuration
    pub fn get_typed<T>(&self, key: &str) -> TypedConfig<T>
    where
        T: Serialize + DeserializeOwned + Send + Sync,
    {
        TypedConfig::new(Arc::clone(&self.service), key.to_string())
    }

    /// Get configuration value with cache
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>, SchedulerError>
    where
        T: DeserializeOwned + Send + Sync,
    {
        // Try cache first
        if let Some(cached_value) = self.cache.get(key).await {
            let typed_value: T = serde_json::from_value(cached_value.value).map_err(|e| {
                SchedulerError::Configuration(format!("Failed to deserialize cached config '{}': {}", key, e))
            })?;
            return Ok(Some(typed_value));
        }

        // Get from service
        if let Some(config_value) = self.service.get_config_value(key).await? {
            // Cache the result
            self.cache.set(key.to_string(), config_value.clone()).await;
            
            let typed_value: T = serde_json::from_value(config_value.value).map_err(|e| {
                SchedulerError::Configuration(format!("Failed to deserialize config '{}': {}", key, e))
            })?;
            Ok(Some(typed_value))
        } else {
            Ok(None)
        }
    }

    /// Get configuration value with default
    pub async fn get_or_default<T>(&self, key: &str, default: T) -> Result<T, SchedulerError>
    where
        T: DeserializeOwned + Send + Sync,
    {
        match self.get::<T>(key).await? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }

    /// Set configuration value
    pub async fn set<T>(&self, key: &str, value: &T) -> Result<(), SchedulerError>
    where
        T: Serialize + Send + Sync,
    {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!("Failed to serialize config '{}': {}", key, e))
        })?;
        
        let config_value = ConfigValue {
            value: json_value,
            source: ConfigSource::Runtime,
            last_updated: std::time::SystemTime::now(),
        };

        // Set in service
        self.service.set_config_value(key, &config_value).await?;
        
        // Update cache
        self.cache.set(key.to_string(), config_value).await;
        
        Ok(())
    }

    /// Delete configuration
    pub async fn delete(&self, key: &str) -> Result<bool, SchedulerError> {
        let deleted = self.service.delete_config(key).await?;
        if deleted {
            // Clear from cache
            let mut cache = self.cache.cache.write().await;
            cache.remove(key);
        }
        Ok(deleted)
    }

    /// List all configuration keys
    pub async fn list_keys(&self) -> Result<Vec<String>, SchedulerError> {
        self.service.list_config_keys().await
    }

    /// Reload configuration
    pub async fn reload(&self) -> Result<(), SchedulerError> {
        self.service.reload_config().await?;
        // Clear cache
        self.cache.clear().await;
        Ok(())
    }

    /// Get configuration source
    pub async fn get_source(&self, key: &str) -> Result<Option<super::core::ConfigSource>, SchedulerError> {
        self.service.get_config_source(key).await
    }

    /// Clear configuration cache
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }

    /// Get hot reload manager
    pub fn get_hot_reload(&self) -> Option<&HotReloadManager> {
        self.hot_reload.as_ref()
    }
}

/// Configuration builder - Fluent interface for building configuration systems
pub struct ConfigBuilder {
    /// Multi-source loader
    loader: MultiSourceLoader,
    /// Profile registry
    profiles: ProfileRegistry,
    /// Configuration cache
    cache_ttl: std::time::Duration,
    /// Validators
    validators: Vec<Box<dyn ConfigValidator>>,
    /// Hot reload enabled
    hot_reload_enabled: bool,
}

impl ConfigBuilder {
    /// Create new configuration builder
    pub fn new() -> Self {
        Self {
            loader: MultiSourceLoader::default(),
            profiles: ProfileRegistry::default(),
            cache_ttl: std::time::Duration::from_secs(300), // 5 minutes
            validators: Vec::new(),
            hot_reload_enabled: false,
        }
    }

    /// Add configuration file
    pub fn add_file(mut self, _path: &str) -> Result<Self, SchedulerError> {
        let file_loader = Box::new(FileConfigLoader::new());
        self.loader = self.loader.add_loader(file_loader);
        Ok(self)
    }

    /// Add environment variables with prefix
    pub fn add_env_vars(mut self, prefix: &str) -> Self {
        let env_loader = Box::new(EnvConfigLoader::new(prefix.to_string()));
        self.loader = self.loader.add_loader(env_loader);
        self
    }

    /// Set merge strategy
    pub fn with_merge_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.loader = self.loader.with_merge_strategy(strategy);
        self
    }

    /// Set cache TTL
    pub fn with_cache_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Add configuration validator
    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.validators.push(validator);
        self
    }

    /// Enable hot reload
    pub fn with_hot_reload(mut self, enabled: bool) -> Self {
        self.hot_reload_enabled = enabled;
        self
    }

    /// Add configuration profile
    pub fn add_profile(mut self, profile: super::environment::ConfigProfile) -> Self {
        self.profiles = self.profiles.add_profile(profile);
        self
    }

    /// Set active profile
    pub fn set_active_profile(mut self, profile_name: &str) -> Result<Self, SchedulerError> {
        self.profiles.set_active_profile(profile_name)?;
        Ok(self)
    }

    /// Build configuration manager
    pub async fn build(self) -> Result<ConfigManager, SchedulerError> {
        // Load configuration from all sources
        let mut config_values = self.loader.load_all()?;
        
        // Apply profile configuration
        if let Some(profile_config) = self.profiles.get_active_config() {
            for (key, value) in profile_config {
                let config_value = ConfigValue {
                    value,
                    source: super::core::ConfigSource::Default,
                    last_updated: std::time::SystemTime::now(),
                };
                config_values.insert(key, config_value);
            }
        }

        // Create configuration service
        let service = Arc::new(InMemoryConfigService::new(config_values));

        // Create config manager
        let mut manager = ConfigManager::new(service)
            .with_cache_ttl(self.cache_ttl);

        // Add validators
        for validator in self.validators {
            manager = manager.with_validator(validator);
        }

        // Enable hot reload if requested
        if self.hot_reload_enabled {
            let hot_reload = HotReloadManager::new();
            manager = manager.with_hot_reload(hot_reload);
        }

        Ok(manager)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
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

/// Environment variable configuration loader
pub struct EnvConfigLoader {
    prefix: String,
}

impl EnvConfigLoader {
    /// Create new environment configuration loader
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }
}

impl super::loader::ConfigLoader for EnvConfigLoader {
    fn load(&self, _source: &super::core::ConfigSource) -> Result<HashMap<String, ConfigValue>, SchedulerError> {
        let mut result = HashMap::new();
        let timestamp = std::time::SystemTime::now();

        for (key, value) in std::env::vars() {
            if key.starts_with(&self.prefix) {
                let config_key = key[self.prefix.len()..].to_lowercase().replace('_', ".");
                let json_value = serde_json::Value::String(value);
                
                result.insert(config_key, ConfigValue {
                    value: json_value,
                    source: ConfigSource::Environment,
                    last_updated: timestamp,
                });
            }
        }

        Ok(result)
    }

    fn load_from_file(&self, _path: &std::path::Path) -> Result<HashMap<String, ConfigValue>, SchedulerError> {
        Err(SchedulerError::Configuration("EnvConfigLoader doesn't support file loading".to_string()))
    }

    fn load_from_env(&self, _prefix: &str) -> Result<HashMap<String, ConfigValue>, SchedulerError> {
        // Already handled in load method
        Ok(HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::core::ConfigSource;

    #[tokio::test]
    async fn test_config_builder() {
        let builder = ConfigBuilder::new()
            .with_cache_ttl(std::time::Duration::from_secs(60))
            .with_hot_reload(true);

        let manager = builder.build().await.unwrap();
        assert!(manager.get_hot_reload().is_some());
    }

    #[tokio::test]
    async fn test_in_memory_config_service() {
        let mut config = HashMap::new();
        config.insert("test_key".to_string(), ConfigValue {
            value: serde_json::Value::String("test_value".to_string()),
            source: ConfigSource::Default,
            last_updated: std::time::SystemTime::now(),
        });

        let service = Arc::new(InMemoryConfigService::new(config));
        let manager = ConfigManager::new(service);

        let value: Option<String> = manager.get("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }
}

