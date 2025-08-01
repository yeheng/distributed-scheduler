//! Configuration validation framework
//!
//! This module provides a flexible validation system for configuration values.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::errors::SchedulerError;

/// Type alias for custom validator function to reduce type complexity
pub type CustomValidator = Box<dyn Fn(&Value) -> Result<(), ConfigValidationError> + Send + Sync>;

/// Configuration validation error
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Required field '{field}' is missing")]
    MissingField { field: String },

    #[error("Field '{field}' has invalid type: expected {expected}, got {actual}")]
    InvalidType {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("Field '{field}' failed validation: {message}")]
    CustomValidation { field: String, message: String },

    #[error("Configuration validation failed: {0}")]
    Other(String),
}

impl From<ConfigValidationError> for SchedulerError {
    fn from(err: ConfigValidationError) -> Self {
        SchedulerError::Configuration(err.to_string())
    }
}

/// Configuration validator trait
#[async_trait]
pub trait ConfigValidator: Send + Sync {
    /// Validate the configuration
    async fn validate(&self, config: &Value) -> Result<(), ConfigValidationError>;
}

/// Basic configuration validator with common validation rules
pub struct BasicConfigValidator {
    required_fields: Vec<String>,
    type_checks: HashMap<String, String>,
    custom_validators: Vec<CustomValidator>,
}

impl BasicConfigValidator {
    /// Create a new basic validator
    pub fn new() -> Self {
        Self {
            required_fields: Vec::new(),
            type_checks: HashMap::new(),
            custom_validators: Vec::new(),
        }
    }

    /// Add a required field
    pub fn required_field(mut self, field: String) -> Self {
        self.required_fields.push(field);
        self
    }

    /// Add a type check for a field
    pub fn type_check(mut self, field: String, expected_type: String) -> Self {
        self.type_checks.insert(field, expected_type);
        self
    }

    /// Add a custom validator
    pub fn custom_validator<F>(mut self, validator: F) -> Self
    where
        F: Fn(&Value) -> Result<(), ConfigValidationError> + Send + Sync + 'static,
    {
        self.custom_validators.push(Box::new(validator));
        self
    }

    /// Check if a field exists in the configuration
    fn field_exists(&self, config: &Value, field_path: &str) -> bool {
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current = config;

        for part in parts {
            match current {
                Value::Object(map) => {
                    if let Some(value) = map.get(part) {
                        current = value;
                    } else {
                        return false;
                    }
                }
                _ => return false,
            }
        }

        true
    }

    /// Get the type of a field
    fn get_field_type(&self, config: &Value, field_path: &str) -> Option<String> {
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current = config;

        for part in parts {
            match current {
                Value::Object(map) => {
                    if let Some(value) = map.get(part) {
                        current = value;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(
            match current {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            }
            .to_string(),
        )
    }
}

#[async_trait]
impl ConfigValidator for BasicConfigValidator {
    async fn validate(&self, config: &Value) -> Result<(), ConfigValidationError> {
        // Check required fields
        for field in &self.required_fields {
            if !self.field_exists(config, field) {
                return Err(ConfigValidationError::MissingField {
                    field: field.clone(),
                });
            }
        }

        // Check type constraints
        for (field, expected_type) in &self.type_checks {
            if let Some(actual_type) = self.get_field_type(config, field) {
                if actual_type != *expected_type {
                    return Err(ConfigValidationError::InvalidType {
                        field: field.clone(),
                        expected: expected_type.clone(),
                        actual: actual_type,
                    });
                }
            }
        }

        // Run custom validators
        for validator in &self.custom_validators {
            validator(config)?;
        }

        Ok(())
    }
}

impl Default for BasicConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_validator() {
        let config = Value::Object(
            serde_json::json!({
                "server": {
                    "host": "localhost",
                    "port": 8080
                },
                "debug": true
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        let validator = BasicConfigValidator::new()
            .required_field("server.host".to_string())
            .required_field("server.port".to_string())
            .type_check("server.port".to_string(), "number".to_string())
            .type_check("debug".to_string(), "boolean".to_string());

        let result = validator.validate(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_missing_field_validation() {
        let config = Value::Object(
            serde_json::json!({
                "server": {
                    "host": "localhost"
                }
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        let validator = BasicConfigValidator::new().required_field("server.port".to_string());

        let result = validator.validate(&config).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigValidationError::MissingField { .. }
        ));
    }
}
