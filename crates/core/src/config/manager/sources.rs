//! Configuration source implementations
//!
//! This module provides various configuration sources including files,
//! environment variables, command-line arguments, and remote configurations.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::{errors::SchedulerError, SchedulerResult};

/// Configuration source enum
///
/// Defines all supported configuration source types.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// File-based configuration source
    File { path: PathBuf },

    /// Environment variable configuration source
    Environment { prefix: String },

    /// Command-line argument configuration source
    CommandLine { args: Vec<String> },

    /// Remote configuration source
    Remote { url: String },

    /// In-memory configuration source
    Memory { data: Value },
}

impl ConfigSource {
    /// Load configuration from this source
    pub async fn load(&self) -> SchedulerResult<Value> {
        match self {
            ConfigSource::File { path } => Self::load_from_file(path).await,
            ConfigSource::Environment { prefix } => Self::load_from_env(prefix),
            ConfigSource::CommandLine { args } => Self::load_from_args(args),
            ConfigSource::Remote { url } => Self::load_from_remote(url).await,
            ConfigSource::Memory { data } => Ok(data.clone()),
        }
    }

    /// Load configuration from file
    async fn load_from_file(path: &PathBuf) -> SchedulerResult<Value> {
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

        let config: Value = serde_json::from_str(&content).map_err(|e| {
            SchedulerError::Configuration(format!(
                "Failed to parse config file {}: {}",
                path.display(),
                e
            ))
        })?;

        Ok(config)
    }

    /// Load configuration from environment variables
    fn load_from_env(prefix: &str) -> SchedulerResult<Value> {
        let mut config = Value::Object(serde_json::Map::new());

        for (key, value) in std::env::vars() {
            if key.starts_with(prefix) {
                let config_key = key[prefix.len()..].to_lowercase().replace('_', ".");
                let parsed_value: Value =
                    serde_json::from_str(&value).unwrap_or_else(|_| Value::String(value.clone()));

                if let Value::Object(ref mut map) = config {
                    map.insert(config_key, parsed_value);
                }
            }
        }

        Ok(config)
    }

    /// Load configuration from command-line arguments
    fn load_from_args(args: &[String]) -> SchedulerResult<Value> {
        let mut config = Value::Object(serde_json::Map::new());
        let mut current_key = None;

        for arg in args {
            if arg.starts_with("--") {
                if let Some(key) = current_key {
                    // Set previous key as boolean flag
                    if let Value::Object(ref mut map) = config {
                        map.insert(key, Value::Bool(true));
                    }
                }
                current_key = Some(arg[2..].to_string());
            } else if let Some(key) = current_key.take() {
                // Set key with value
                let parsed_value: Value =
                    serde_json::from_str(arg).unwrap_or_else(|_| Value::String(arg.to_string()));

                if let Value::Object(ref mut map) = config {
                    map.insert(key, parsed_value);
                }
            }
        }

        // Handle last key if it's a boolean flag
        if let Some(key) = current_key {
            if let Value::Object(ref mut map) = config {
                map.insert(key, Value::Bool(true));
            }
        }

        Ok(config)
    }

    /// Load configuration from remote source
    async fn load_from_remote(_url: &str) -> SchedulerResult<Value> {
        // For now, return empty config
        // In a real implementation, this would make HTTP requests
        Ok(Value::Object(serde_json::Map::new()))
    }

    /// Check if this source supports file watching
    pub fn supports_watching(&self) -> bool {
        matches!(self, ConfigSource::File { .. })
    }

    /// Get the file path if this is a file source
    pub fn get_file_path(&self) -> Option<&PathBuf> {
        match self {
            ConfigSource::File { path } => Some(path),
            _ => None,
        }
    }
}

/// Configuration source merger
pub struct ConfigMerger;

impl ConfigMerger {
    /// Merge two configurations with support for flat keys
    pub fn merge(base: Value, override_config: Value) -> Value {
        match (base, override_config) {
            (Value::Object(mut base_map), Value::Object(override_map)) => {
                for (key, value) in override_map {
                    // Handle flat keys with dots (e.g., "server.port") by converting to nested structure
                    if key.contains('.') {
                        let parts: Vec<&str> = key.split('.').collect();
                        if parts.len() == 2 {
                            // Simple case: server.port -> {server: {port: value}}
                            let parent_key = parts[0];
                            let child_key = parts[1];

                            // Get or create parent object
                            let parent_obj = base_map
                                .entry(parent_key.to_string())
                                .or_insert_with(|| Value::Object(serde_json::Map::new()));

                            if let Value::Object(parent_map) = parent_obj {
                                parent_map.insert(child_key.to_string(), value);
                            }
                        } else {
                            // For complex nested keys, just insert as-is
                            base_map.insert(key, value);
                        }
                    } else {
                        // Handle nested merge for non-flat keys
                        if let (Some(base_value), Value::Object(override_obj)) =
                            (base_map.get(&key), &value)
                        {
                            if let Value::Object(base_obj) = base_value {
                                // Recursively merge nested objects
                                let merged = Self::merge(
                                    Value::Object(base_obj.clone()),
                                    Value::Object(override_obj.clone()),
                                );
                                base_map.insert(key, merged);
                                continue;
                            }
                        }
                        base_map.insert(key, value);
                    }
                }
                Value::Object(base_map)
            }
            (_, override_config) => override_config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_source() {
        let data = Value::Object(
            serde_json::json!({"test": "value"})
                .as_object()
                .unwrap()
                .clone(),
        );
        let source = ConfigSource::Memory { data };

        let result = source.load().await.unwrap();
        assert_eq!(result["test"], "value");
    }

    #[tokio::test]
    async fn test_environment_source() {
        std::env::set_var("TEST_APP_NAME", "test_app");
        std::env::set_var("TEST_APP_PORT", "8080");

        let source = ConfigSource::Environment {
            prefix: "TEST_".to_string(),
        };
        let result = source.load().await.unwrap();

        assert_eq!(result["app.name"], "test_app");
        assert_eq!(result["app.port"], "8080");

        // Clean up
        std::env::remove_var("TEST_APP_NAME");
        std::env::remove_var("TEST_APP_PORT");
    }

    #[tokio::test]
    async fn test_command_line_source() {
        let args = vec![
            "--host".to_string(),
            "localhost".to_string(),
            "--port".to_string(),
            "8080".to_string(),
            "--debug".to_string(),
        ];

        let source = ConfigSource::CommandLine { args };
        let result = source.load().await.unwrap();

        assert_eq!(result["host"], "localhost");
        assert_eq!(result["port"], "8080");
        assert_eq!(result["debug"], true);
    }

    #[test]
    fn test_config_merger() {
        let base = Value::Object(
            serde_json::json!({
                "server": {"host": "localhost"},
                "app": {"name": "test"}
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        let override_config = Value::Object(
            serde_json::json!({
                "server": {"port": 8080},
                "debug": true
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        let merged = ConfigMerger::merge(base, override_config);

        assert_eq!(merged["server"]["host"], "localhost");
        assert_eq!(merged["server"]["port"], 8080);
        assert_eq!(merged["app"]["name"], "test");
        assert_eq!(merged["debug"], true);
    }
}
