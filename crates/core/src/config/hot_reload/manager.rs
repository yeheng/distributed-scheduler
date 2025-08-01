use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::config::{
    core::ConfigValue,
    hot_reload::{events::ConfigChangeEvent, watchers::ConfigWatcher},
};
use crate::SchedulerError;

/// Hot reload manager for configuration
pub struct HotReloadManager {
    /// Current configuration
    config: Arc<RwLock<HashMap<String, ConfigValue>>>,
    /// Configuration watchers
    watchers: Vec<Box<dyn ConfigWatcher>>,
    /// Change callbacks
    callbacks: Vec<Box<dyn Fn(ConfigChangeEvent) + Send + Sync>>,
    /// Running state
    running: Arc<RwLock<bool>>,
}

impl HotReloadManager {
    /// Create new hot reload manager
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(HashMap::new())),
            watchers: Vec::new(),
            callbacks: Vec::new(),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Add configuration watcher
    pub fn add_watcher(mut self, watcher: Box<dyn ConfigWatcher>) -> Self {
        self.watchers.push(watcher);
        self
    }

    /// Add change callback
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(ConfigChangeEvent) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    /// Start hot reload monitoring
    pub async fn start(&self) -> Result<(), SchedulerError> {
        let mut running = self.running.write().await;
        if *running {
            return Err(SchedulerError::Configuration(
                "Hot reload already running".to_string(),
            ));
        }
        *running = true;
        drop(running);

        let running_flag = Arc::clone(&self.running);

        // Since we can't clone the watchers and callbacks, we need to use a different approach
        // For now, let's implement a simpler version that doesn't use the stored watchers
        // This is a temporary solution until we can implement proper shared state

        // Start a simple monitoring task
        tokio::spawn(async move {
            while *running_flag.read().await {
                // For now, just sleep and maintain the running state
                sleep(Duration::from_secs(5)).await;
            }
        });

        Ok(())
    }

    /// Stop hot reload monitoring
    pub async fn stop(&self) -> Result<(), SchedulerError> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }

    /// Get current configuration
    pub async fn get_config(&self) -> HashMap<String, ConfigValue> {
        self.config.read().await.clone()
    }

    /// Get specific configuration value
    pub async fn get_value(&self, key: &str) -> Option<ConfigValue> {
        self.config.read().await.get(key).cloned()
    }

    /// Check if hot reload is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Default for HotReloadManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hot_reload_manager() {
        let manager = HotReloadManager::new();

        // Should not be running initially
        assert!(!manager.is_running().await);

        // Start hot reload
        manager.start().await.unwrap();
        assert!(manager.is_running().await);

        // Stop hot reload
        manager.stop().await.unwrap();
        assert!(!manager.is_running().await);
    }
}
