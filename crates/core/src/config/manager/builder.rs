//! Configuration builder pattern
//!
//! This module provides a fluent builder interface for creating configuration managers.

use super::{
    reloading::ReloadStrategy, sources::ConfigSource, validation::ConfigValidator,
    UnifiedConfigManager,
};

/// Configuration builder for fluent interface
pub struct ConfigBuilder {
    manager: UnifiedConfigManager,
}

impl ConfigBuilder {
    /// Create new configuration builder
    pub fn new() -> Self {
        Self {
            manager: UnifiedConfigManager::new(),
        }
    }

    /// Add configuration source
    pub fn add_source(mut self, source: ConfigSource) -> Self {
        self.manager = self.manager.add_source(source);
        self
    }

    /// Add configuration validator
    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.manager = self.manager.add_validator(validator);
        self
    }

    /// Set reload strategy
    pub fn with_reload_strategy(mut self, strategy: ReloadStrategy) -> Self {
        self.manager = self.manager.with_reload_strategy(strategy);
        self
    }

    /// Add configuration change callback
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&serde_json::Value) + Send + Sync + 'static,
    {
        self.manager = self.manager.add_callback(callback);
        self
    }

    /// Build the configuration manager
    pub fn build(self) -> UnifiedConfigManager {
        self.manager
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::manager::validation::BasicConfigValidator;
    use serde_json::json;

    #[test]
    fn test_config_builder() {
        let manager = ConfigBuilder::new()
            .add_source(ConfigSource::Memory {
                data: json!({"test": "value"}),
            })
            .build();

        // Basic test that the builder creates a manager
        assert_eq!(manager.sources().len(), 1);
    }

    #[test]
    fn test_config_builder_with_validator() {
        let validator = BasicConfigValidator::new().required_field("test".to_string());

        let manager = ConfigBuilder::new()
            .add_source(ConfigSource::Memory {
                data: json!({"test": "value"}),
            })
            .add_validator(Box::new(validator))
            .build();

        assert_eq!(manager.sources().len(), 1);
        assert_eq!(manager.validators().len(), 1);
    }

    #[test]
    fn test_config_builder_with_reload_strategy() {
        let manager = ConfigBuilder::new()
            .add_source(ConfigSource::Memory {
                data: json!({"test": "value"}),
            })
            .with_reload_strategy(ReloadStrategy::Auto { debounce_ms: 1000 })
            .build();

        assert_eq!(manager.sources().len(), 1);
        assert!(matches!(
            manager.reload_strategy(),
            ReloadStrategy::Auto { .. }
        ));
    }
}
