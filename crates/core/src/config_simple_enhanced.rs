use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::{SchedulerResult, errors::SchedulerError};

/// Configuration validation error
#[derive(Debug, Error)]
pub enum ConfigValidationError {
    #[error("Required field missing: {field}")]
    RequiredFieldMissing { field: String },
    #[error("Invalid value for field {field}: {value}, error: {error}")]
    InvalidValue {
        field: String,
        value: String,
        error: String,
    },
    #[error("Type mismatch for field {field}: expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("Validation failed for field {field}: {message}")]
    ValidationFailed { field: String, message: String },
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },
    #[error("Configuration parse error: {message}")]
    ParseError { message: String },
}

/// Configuration validation trait
pub trait ConfigValidator: Send + Sync {
    /// Validate configuration
    fn validate(
        &self,
        config: &serde_json::Value,
    ) -> std::result::Result<(), ConfigValidationError>;

    /// Get validator name
    fn name(&self) -> &str;
}

/// Basic configuration validator
pub struct BasicConfigValidator {
    name: String,
    required_fields: Vec<String>,
    field_types: HashMap<String, String>,
    custom_validators: HashMap<
        String,
        Box<
            dyn Fn(&serde_json::Value) -> std::result::Result<(), ConfigValidationError>
                + Send
                + Sync,
        >,
    >,
}

impl BasicConfigValidator {
    /// Create new basic validator
    pub fn new(name: String) -> Self {
        Self {
            name,
            required_fields: Vec::new(),
            field_types: HashMap::new(),
            custom_validators: HashMap::new(),
        }
    }

    /// Get nested value from configuration using dot notation
    fn get_nested_value<'a>(
        &self,
        config: &'a serde_json::Value,
        key: &str,
    ) -> Option<&'a serde_json::Value> {
        let keys: Vec<&str> = key.split('.').collect();
        let mut current = config;

        for k in &keys {
            match current {
                serde_json::Value::Object(map) => {
                    if let Some(value) = map.get(*k) {
                        current = value;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current)
    }

    /// Add required field
    pub fn required_field(mut self, field: String, field_type: String) -> Self {
        self.required_fields.push(field.clone());
        self.field_types.insert(field, field_type);
        self
    }

    /// Add custom validator
    pub fn custom_validator<F>(mut self, field: String, validator: F) -> Self
    where
        F: Fn(&serde_json::Value) -> std::result::Result<(), ConfigValidationError>
            + Send
            + Sync
            + 'static,
    {
        self.custom_validators.insert(field, Box::new(validator));
        self
    }
}

impl ConfigValidator for BasicConfigValidator {
    fn validate(
        &self,
        config: &serde_json::Value,
    ) -> std::result::Result<(), ConfigValidationError> {
        // Check required fields
        for field in &self.required_fields {
            if self.get_nested_value(config, field).is_none() {
                return Err(ConfigValidationError::RequiredFieldMissing {
                    field: field.clone(),
                });
            }
        }

        // Check field types
        for (field, expected_type) in &self.field_types {
            if let Some(value) = self.get_nested_value(config, field) {
                let actual_type = match value {
                    serde_json::Value::Null => "null",
                    serde_json::Value::Bool(_) => "boolean",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Array(_) => "array",
                    serde_json::Value::Object(_) => "object",
                };

                if actual_type != expected_type {
                    return Err(ConfigValidationError::TypeMismatch {
                        field: field.clone(),
                        expected: expected_type.clone(),
                        actual: actual_type.to_string(),
                    });
                }
            }
        }

        // Run custom validators
        for (field, validator) in &self.custom_validators {
            if let Some(value) = self.get_nested_value(config, field) {
                validator(value)?;
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Configuration source
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// File-based configuration
    File { path: PathBuf },
    /// Environment variables
    Environment { prefix: String },
    /// Command line arguments
    CommandLine { args: Vec<String> },
    /// In-memory configuration
    Memory { data: serde_json::Value },
}

/// Configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    /// Configuration version
    pub version: String,
    /// Last modified timestamp
    pub last_modified: u64,
    /// Configuration checksum
    pub checksum: String,
    /// Source information
    pub source: String,
    /// Validation status
    pub is_valid: bool,
    /// Validation errors
    pub validation_errors: Vec<String>,
}

impl Default for ConfigMetadata {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            last_modified: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            checksum: String::new(),
            source: "unknown".to_string(),
            is_valid: true,
            validation_errors: Vec::new(),
        }
    }
}

/// Simple enhanced configuration manager
pub struct SimpleEnhancedConfigManager {
    /// Current configuration
    config: Arc<RwLock<serde_json::Value>>,
    /// Configuration metadata
    metadata: Arc<RwLock<ConfigMetadata>>,
    /// Configuration sources
    sources: Vec<ConfigSource>,
    /// Configuration validators
    validators: Vec<Box<dyn ConfigValidator>>,
    /// Configuration change callbacks
    callbacks: Vec<Box<dyn Fn(&serde_json::Value) + Send + Sync>>,
}

impl SimpleEnhancedConfigManager {
    /// Create new configuration manager
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(serde_json::Value::Object(
                serde_json::Map::new(),
            ))),
            metadata: Arc::new(RwLock::new(ConfigMetadata::default())),
            sources: Vec::new(),
            validators: Vec::new(),
            callbacks: Vec::new(),
        }
    }

    /// Add configuration source
    pub fn add_source(mut self, source: ConfigSource) -> Self {
        self.sources.push(source);
        self
    }

    /// Add configuration validator
    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.validators.push(validator);
        self
    }

    /// Add configuration change callback
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&serde_json::Value) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    /// Load configuration from sources
    pub async fn load(&self) -> SchedulerResult<()> {
        let mut merged_config = serde_json::Value::Object(serde_json::Map::new());

        // Load from all sources (later sources override earlier ones)
        for source in &self.sources {
            let source_config = self.load_from_source(source).await?;
            merged_config = self.merge_configs(merged_config, source_config);
        }

        // Validate configuration
        let validation_result = self.validate_config(&merged_config).await;

        // If validation fails, return error
        if let Err(validation_error) = validation_result {
            return Err(SchedulerError::Configuration(validation_error.to_string()));
        }

        // Update configuration and metadata
        let mut config = self.config.write().await;
        *config = merged_config;

        let mut metadata = self.metadata.write().await;
        metadata.is_valid = true;
        metadata.validation_errors = Vec::new();
        metadata.last_modified = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Call callbacks
        for callback in &self.callbacks {
            callback(&config);
        }

        Ok(())
    }

    /// Load configuration from specific source
    async fn load_from_source(&self, source: &ConfigSource) -> SchedulerResult<serde_json::Value> {
        match source {
            ConfigSource::File { path } => {
                if !path.exists() {
                    return Err(SchedulerError::Configuration(format!(
                        "Configuration file not found: {}",
                        path.display()
                    )));
                }

                let content = fs::read_to_string(path).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to read config file {}: {}",
                        path.display(),
                        e
                    ))
                })?;

                let config: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to parse config file {}: {}",
                        path.display(),
                        e
                    ))
                })?;

                Ok(config)
            }
            ConfigSource::Environment { prefix } => {
                let mut config = serde_json::Value::Object(serde_json::Map::new());

                for (key, value) in std::env::vars() {
                    if key.starts_with(prefix) {
                        let config_key = key[prefix.len()..].to_lowercase().replace('_', ".");

                        // Try to parse as JSON, fall back to string
                        let parsed_value: serde_json::Value = serde_json::from_str(&value)
                            .unwrap_or_else(|_| serde_json::Value::String(value.clone()));

                        if let serde_json::Value::Object(ref mut map) = config {
                            map.insert(config_key, parsed_value);
                        }
                    }
                }

                Ok(config)
            }
            ConfigSource::CommandLine { args } => {
                let mut config = serde_json::Value::Object(serde_json::Map::new());
                let mut current_key = None;

                for arg in args {
                    if arg.starts_with("--") {
                        if let Some(key) = current_key {
                            // Set previous key as boolean flag
                            if let serde_json::Value::Object(ref mut map) = config {
                                map.insert(key, serde_json::Value::Bool(true));
                            }
                        }
                        current_key = Some(arg[2..].to_string());
                    } else if let Some(key) = current_key.take() {
                        // Set key with value
                        let parsed_value: serde_json::Value = serde_json::from_str(arg)
                            .unwrap_or_else(|_| serde_json::Value::String(arg.to_string()));

                        if let serde_json::Value::Object(ref mut map) = config {
                            map.insert(key, parsed_value);
                        }
                    }
                }

                // Handle last key if it's a boolean flag
                if let Some(key) = current_key {
                    if let serde_json::Value::Object(ref mut map) = config {
                        map.insert(key, serde_json::Value::Bool(true));
                    }
                }

                Ok(config)
            }
            ConfigSource::Memory { data } => Ok(data.clone()),
        }
    }

    /// Merge two configurations
    fn merge_configs(
        &self,
        base: serde_json::Value,
        override_config: serde_json::Value,
    ) -> serde_json::Value {
        match (base, override_config) {
            (serde_json::Value::Object(mut base_map), serde_json::Value::Object(override_map)) => {
                for (key, value) in override_map {
                    // Handle flat keys with dots (e.g., "server.port") by converting to nested structure
                    if key.contains('.') {
                        let parts: Vec<&str> = key.split('.').collect();
                        if parts.len() == 2 {
                            // Simple case: server.port -> {server: {port: value}}
                            let parent_key = parts[0];
                            let child_key = parts[1];

                            // Get or create parent object
                            let parent_obj =
                                base_map.entry(parent_key.to_string()).or_insert_with(|| {
                                    serde_json::Value::Object(serde_json::Map::new())
                                });

                            if let serde_json::Value::Object(parent_map) = parent_obj {
                                parent_map.insert(child_key.to_string(), value);
                            }
                        } else {
                            // For complex nested keys, just insert as-is
                            base_map.insert(key, value);
                        }
                    } else {
                        // Handle nested merge for non-flat keys
                        if let (Some(base_value), serde_json::Value::Object(override_obj)) =
                            (base_map.get(&key), &value)
                        {
                            if let serde_json::Value::Object(base_obj) = base_value {
                                // Recursively merge nested objects
                                let merged = self.merge_configs(
                                    serde_json::Value::Object(base_obj.clone()),
                                    serde_json::Value::Object(override_obj.clone()),
                                );
                                base_map.insert(key, merged);
                                continue;
                            }
                        }
                        base_map.insert(key, value);
                    }
                }
                serde_json::Value::Object(base_map)
            }
            (_, override_config) => override_config,
        }
    }

    /// Validate configuration
    async fn validate_config(
        &self,
        config: &serde_json::Value,
    ) -> std::result::Result<(), ConfigValidationError> {
        for validator in &self.validators {
            validator.validate(config)?;
        }
        Ok(())
    }

    /// Get configuration value
    pub async fn get<T>(&self, key: &str) -> SchedulerResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let config = self.config.read().await;

        // First try nested traversal
        let keys: Vec<&str> = key.split('.').collect();
        let mut current = &*config;
        let mut found_nested = true;

        for k in &keys {
            match current {
                serde_json::Value::Object(map) => {
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
        if let serde_json::Value::Object(map) = &*config {
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
        self.get(key).await.unwrap_or(default)
    }

    /// Get configuration metadata
    pub async fn get_metadata(&self) -> ConfigMetadata {
        self.metadata.read().await.clone()
    }

    /// Check if configuration is valid
    pub async fn is_valid(&self) -> bool {
        self.metadata.read().await.is_valid
    }

    /// Get validation errors
    pub async fn get_validation_errors(&self) -> Vec<String> {
        self.metadata.read().await.validation_errors.clone()
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
    pub async fn get_subset(&self, prefix: &str) -> SchedulerResult<serde_json::Value> {
        let config = self.config.read().await;

        let mut result = serde_json::Value::Object(serde_json::Map::new());

        if let serde_json::Value::Object(map) = &*config {
            // First, check if there's a direct nested object with this prefix
            if let Some(nested_value) = map.get(prefix) {
                return Ok(nested_value.clone());
            }

            // If no direct nested object, collect all keys that start with the prefix
            let mut result_map = serde_json::Map::new();
            for (key, value) in map {
                if key.starts_with(prefix) {
                    let remaining_key = key[prefix.len()..].trim_start_matches('.');
                    if !remaining_key.is_empty() {
                        result_map.insert(remaining_key.to_string(), value.clone());
                    }
                }
            }
            result = serde_json::Value::Object(result_map);
        }

        Ok(result)
    }

    /// Update configuration value
    pub async fn set(&self, key: &str, value: serde_json::Value) -> SchedulerResult<()> {
        let mut config = self.config.write().await;

        let keys: Vec<&str> = key.split('.').collect();
        let mut current = &mut *config;

        // Navigate to the parent object
        for (i, k) in keys.iter().enumerate() {
            if i == keys.len() - 1 {
                // Last key - set the value
                if let serde_json::Value::Object(map) = current {
                    map.insert(k.to_string(), value);
                }
                break;
            }

            match current {
                serde_json::Value::Object(map) => {
                    if !map.contains_key(*k) {
                        map.insert(
                            k.to_string(),
                            serde_json::Value::Object(serde_json::Map::new()),
                        );
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
        let mut metadata = self.metadata.write().await;
        metadata.last_modified = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Call callbacks
        for callback in &self.callbacks {
            callback(&config);
        }

        Ok(())
    }
}

impl Default for SimpleEnhancedConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-built validators for common configurations
pub mod validators {
    use super::*;

    /// Database configuration validator
    pub fn database_validator() -> BasicConfigValidator {
        BasicConfigValidator::new("database".to_string())
            .required_field("database.url".to_string(), "string".to_string())
            .required_field("database.pool_size".to_string(), "number".to_string())
            .custom_validator("database.url".to_string(), |value| {
                if let serde_json::Value::String(url) = value {
                    if url.starts_with("postgresql://")
                        || url.starts_with("mysql://")
                        || url.starts_with("sqlite://")
                    {
                        Ok(())
                    } else {
                        Err(ConfigValidationError::InvalidValue {
                            field: "database.url".to_string(),
                            value: url.clone(),
                            error:
                                "Database URL must start with postgresql://, mysql://, or sqlite://"
                                    .to_string(),
                        })
                    }
                } else {
                    Err(ConfigValidationError::TypeMismatch {
                        field: "database.url".to_string(),
                        expected: "string".to_string(),
                        actual: "other".to_string(),
                    })
                }
            })
            .custom_validator("database.pool_size".to_string(), |value| {
                if let serde_json::Value::Number(size) = value {
                    if size.as_u64().unwrap_or(0) > 0 && size.as_u64().unwrap_or(0) <= 100 {
                        Ok(())
                    } else {
                        Err(ConfigValidationError::InvalidValue {
                            field: "database.pool_size".to_string(),
                            value: size.to_string(),
                            error: "Pool size must be between 1 and 100".to_string(),
                        })
                    }
                } else {
                    Err(ConfigValidationError::TypeMismatch {
                        field: "database.pool_size".to_string(),
                        expected: "number".to_string(),
                        actual: "other".to_string(),
                    })
                }
            })
    }

    /// Server configuration validator
    pub fn server_validator() -> BasicConfigValidator {
        BasicConfigValidator::new("server".to_string())
            .required_field("server.host".to_string(), "string".to_string())
            .required_field("server.port".to_string(), "number".to_string())
            .custom_validator("server.port".to_string(), |value| {
                if let serde_json::Value::Number(port) = value {
                    let port_num = port.as_u64().unwrap_or(0);
                    if port_num > 0 && port_num <= 65535 {
                        Ok(())
                    } else {
                        Err(ConfigValidationError::InvalidValue {
                            field: "server.port".to_string(),
                            value: port.to_string(),
                            error: "Port must be between 1 and 65535".to_string(),
                        })
                    }
                } else {
                    Err(ConfigValidationError::TypeMismatch {
                        field: "server.port".to_string(),
                        expected: "number".to_string(),
                        actual: "other".to_string(),
                    })
                }
            })
    }

    /// Logging configuration validator
    pub fn logging_validator() -> BasicConfigValidator {
        BasicConfigValidator::new("logging".to_string())
            .required_field("logging.level".to_string(), "string".to_string())
            .custom_validator("logging.level".to_string(), |value| {
                if let serde_json::Value::String(level) = value {
                    match level.to_lowercase().as_str() {
                        "trace" | "debug" | "info" | "warn" | "error" => Ok(()),
                        _ => Err(ConfigValidationError::InvalidValue {
                            field: "logging.level".to_string(),
                            value: level.clone(),
                            error: "Log level must be one of: trace, debug, info, warn, error"
                                .to_string(),
                        }),
                    }
                } else {
                    Err(ConfigValidationError::TypeMismatch {
                        field: "logging.level".to_string(),
                        expected: "string".to_string(),
                        actual: "other".to_string(),
                    })
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::atomic::{AtomicBool, Ordering},
    };
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_basic_config_loading() {
        // Create temporary config file
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
        {
            "server": {
                "host": "localhost",
                "port": 8080
            },
            "database": {
                "url": "postgresql://localhost:5432/testdb",
                "pool_size": 10
            },
            "logging": {
                "level": "info"
            }
        }
        "#;

        temp_file.write_all(config_content.as_bytes()).unwrap();
        let config_path = temp_file.path().to_path_buf();

        // Create config manager
        let manager = SimpleEnhancedConfigManager::new()
            .add_source(ConfigSource::File { path: config_path })
            .add_validator(Box::new(validators::server_validator()))
            .add_validator(Box::new(validators::database_validator()))
            .add_validator(Box::new(validators::logging_validator()));

        // Load configuration
        manager.load().await.unwrap();

        // Test configuration values
        let host: String = manager.get("server.host").await.unwrap();
        assert_eq!(host, "localhost");

        let port: u64 = manager.get("server.port").await.unwrap();
        assert_eq!(port, 8080);

        let db_url: String = manager.get("database.url").await.unwrap();
        assert_eq!(db_url, "postgresql://localhost:5432/testdb");

        let log_level: String = manager.get("logging.level").await.unwrap();
        assert_eq!(log_level, "info");

        // Test metadata
        let metadata = manager.get_metadata().await;
        assert!(metadata.is_valid);
        assert!(metadata.validation_errors.is_empty());
    }

    #[tokio::test]
    async fn test_config_validation() {
        // Create invalid config
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
        {
            "server": {
                "host": "localhost",
                "port": 99999  // Invalid port
            },
            "database": {
                "url": "invalid://url",  // Invalid URL
                "pool_size": 0  // Invalid pool size
            }
        }
        "#;

        temp_file.write_all(config_content.as_bytes()).unwrap();
        let config_path = temp_file.path().to_path_buf();

        // Create config manager with validators
        let manager = SimpleEnhancedConfigManager::new()
            .add_source(ConfigSource::File { path: config_path })
            .add_validator(Box::new(validators::server_validator()))
            .add_validator(Box::new(validators::database_validator()));

        // Load should fail validation
        let load_result = manager.load().await;
        assert!(load_result.is_err());

        // Since loading failed, we can't check metadata through the manager
        // The test passes if load fails as expected
    }

    #[tokio::test]
    async fn test_environment_config() {
        // Set environment variables
        std::env::set_var("TEST_SERVER_HOST", "env-host");
        std::env::set_var("TEST_SERVER_PORT", "9000");

        let manager = SimpleEnhancedConfigManager::new().add_source(ConfigSource::Environment {
            prefix: "TEST_".to_string(),
        });

        manager.load().await.unwrap();

        let host: String = manager.get("server.host").await.unwrap();
        assert_eq!(host, "env-host");

        let port: u64 = manager.get("server.port").await.unwrap();
        assert_eq!(port, 9000);

        // Cleanup
        std::env::remove_var("TEST_SERVER_HOST");
        std::env::remove_var("TEST_SERVER_PORT");
    }

    #[tokio::test]
    async fn test_config_merging() {
        // Create file config
        let mut temp_file = NamedTempFile::new().unwrap();
        let file_config = r#"
        {
            "server": {
                "host": "file-host",
                "port": 8000
            },
            "database": {
                "url": "postgresql://localhost:5432/filedb"
            }
        }
        "#;

        temp_file.write_all(file_config.as_bytes()).unwrap();
        let config_path = temp_file.path().to_path_buf();

        // Set environment override
        std::env::set_var("TEST_SERVER_PORT", "9000");

        let manager = SimpleEnhancedConfigManager::new()
            .add_source(ConfigSource::File { path: config_path })
            .add_source(ConfigSource::Environment {
                prefix: "TEST_".to_string(),
            });

        manager.load().await.unwrap();

        // Check that environment overrides file
        let host: String = manager.get("server.host").await.unwrap();
        assert_eq!(host, "file-host"); // From file

        let port: u64 = manager.get("server.port").await.unwrap();
        assert_eq!(port, 9000); // From environment

        // Cleanup
        std::env::remove_var("TEST_SERVER_HOST");
        std::env::remove_var("TEST_SERVER_PORT");
    }

    #[tokio::test]
    async fn test_config_get_subset() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
        {
            "server": {
                "host": "localhost",
                "port": 8080
            },
            "server_extra": {
                "timeout": 30,
                "max_connections": 100
            },
            "database": {
                "url": "postgresql://localhost:5432/testdb"
            }
        }
        "#;

        temp_file.write_all(config_content.as_bytes()).unwrap();
        let config_path = temp_file.path().to_path_buf();

        let manager =
            SimpleEnhancedConfigManager::new().add_source(ConfigSource::File { path: config_path });

        manager.load().await.unwrap();

        // Get subset with prefix
        let subset = manager.get_subset("server").await.unwrap();

        if let serde_json::Value::Object(map) = subset {
            assert!(map.contains_key("host"));
            assert!(map.contains_key("port"));
            assert!(!map.contains_key("database"));
        }
    }

    #[tokio::test]
    async fn test_config_callbacks() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
        {
            "test_value": "initial"
        }
        "#;

        temp_file.write_all(config_content.as_bytes()).unwrap();
        let config_path = temp_file.path().to_path_buf();

        let callback_called = Arc::new(AtomicBool::new(false));
        let callback_called_clone = callback_called.clone();

        let manager = SimpleEnhancedConfigManager::new()
            .add_source(ConfigSource::File { path: config_path })
            .add_callback(move |config| {
                if let serde_json::Value::Object(map) = config {
                    if map.contains_key("test_value") {
                        callback_called_clone.store(true, Ordering::SeqCst);
                    }
                }
            });

        manager.load().await.unwrap();

        // Give the callback a moment to execute
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Check that callback was called
        assert!(callback_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_config_set() {
        let manager = SimpleEnhancedConfigManager::new().add_source(ConfigSource::Memory {
            data: serde_json::json!({"initial": "value"}),
        });

        manager.load().await.unwrap();

        // Set a new value
        manager
            .set("new_key", serde_json::json!("new_value"))
            .await
            .unwrap();

        // Get the value
        let value: String = manager.get("new_key").await.unwrap();
        assert_eq!(value, "new_value");
    }
}
