// Simplified configuration validation - only essential validation utilities
use thiserror::Error;

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
}

// Simple validation trait that each config struct should implement
pub trait ConfigValidator {
    fn validate(&self) -> std::result::Result<(), ConfigValidationError>;
}

// Basic config validator for common validation scenarios
pub struct BasicConfigValidator;

impl BasicConfigValidator {
    pub fn new() -> Self {
        Self
    }
    
    pub fn validate_required_string(&self, value: &Option<String>, field_name: &str) -> std::result::Result<(), ConfigValidationError> {
        validate_required_string(value, field_name)
    }
    
    pub fn validate_required_number<T: Into<f64> + Copy>(&self, value: T, field_name: &str, min: Option<f64>, max: Option<f64>) -> std::result::Result<(), ConfigValidationError> {
        validate_required_number(value, field_name, min, max)
    }
    
    pub fn validate_url(&self, url: &str, field_name: &str) -> std::result::Result<(), ConfigValidationError> {
        validate_url(url, field_name)
    }
    
    pub fn validate_positive_integer(&self, value: i32, field_name: &str) -> std::result::Result<(), ConfigValidationError> {
        validate_positive_integer(value, field_name)
    }
}

// Helper functions for common validation scenarios
pub fn validate_required_string(value: &Option<String>, field_name: &str) -> std::result::Result<(), ConfigValidationError> {
    match value {
        Some(s) if !s.trim().is_empty() => Ok(()),
        Some(_) => Err(ConfigValidationError::ValidationFailed {
            field: field_name.to_string(),
            message: format!("Field {} cannot be empty", field_name),
        }),
        None => Err(ConfigValidationError::RequiredFieldMissing {
            field: field_name.to_string(),
        }),
    }
}

pub fn validate_required_number<T: Into<f64> + Copy>(value: T, field_name: &str, min: Option<f64>, max: Option<f64>) -> std::result::Result<(), ConfigValidationError> {
    let num = value.into();
    
    if let Some(min_val) = min {
        if num < min_val {
            return Err(ConfigValidationError::ValidationFailed {
                field: field_name.to_string(),
                message: format!("Field {} must be at least {}", field_name, min_val),
            });
        }
    }
    
    if let Some(max_val) = max {
        if num > max_val {
            return Err(ConfigValidationError::ValidationFailed {
                field: field_name.to_string(),
                message: format!("Field {} must be at most {}", field_name, max_val),
            });
        }
    }
    
    Ok(())
}

pub fn validate_url(url: &str, field_name: &str) -> std::result::Result<(), ConfigValidationError> {
    if url.trim().is_empty() {
        return Err(ConfigValidationError::RequiredFieldMissing {
            field: field_name.to_string(),
        });
    }
    
    // Basic URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("postgres://") && !url.starts_with("sqlite:") {
        return Err(ConfigValidationError::ValidationFailed {
            field: field_name.to_string(),
            message: format!("Field {} must be a valid URL (http://, https://, postgres://, or sqlite:)", field_name),
        });
    }
    
    Ok(())
}

pub fn validate_positive_integer(value: i32, field_name: &str) -> std::result::Result<(), ConfigValidationError> {
    if value > 0 {
        Ok(())
    } else {
        Err(ConfigValidationError::ValidationFailed {
            field: field_name.to_string(),
            message: format!("Field {} must be a positive integer", field_name),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_required_string() {
        assert!(validate_required_string(&Some("test".to_string()), "field").is_ok());
        assert!(validate_required_string(&Some("".to_string()), "field").is_err());
        assert!(validate_required_string(&None, "field").is_err());
    }

    #[test]
    fn test_validate_required_number() {
        assert!(validate_required_number(5, "field", Some(1.0), Some(10.0)).is_ok());
        assert!(validate_required_number(0, "field", Some(1.0), Some(10.0)).is_err());
        assert!(validate_required_number(15, "field", Some(1.0), Some(10.0)).is_err());
        assert!(validate_required_number(5, "field", None, None).is_ok());
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("http://localhost:8080", "url").is_ok());
        assert!(validate_url("https://example.com", "url").is_ok());
        assert!(validate_url("postgres://localhost/db", "url").is_ok());
        assert!(validate_url("sqlite:test.db", "url").is_ok());
        assert!(validate_url("", "url").is_err());
        assert!(validate_url("invalid-url", "url").is_err());
    }

    #[test]
    fn test_validate_positive_integer() {
        assert!(validate_positive_integer(1, "field").is_ok());
        assert!(validate_positive_integer(0, "field").is_err());
        assert!(validate_positive_integer(-1, "field").is_err());
    }
}