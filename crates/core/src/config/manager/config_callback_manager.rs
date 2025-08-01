//! Configuration callback manager - Handles configuration change callbacks
//!
//! This component is responsible only for managing configuration change callbacks
//! and notifying them when configuration changes.

use serde_json::Value;

/// Type alias for configuration callback function to reduce type complexity
pub type ConfigCallback = Box<dyn Fn(&Value) + Send + Sync>;

/// Configuration callback manager - Handles configuration change callbacks
/// Follows SRP: Only responsible for callback management
pub struct ConfigCallbackManager {
    /// Configuration change callbacks
    callbacks: Vec<ConfigCallback>,
}

impl ConfigCallbackManager {
    /// Create a new callback manager
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
        }
    }

    /// Add a configuration change callback
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&Value) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    /// Add a configuration change callback (mutable version)
    pub fn add_callback_mut<F>(&mut self, callback: F)
    where
        F: Fn(&Value) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
    }

    /// Notify all callbacks of configuration change
    pub fn notify_callbacks(&self, config: &Value) {
        for callback in &self.callbacks {
            callback(config);
        }
    }

    /// Get number of callbacks
    pub fn callback_count(&self) -> usize {
        self.callbacks.len()
    }

    /// Clear all callbacks
    pub fn clear_callbacks(&mut self) {
        self.callbacks.clear();
    }
}

impl Default for ConfigCallbackManager {
    fn default() -> Self {
        Self::new()
    }
}
