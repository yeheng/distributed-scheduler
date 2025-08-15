use crate::validation::{ConfigValidator, ValidationUtils};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

impl ConfigValidator for DatabaseConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        ValidationUtils::validate_not_empty(&self.url, "database.url")?;

        if !self.url.starts_with("postgresql://") && !self.url.starts_with("postgres://") {
            return Err(crate::ConfigError::Validation(
                "database.url must start with postgresql:// or postgres://".to_string(),
            ));
        }

        ValidationUtils::validate_count(self.max_connections as usize, "database.max_connections")?;
        ValidationUtils::validate_count(self.min_connections as usize, "database.min_connections")?;

        if self.min_connections > self.max_connections {
            return Err(crate::ConfigError::Validation(
                "database.min_connections must be less than or equal to max_connections"
                    .to_string(),
            ));
        }

        ValidationUtils::validate_timeout_seconds(self.connection_timeout_seconds)?;
        ValidationUtils::validate_timeout_seconds(self.idle_timeout_seconds)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_config_validation() {
        let config = DatabaseConfig {
            url: "postgresql://localhost/test".to_string(),
            max_connections: 10,
            min_connections: 1,
            connection_timeout_seconds: 30,
            idle_timeout_seconds: 600,
        };

        assert!(config.validate().is_ok());

        // Test invalid URL
        let mut invalid_config = config.clone();
        invalid_config.url = "".to_string();
        assert!(invalid_config.validate().is_err());

        // Test invalid max_connections
        let mut invalid_config = config.clone();
        invalid_config.max_connections = 0;
        assert!(invalid_config.validate().is_err());

        // Test min_connections > max_connections
        let mut invalid_config = config.clone();
        invalid_config.min_connections = 15;
        invalid_config.max_connections = 10;
        assert!(invalid_config.validate().is_err());

        // Test invalid timeout
        let mut invalid_config = config.clone();
        invalid_config.connection_timeout_seconds = 0;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_database_config_serialization() {
        let config = DatabaseConfig {
            url: "postgresql://localhost/test".to_string(),
            max_connections: 10,
            min_connections: 1,
            connection_timeout_seconds: 30,
            idle_timeout_seconds: 600,
        };

        let serialized = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: DatabaseConfig =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(config.url, deserialized.url);
        assert_eq!(config.max_connections, deserialized.max_connections);
        assert_eq!(config.min_connections, deserialized.min_connections);
    }
}
