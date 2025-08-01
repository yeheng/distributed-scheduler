use std::collections::HashMap;
use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::{SchedulerError, SchedulerResult};

use super::{ConfigSource, ConfigValue};

/// Configuration loading error
#[derive(Debug, Error)]
pub enum ConfigLoadError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },
    #[error("Parse error: {message}")]
    ParseError { message: String },
    #[error("Invalid format: {message}")]
    InvalidFormat { message: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration source type
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSourceType {
    Toml,
    Json,
    Yaml,
    Env,
}

/// Configuration loader trait
pub trait ConfigLoader: Send + Sync {
    fn load(&self, source: &ConfigSource) -> SchedulerResult<HashMap<String, ConfigValue>>;
    fn load_from_file(&self, path: &Path) -> SchedulerResult<HashMap<String, ConfigValue>>;
    fn load_from_env(&self, prefix: &str) -> SchedulerResult<HashMap<String, ConfigValue>>;
}

/// File-based configuration loader
pub struct FileConfigLoader {
    _supported_formats: Vec<ConfigSourceType>,
    validators: Vec<Box<dyn super::validation::ConfigValidator>>,
}

impl FileConfigLoader {
    pub fn new() -> Self {
        Self {
            _supported_formats: vec![ConfigSourceType::Toml, ConfigSourceType::Json],
            validators: Vec::new(),
        }
    }

    pub fn with_validator(
        mut self,
        validator: Box<dyn super::validation::ConfigValidator>,
    ) -> Self {
        self.validators.push(validator);
        self
    }

    fn load_toml_file(&self, path: &Path) -> SchedulerResult<HashMap<String, ConfigValue>> {
        let content = fs::read_to_string(path)
            .map_err(|e| SchedulerError::Configuration(format!("Failed to read file: {e}")))?;

        let config: HashMap<String, serde_json::Value> = toml::from_str(&content)
            .map_err(|e| SchedulerError::Configuration(format!("TOML parse error: {e}")))?;

        self.convert_to_config_values(
            config,
            ConfigSource::File(path.to_string_lossy().to_string()),
        )
    }

    fn load_json_file(&self, path: &Path) -> SchedulerResult<HashMap<String, ConfigValue>> {
        let content = fs::read_to_string(path)
            .map_err(|e| SchedulerError::Configuration(format!("Failed to read file: {e}")))?;

        let config: HashMap<String, serde_json::Value> = serde_json::from_str(&content)
            .map_err(|e| SchedulerError::Configuration(format!("JSON parse error: {e}")))?;

        self.convert_to_config_values(
            config,
            ConfigSource::File(path.to_string_lossy().to_string()),
        )
    }

    fn convert_to_config_values(
        &self,
        config: HashMap<String, serde_json::Value>,
        source: ConfigSource,
    ) -> SchedulerResult<HashMap<String, ConfigValue>> {
        let mut result = HashMap::new();
        let timestamp = std::time::SystemTime::now();

        for (key, value) in config {
            result.insert(
                key,
                ConfigValue {
                    value,
                    source: source.clone(),
                    last_updated: timestamp,
                },
            );
        }

        // Validate configuration
        for validator in &self.validators {
            let config_json = serde_json::to_value(result.clone()).map_err(|e| {
                SchedulerError::Configuration(format!(
                    "Failed to serialize config for validation: {e}"
                ))
            })?;

            validator
                .validate(&config_json)
                .map_err(|e| SchedulerError::Configuration(format!("Validation failed: {e}")))?;
        }

        Ok(result)
    }
}

impl ConfigLoader for FileConfigLoader {
    fn load(&self, source: &ConfigSource) -> SchedulerResult<HashMap<String, ConfigValue>> {
        match source {
            ConfigSource::File(path) => {
                let path = Path::new(path);
                let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

                match extension {
                    "toml" => self.load_toml_file(path),
                    "json" => self.load_json_file(path),
                    _ => Err(SchedulerError::Configuration(format!(
                        "Unsupported file format: {extension}"
                    ))),
                }
            }
            ConfigSource::Environment => self.load_from_env("APP_"),
            _ => Err(SchedulerError::Configuration(
                "Unsupported config source".to_string(),
            )),
        }
    }

    fn load_from_file(&self, path: &Path) -> SchedulerResult<HashMap<String, ConfigValue>> {
        let source = ConfigSource::File(path.to_string_lossy().to_string());
        self.load(&source)
    }

    fn load_from_env(&self, prefix: &str) -> SchedulerResult<HashMap<String, ConfigValue>> {
        let mut result = HashMap::new();
        let timestamp = std::time::SystemTime::now();

        for (key, value) in std::env::vars() {
            if key.starts_with(prefix) {
                let config_key = key[prefix.len()..].to_lowercase().replace('_', ".");
                let json_value = serde_json::Value::String(value);

                result.insert(
                    config_key,
                    ConfigValue {
                        value: json_value,
                        source: ConfigSource::Environment,
                        last_updated: timestamp,
                    },
                );
            }
        }

        Ok(result)
    }
}

impl Default for FileConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-source configuration loader
pub struct MultiSourceLoader {
    loaders: Vec<Box<dyn ConfigLoader>>,
    merge_strategy: MergeStrategy,
}

#[derive(Debug, Clone)]
pub enum MergeStrategy {
    /// First source wins
    FirstWins,
    /// Last source wins
    LastWins,
    /// Merge with deep merge
    DeepMerge,
}

impl MultiSourceLoader {
    pub fn new() -> Self {
        Self {
            loaders: Vec::new(),
            merge_strategy: MergeStrategy::LastWins,
        }
    }

    pub fn add_loader(mut self, loader: Box<dyn ConfigLoader>) -> Self {
        self.loaders.push(loader);
        self
    }

    pub fn with_merge_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.merge_strategy = strategy;
        self
    }

    pub fn load_all(&self) -> SchedulerResult<HashMap<String, ConfigValue>> {
        let mut result = HashMap::new();

        for loader in &self.loaders {
            let config = loader.load(&ConfigSource::Default)?;

            match self.merge_strategy {
                MergeStrategy::FirstWins => {
                    for (key, value) in config {
                        result.entry(key).or_insert(value);
                    }
                }
                MergeStrategy::LastWins => {
                    result.extend(config);
                }
                MergeStrategy::DeepMerge => {
                    for (key, value) in config {
                        if let Some(existing) = result.get_mut(&key) {
                            if let Some(merged) = self.deep_merge(&existing.value, &value.value) {
                                existing.value = merged;
                                existing.last_updated = value.last_updated;
                                existing.source = value.source;
                            } else {
                                *existing = value;
                            }
                        } else {
                            result.insert(key, value);
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn deep_merge(
        &self,
        base: &serde_json::Value,
        override_val: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        match (base, override_val) {
            (serde_json::Value::Object(base_map), serde_json::Value::Object(override_map)) => {
                let mut merged = base_map.clone();
                for (key, value) in override_map {
                    if let Some(base_value) = base_map.get(key) {
                        if let Some(merged_value) = self.deep_merge(base_value, value) {
                            merged.insert(key.clone(), merged_value);
                        } else {
                            merged.insert(key.clone(), value.clone());
                        }
                    } else {
                        merged.insert(key.clone(), value.clone());
                    }
                }
                Some(serde_json::Value::Object(merged))
            }
            _ => None,
        }
    }
}

impl Default for MultiSourceLoader {
    fn default() -> Self {
        Self::new()
    }
}
