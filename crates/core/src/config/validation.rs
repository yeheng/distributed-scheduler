use std::collections::HashMap;

use serde_json::Value;
use thiserror::Error;

/// Configuration validation error
#[derive(Debug, Error)]
pub enum ConfigValidationError {
    #[error("Required field missing: {field}")]
    RequiredFieldMissing { field: String },
    #[error("Invalid value for field {field}: {value}, error: {error}")]
    InvalidValue {
        field: String,
        value: String,
        error: String,
    },
    #[error("Type mismatch for field {field}: expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("Validation failed for field {field}: {message}")]
    ValidationFailed { field: String, message: String },
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },
    #[error("Configuration parse error: {message}")]
    ParseError { message: String },
    #[error("Schema validation error: {message}")]
    SchemaError { message: String },
}

/// Configuration validation trait
pub trait ConfigValidator: Send + Sync {
    /// Validate configuration
    fn validate(&self, config: &Value) -> std::result::Result<(), ConfigValidationError>;

    /// Get validator name
    fn name(&self) -> &str;
}

/// Basic configuration validator
pub struct BasicConfigValidator {
    name: String,
    required_fields: Vec<String>,
    field_types: HashMap<String, String>,
    custom_validators: HashMap<
        String,
        Box<dyn Fn(&Value) -> std::result::Result<(), ConfigValidationError> + Send + Sync>,
    >,
}

impl BasicConfigValidator {
    /// Create new basic validator
    pub fn new(name: String) -> Self {
        Self {
            name,
            required_fields: Vec::new(),
            field_types: HashMap::new(),
            custom_validators: HashMap::new(),
        }
    }

    /// Add required field
    pub fn required_field(mut self, field: String, field_type: String) -> Self {
        self.required_fields.push(field.clone());
        self.field_types.insert(field, field_type);
        self
    }

    /// Add custom validator
    pub fn custom_validator<F>(mut self, field: String, validator: F) -> Self
    where
        F: Fn(&Value) -> std::result::Result<(), ConfigValidationError> + Send + Sync + 'static,
    {
        self.custom_validators.insert(field, Box::new(validator));
        self
    }

    /// Get nested value from configuration using dot notation
    fn get_nested_value<'a>(&self, config: &'a Value, key: &str) -> Option<&'a Value> {
        let keys: Vec<&str> = key.split('.').collect();
        let mut current = config;

        for k in &keys {
            match current {
                Value::Object(map) => {
                    if let Some(value) = map.get(*k) {
                        current = value;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current)
    }
}

impl ConfigValidator for BasicConfigValidator {
    fn validate(&self, config: &Value) -> std::result::Result<(), ConfigValidationError> {
        // Check required fields
        for field in &self.required_fields {
            if self.get_nested_value(config, field).is_none() {
                return Err(ConfigValidationError::RequiredFieldMissing {
                    field: field.clone(),
                });
            }
        }

        // Check field types
        for (field, expected_type) in &self.field_types {
            if let Some(value) = self.get_nested_value(config, field) {
                let actual_type = match value {
                    Value::Null => "null",
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                    Value::Object(_) => "object",
                };

                if actual_type != expected_type {
                    return Err(ConfigValidationError::TypeMismatch {
                        field: field.clone(),
                        expected: expected_type.clone(),
                        actual: actual_type.to_string(),
                    });
                }
            }
        }

        // Run custom validators
        for (field, validator) in &self.custom_validators {
            if let Some(value) = self.get_nested_value(config, field) {
                validator(value)?;
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Schema-based validator using JSON Schema
pub struct SchemaValidator {
    name: String,
    schema: Value,
}

impl SchemaValidator {
    pub fn new(name: String, schema: Value) -> Self {
        Self { name, schema }
    }
}

impl ConfigValidator for SchemaValidator {
    fn validate(&self, config: &Value) -> std::result::Result<(), ConfigValidationError> {
        // Basic schema validation - in production, use a proper JSON Schema library
        if let Some(schema_obj) = self.schema.as_object() {
            if let Some(required) = schema_obj.get("required").and_then(|r| r.as_array()) {
                for field in required {
                    if let Some(field_str) = field.as_str() {
                        if config.get(field_str).is_none() {
                            return Err(ConfigValidationError::RequiredFieldMissing {
                                field: field_str.to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Validator registry for managing multiple validators
pub struct ValidatorRegistry {
    validators: Vec<Box<dyn ConfigValidator>>,
}

impl ValidatorRegistry {
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn validate(&self, config: &Value) -> std::result::Result<(), ConfigValidationError> {
        for validator in &self.validators {
            validator.validate(config)?;
        }
        Ok(())
    }

    pub fn validator_names(&self) -> Vec<&str> {
        self.validators.iter().map(|v| v.name()).collect()
    }
}

impl Default for ValidatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
