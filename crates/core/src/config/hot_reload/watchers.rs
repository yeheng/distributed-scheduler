use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

use crate::config::{
    core::ConfigValue,
    hot_reload::{events::ConfigChangeEvent, ConfigChangeSource},
};
use crate::SchedulerError;

/// Configuration watcher trait
#[async_trait]
pub trait ConfigWatcher: Send + Sync {
    /// Wait for configuration change
    async fn wait_for_change(&mut self) -> Result<ConfigChangeEvent, SchedulerError>;

    /// Get current configuration
    async fn get_current_config(&self) -> Result<HashMap<String, ConfigValue>, SchedulerError>;

    /// Stop watching
    async fn stop(&mut self) -> Result<(), SchedulerError>;
}

/// File watcher for configuration files
pub struct FileConfigWatcher {
    /// File path to watch
    file_path: String,
    /// Last modification time
    last_modified: Option<std::time::SystemTime>,
    /// Polling interval
    polling_interval: Duration,
    /// Current configuration
    current_config: HashMap<String, ConfigValue>,
}

impl FileConfigWatcher {
    /// Create new file configuration watcher
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            last_modified: None,
            polling_interval: Duration::from_secs(5),
            current_config: HashMap::new(),
        }
    }

    /// Set polling interval
    pub fn with_polling_interval(mut self, interval: Duration) -> Self {
        self.polling_interval = interval;
        self
    }

    /// Load configuration from file
    async fn load_config(&self) -> Result<HashMap<String, ConfigValue>, SchedulerError> {
        let path = std::path::Path::new(&self.file_path);

        if !path.exists() {
            return Err(SchedulerError::Configuration(format!(
                "Configuration file not found: {}",
                self.file_path
            )));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| SchedulerError::Configuration(format!("Failed to read file: {e}")))?;

        let config: HashMap<String, serde_json::Value> =
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                toml::from_str(&content)
                    .map_err(|e| SchedulerError::Configuration(format!("TOML parse error: {e}")))?
            } else {
                serde_json::from_str(&content)
                    .map_err(|e| SchedulerError::Configuration(format!("JSON parse error: {e}")))?
            };

        let mut result = HashMap::new();
        let timestamp = std::time::SystemTime::now();

        for (key, value) in config {
            result.insert(
                key,
                ConfigValue {
                    value,
                    source: crate::config::core::ConfigSource::File(self.file_path.clone()),
                    last_updated: timestamp,
                },
            );
        }

        Ok(result)
    }

    /// Get file modification time
    fn get_file_modified_time(&self) -> Result<Option<std::time::SystemTime>, SchedulerError> {
        let path = std::path::Path::new(&self.file_path);

        if !path.exists() {
            return Ok(None);
        }

        let metadata = std::fs::metadata(path).map_err(|e| {
            SchedulerError::Configuration(format!("Failed to get file metadata: {e}"))
        })?;

        Ok(Some(metadata.modified().map_err(|e| {
            SchedulerError::Configuration(format!("Failed to get modified time: {e}"))
        })?))
    }

    fn find_config_changes(
        &self,
        old_config: &HashMap<String, ConfigValue>,
        new_config: &HashMap<String, ConfigValue>,
    ) -> Vec<ConfigChangeEvent> {
        let mut changes = Vec::new();
        let timestamp = std::time::SystemTime::now();

        // Check for added or modified keys
        for (key, new_value) in new_config {
            match old_config.get(key) {
                Some(old_value) if old_value.value != new_value.value => {
                    changes.push(ConfigChangeEvent {
                        key: key.clone(),
                        old_value: Some(old_value.clone()),
                        new_value: Some(new_value.clone()),
                        timestamp,
                        source: ConfigChangeSource::File(self.file_path.clone()),
                    });
                }
                None => {
                    changes.push(ConfigChangeEvent {
                        key: key.clone(),
                        old_value: None,
                        new_value: Some(new_value.clone()),
                        timestamp,
                        source: ConfigChangeSource::File(self.file_path.clone()),
                    });
                }
                _ => {}
            }
        }

        // Check for removed keys
        for key in old_config.keys() {
            if !new_config.contains_key(key) {
                changes.push(ConfigChangeEvent {
                    key: key.clone(),
                    old_value: old_config.get(key).cloned(),
                    new_value: None,
                    timestamp,
                    source: ConfigChangeSource::File(self.file_path.clone()),
                });
            }
        }

        changes
    }
}

#[async_trait]
impl ConfigWatcher for FileConfigWatcher {
    async fn wait_for_change(&mut self) -> Result<ConfigChangeEvent, SchedulerError> {
        loop {
            // Check for file changes
            if let Ok(current_modified) = self.get_file_modified_time() {
                if current_modified != self.last_modified {
                    // Load new configuration
                    let new_config = self.load_config().await?;

                    // Compare with current configuration to find changes
                    let changes = self.find_config_changes(&self.current_config, &new_config);

                    // Update state
                    self.last_modified = current_modified;
                    self.current_config = new_config;

                    // Return first change found
                    if let Some(change) = changes.into_iter().next() {
                        return Ok(change);
                    }
                }
            }

            // Wait before next check
            sleep(self.polling_interval).await;
        }
    }

    async fn get_current_config(&self) -> Result<HashMap<String, ConfigValue>, SchedulerError> {
        Ok(self.current_config.clone())
    }

    async fn stop(&mut self) -> Result<(), SchedulerError> {
        // Nothing to stop for file watcher
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_file_config_watcher() {
        // Create a temporary config file
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"{"test_key": "test_value", "number": 42}"#;
        temp_file.write_all(config_content.as_bytes()).unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();

        let mut watcher = FileConfigWatcher::new(file_path.clone());

        // Load initial config
        let initial_config = watcher.load_config().await.unwrap();
        assert_eq!(initial_config.get("test_key").unwrap().value, "test_value");

        // Get current config
        let current_config = watcher.get_current_config().await.unwrap();
        assert!(current_config.is_empty()); // Initially empty

        // Stop watcher
        watcher.stop().await.unwrap();
    }
}
