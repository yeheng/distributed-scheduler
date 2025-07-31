use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::SchedulerError;

/// Environment-specific configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

impl Environment {
    /// Parse environment from string
    pub fn from_str(env: &str) -> Result<Self, SchedulerError> {
        match env.to_lowercase().as_str() {
            "development" | "dev" => Ok(Environment::Development),
            "testing" | "test" => Ok(Environment::Testing),
            "staging" | "stage" => Ok(Environment::Staging),
            "production" | "prod" => Ok(Environment::Production),
            _ => Err(SchedulerError::Configuration(format!(
                "Invalid environment: {env}"
            ))),
        }
    }

    /// Get current environment from environment variable
    pub fn current() -> Result<Self, SchedulerError> {
        std::env::var("APP_ENV")
            .map(|s| Self::from_str(&s))
            .unwrap_or(Ok(Environment::Development))
    }

    /// Get environment-specific defaults
    pub fn get_defaults(&self) -> HashMap<String, serde_json::Value> {
        let mut defaults = HashMap::new();
        
        match self {
            Environment::Development => {
                defaults.insert("log_level".to_string(), serde_json::Value::String("debug".to_string()));
                defaults.insert("debug_mode".to_string(), serde_json::Value::Bool(true));
                defaults.insert("database_pool_size".to_string(), serde_json::Value::Number(serde_json::Number::from(5)));
            }
            Environment::Testing => {
                defaults.insert("log_level".to_string(), serde_json::Value::String("info".to_string()));
                defaults.insert("debug_mode".to_string(), serde_json::Value::Bool(false));
                defaults.insert("database_pool_size".to_string(), serde_json::Value::Number(serde_json::Number::from(10)));
            }
            Environment::Staging => {
                defaults.insert("log_level".to_string(), serde_json::Value::String("warn".to_string()));
                defaults.insert("debug_mode".to_string(), serde_json::Value::Bool(false));
                defaults.insert("database_pool_size".to_string(), serde_json::Value::Number(serde_json::Number::from(20)));
            }
            Environment::Production => {
                defaults.insert("log_level".to_string(), serde_json::Value::String("error".to_string()));
                defaults.insert("debug_mode".to_string(), serde_json::Value::Bool(false));
                defaults.insert("database_pool_size".to_string(), serde_json::Value::Number(serde_json::Number::from(50)));
            }
        }
        
        defaults
    }

    /// Check if environment is production
    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }

    /// Check if environment is development
    pub fn is_development(&self) -> bool {
        matches!(self, Environment::Development)
    }

    /// Get environment display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Environment::Development => "Development",
            Environment::Testing => "Testing",
            Environment::Staging => "Staging",
            Environment::Production => "Production",
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Configuration profile for different deployment scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigProfile {
    /// Profile name
    pub name: String,
    /// Environment
    pub environment: Environment,
    /// Profile-specific overrides
    pub overrides: HashMap<String, serde_json::Value>,
    /// Feature flags
    pub features: HashMap<String, bool>,
}

impl ConfigProfile {
    /// Create new configuration profile
    pub fn new(name: String, environment: Environment) -> Self {
        Self {
            name,
            environment,
            overrides: HashMap::new(),
            features: HashMap::new(),
        }
    }

    /// Add configuration override
    pub fn with_override(mut self, key: String, value: serde_json::Value) -> Self {
        self.overrides.insert(key, value);
        self
    }

    /// Add feature flag
    pub fn with_feature(mut self, feature: String, enabled: bool) -> Self {
        self.features.insert(feature, enabled);
        self
    }

    /// Get merged configuration (environment defaults + profile overrides)
    pub fn get_merged_config(&self) -> HashMap<String, serde_json::Value> {
        let mut config = self.environment.get_defaults();
        
        // Apply profile overrides
        for (key, value) in &self.overrides {
            config.insert(key.clone(), value.clone());
        }
        
        // Add environment info
        config.insert("environment".to_string(), serde_json::Value::String(self.environment.to_string()));
        config.insert("profile".to_string(), serde_json::Value::String(self.name.clone()));
        
        config
    }

    /// Check if feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.features.get(feature).copied().unwrap_or(false)
    }
}

/// Profile registry for managing multiple configuration profiles
pub struct ProfileRegistry {
    profiles: HashMap<String, ConfigProfile>,
    active_profile: Option<String>,
}

impl ProfileRegistry {
    /// Create new profile registry
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            active_profile: None,
        }
    }

    /// Add profile to registry
    pub fn add_profile(mut self, profile: ConfigProfile) -> Self {
        self.profiles.insert(profile.name.clone(), profile);
        self
    }

    /// Set active profile
    pub fn set_active_profile(&mut self, profile_name: &str) -> Result<(), SchedulerError> {
        if !self.profiles.contains_key(profile_name) {
            return Err(SchedulerError::Configuration(format!(
                "Profile '{}' not found",
                profile_name
            )));
        }
        
        self.active_profile = Some(profile_name.to_string());
        Ok(())
    }

    /// Get active profile
    pub fn get_active_profile(&self) -> Option<&ConfigProfile> {
        self.active_profile
            .as_ref()
            .and_then(|name| self.profiles.get(name))
    }

    /// Get profile by name
    pub fn get_profile(&self, name: &str) -> Option<&ConfigProfile> {
        self.profiles.get(name)
    }

    /// List all available profiles
    pub fn list_profiles(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }

    /// Get active configuration
    pub fn get_active_config(&self) -> Option<HashMap<String, serde_json::Value>> {
        self.get_active_profile()
            .map(|profile| profile.get_merged_config())
    }
}

impl Default for ProfileRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        
        // Add default profiles
        registry = registry.add_profile(
            ConfigProfile::new("development".to_string(), Environment::Development)
                .with_feature("hot_reload".to_string(), true)
                .with_feature("debug_endpoints".to_string(), true)
        );
        
        registry = registry.add_profile(
            ConfigProfile::new("testing".to_string(), Environment::Testing)
                .with_feature("test_mode".to_string(), true)
                .with_feature("mock_services".to_string(), true)
        );
        
        registry = registry.add_profile(
            ConfigProfile::new("production".to_string(), Environment::Production)
                .with_feature("monitoring".to_string(), true)
                .with_feature("circuit_breaker".to_string(), true)
        );
        
        // Set default active profile based on current environment
        if let Ok(env) = Environment::current() {
            let profile_name = match env {
                Environment::Development => "development",
                Environment::Testing => "testing",
                Environment::Staging => "staging",
                Environment::Production => "production",
            };
            
            let _ = registry.set_active_profile(profile_name);
        }
        
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_from_str() {
        assert_eq!(Environment::from_str("dev").unwrap(), Environment::Development);
        assert_eq!(Environment::from_str("production").unwrap(), Environment::Production);
        assert!(Environment::from_str("invalid").is_err());
    }

    #[test]
    fn test_environment_defaults() {
        let dev_defaults = Environment::Development.get_defaults();
        assert_eq!(dev_defaults.get("log_level").unwrap(), "debug");
        assert_eq!(dev_defaults.get("debug_mode").unwrap(), true);
        
        let prod_defaults = Environment::Production.get_defaults();
        assert_eq!(prod_defaults.get("log_level").unwrap(), "error");
        assert_eq!(prod_defaults.get("debug_mode").unwrap(), false);
    }

    #[test]
    fn test_config_profile() {
        let profile = ConfigProfile::new("test".to_string(), Environment::Development)
            .with_override("custom_key".to_string(), serde_json::Value::String("custom_value".to_string()))
            .with_feature("test_feature".to_string(), true);
        
        let config = profile.get_merged_config();
        assert_eq!(config.get("log_level").unwrap(), "debug");
        assert_eq!(config.get("custom_key").unwrap(), "custom_value");
        assert!(profile.is_feature_enabled("test_feature"));
        assert!(!profile.is_feature_enabled("nonexistent"));
    }

    #[test]
    fn test_profile_registry() {
        let mut registry = ProfileRegistry::default();
        
        assert!(registry.get_active_profile().is_some());
        assert!(registry.list_profiles().contains(&"development"));
        assert!(registry.list_profiles().contains(&"production"));
        
        let active_config = registry.get_active_config();
        assert!(active_config.is_some());
        
        // Test setting active profile
        let result = registry.set_active_profile("production");
        assert!(result.is_ok());
        assert_eq!(registry.get_active_profile().unwrap().environment, Environment::Production);
    }
}