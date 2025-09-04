use crate::validation::ConfigValidator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    pub enabled: bool,
    pub executors: HashMap<String, ExecutorInstanceConfig>,
    pub default_executor: Option<String>,
    pub executor_factory: ExecutorFactoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorInstanceConfig {
    pub executor_type: String,
    pub config: Option<serde_json::Value>,
    pub supported_task_types: Vec<String>,
    pub priority: i32,
    pub max_concurrent_tasks: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub retry_config: Option<RetryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorFactoryConfig {
    pub auto_discovery: bool,
    pub discovery_path: Option<String>,
    pub dynamic_loading: bool,
    pub plugin_directories: Vec<String>,
    pub validation_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub jitter_enabled: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        let mut executors = HashMap::new();

        executors.insert(
            "shell".to_string(),
            ExecutorInstanceConfig {
                executor_type: "shell".to_string(),
                config: None,
                supported_task_types: vec!["shell".to_string()],
                priority: 100,
                max_concurrent_tasks: Some(10),
                timeout_seconds: Some(300),
                retry_config: Some(RetryConfig::default()),
            },
        );

        executors.insert(
            "http".to_string(),
            ExecutorInstanceConfig {
                executor_type: "http".to_string(),
                config: None,
                supported_task_types: vec!["http".to_string()],
                priority: 90,
                max_concurrent_tasks: Some(20),
                timeout_seconds: Some(60),
                retry_config: Some(RetryConfig::default()),
            },
        );

        Self {
            enabled: true,
            executors,
            default_executor: Some("shell".to_string()),
            executor_factory: ExecutorFactoryConfig::default(),
        }
    }
}

impl Default for ExecutorFactoryConfig {
    fn default() -> Self {
        Self {
            auto_discovery: false,
            discovery_path: Some("./executors".to_string()),
            dynamic_loading: false,
            plugin_directories: vec!["./plugins".to_string()],
            validation_enabled: true,
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            jitter_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        assert!(config.enabled);
        assert_eq!(config.executors.len(), 2);
        assert!(config.executors.contains_key("shell"));
        assert!(config.executors.contains_key("http"));
        assert_eq!(config.default_executor, Some("shell".to_string()));
    }

    #[test]
    fn test_executor_instance_config() {
        let config = ExecutorInstanceConfig {
            executor_type: "test".to_string(),
            config: None,
            supported_task_types: vec!["test".to_string()],
            priority: 50,
            max_concurrent_tasks: Some(5),
            timeout_seconds: Some(120),
            retry_config: None,
        };

        assert_eq!(config.executor_type, "test");
        assert_eq!(config.priority, 50);
        assert_eq!(config.max_concurrent_tasks, Some(5));
    }

    #[test]
    fn test_retry_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.jitter_enabled);
    }

    #[test]
    fn test_executor_factory_config() {
        let config = ExecutorFactoryConfig::default();
        assert!(!config.auto_discovery);
        assert!(!config.dynamic_loading);
        assert!(config.validation_enabled);
        assert_eq!(config.plugin_directories.len(), 1);
        assert_eq!(config.plugin_directories[0], "./plugins");
    }

    #[test]
    fn test_executor_config_validation() {
        let config = ExecutorConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_executor_config_validation_empty_executors() {
        let mut config = ExecutorConfig::default();
        config.executors.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_executor_config_validation_invalid_default() {
        let mut config = ExecutorConfig::default();
        config.default_executor = Some("nonexistent".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_executor_config_validation_empty_executor_type() {
        let mut config = ExecutorConfig::default();
        if let Some(executor) = config.executors.get_mut("shell") {
            executor.executor_type = "".to_string();
        }
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_executor_config_validation_empty_task_types() {
        let mut config = ExecutorConfig::default();
        if let Some(executor) = config.executors.get_mut("shell") {
            executor.supported_task_types.clear();
        }
        assert!(config.validate().is_err());
    }
}

impl ConfigValidator for ExecutorConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        if !self.enabled {
            return Ok(());
        }

        // Validate that we have at least one executor
        if self.executors.is_empty() {
            return Err(crate::ConfigError::Validation(
                "At least one executor must be configured".to_string(),
            ));
        }

        // Validate default executor exists
        if let Some(ref default_executor) = self.default_executor {
            if !self.executors.contains_key(default_executor) {
                return Err(crate::ConfigError::Validation(format!(
                    "Default executor '{default_executor}' not found in executor configurations"
                )));
            }
        }

        // Validate each executor configuration
        for (name, executor_config) in &self.executors {
            if executor_config.executor_type.is_empty() {
                return Err(crate::ConfigError::Validation(format!(
                    "Executor '{name}' has empty executor_type"
                )));
            }

            if executor_config.supported_task_types.is_empty() {
                return Err(crate::ConfigError::Validation(format!(
                    "Executor '{name}' has no supported task types"
                )));
            }

            // Validate retry configuration if present
            if let Some(ref retry_config) = executor_config.retry_config {
                if retry_config.max_retries == 0 {
                    return Err(crate::ConfigError::Validation(format!(
                        "Executor '{name}' has max_retries set to 0"
                    )));
                }
                if retry_config.base_delay_ms == 0 {
                    return Err(crate::ConfigError::Validation(format!(
                        "Executor '{name}' has base_delay_ms set to 0"
                    )));
                }
                if retry_config.backoff_multiplier <= 1.0 && retry_config.max_retries > 1 {
                    return Err(crate::ConfigError::Validation(format!(
                        "Executor '{name}' has backoff_multiplier <= 1.0 with multiple retries"
                    )));
                }
            }
        }

        Ok(())
    }
}
