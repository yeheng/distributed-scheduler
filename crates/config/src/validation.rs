use crate::ConfigResult;

/// Trait for configuration validation
pub trait ConfigValidator {
    fn validate(&self) -> ConfigResult<()>;
}

/// General validation utilities
pub struct ValidationUtils;

impl ValidationUtils {
    /// Validate that a string is not empty
    pub fn validate_not_empty(value: &str, field_name: &str) -> ConfigResult<()> {
        if value.trim().is_empty() {
            return Err(crate::ConfigError::Validation(
                format!("{field_name} cannot be empty"),
            ));
        }
        Ok(())
    }
    
    /// Validate that a port number is valid
    pub fn validate_port(port: u16) -> ConfigResult<()> {
        if port == 0 {
            return Err(crate::ConfigError::Validation(
                "port cannot be 0".to_string(),
            ));
        }
        Ok(())
    }
    
    /// Validate that a timeout is reasonable
    pub fn validate_timeout_seconds(timeout_seconds: u64) -> ConfigResult<()> {
        if timeout_seconds == 0 {
            return Err(crate::ConfigError::Validation(
                "timeout_seconds must be greater than 0".to_string(),
            ));
        }
        if timeout_seconds > 3600 {
            return Err(crate::ConfigError::Validation(
                "timeout_seconds must be less than or equal to 3600".to_string(),
            ));
        }
        Ok(())
    }
    
    /// Validate that a count is reasonable
    pub fn validate_count(count: usize, field_name: &str) -> ConfigResult<()> {
        if count == 0 {
            return Err(crate::ConfigError::Validation(
                format!("{field_name} must be greater than 0"),
            ));
        }
        if count > 10000 {
            return Err(crate::ConfigError::Validation(
                format!("{field_name} must be less than or equal to 10000"),
            ));
        }
        Ok(())
    }
    
    /// Validate that a URL has a valid format
    pub fn validate_url(url: &str, field_name: &str) -> ConfigResult<()> {
        if url.trim().is_empty() {
            return Err(crate::ConfigError::Validation(
                format!("{field_name} cannot be empty"),
            ));
        }
        
        // Basic URL format validation
        if !url.contains("://") {
            return Err(crate::ConfigError::Validation(
                format!("{field_name} must be a valid URL with protocol"),
            ));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_not_empty() {
        assert!(ValidationUtils::validate_not_empty("test", "field").is_ok());
        assert!(ValidationUtils::validate_not_empty("  test  ", "field").is_ok());
        assert!(ValidationUtils::validate_not_empty("", "field").is_err());
        assert!(ValidationUtils::validate_not_empty("   ", "field").is_err());
    }

    #[test]
    fn test_validate_port() {
        assert!(ValidationUtils::validate_port(8080).is_ok());
        assert!(ValidationUtils::validate_port(1).is_ok());
        assert!(ValidationUtils::validate_port(65535).is_ok());
        assert!(ValidationUtils::validate_port(0).is_err());
    }

    #[test]
    fn test_validate_timeout_seconds() {
        assert!(ValidationUtils::validate_timeout_seconds(30).is_ok());
        assert!(ValidationUtils::validate_timeout_seconds(3600).is_ok());
        assert!(ValidationUtils::validate_timeout_seconds(0).is_err());
        assert!(ValidationUtils::validate_timeout_seconds(3601).is_err());
    }

    #[test]
    fn test_validate_count() {
        assert!(ValidationUtils::validate_count(10, "test").is_ok());
        assert!(ValidationUtils::validate_count(1, "test").is_ok());
        assert!(ValidationUtils::validate_count(10000, "test").is_ok());
        assert!(ValidationUtils::validate_count(0, "test").is_err());
        assert!(ValidationUtils::validate_count(10001, "test").is_err());
    }

    #[test]
    fn test_validate_url() {
        assert!(ValidationUtils::validate_url("http://localhost:8080", "url").is_ok());
        assert!(ValidationUtils::validate_url("https://example.com", "url").is_ok());
        assert!(ValidationUtils::validate_url("amqp://localhost:5672", "url").is_ok());
        assert!(ValidationUtils::validate_url("", "url").is_err());
        assert!(ValidationUtils::validate_url("localhost:8080", "url").is_err());
    }
}