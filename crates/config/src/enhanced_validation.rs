use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use chrono::{DateTime, Utc};
use regex::Regex;
use url::Url;

use crate::{ConfigError, ConfigResult, ConfigSecurity, Environment, SecurityPolicy};

/// Comprehensive configuration validation system
pub struct ConfigValidator {
    security: ConfigSecurity,
    environment: Environment,
    security_policy: SecurityPolicy,
}

/// Validation result with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub suggestions: Vec<String>,
    pub score: ValidationScore,
}

/// Individual validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field_path: String,
    pub error_type: ValidationErrorType,
    pub message: String,
    pub severity: ValidationSeverity,
    pub fix_suggestion: Option<String>,
}

/// Validation warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub field_path: String,
    pub message: String,
    pub warning_type: ValidationWarningType,
    pub suggestion: Option<String>,
}

/// Validation error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationErrorType {
    RequiredFieldMissing,
    InvalidFormat,
    OutOfRange,
    SecurityViolation,
    EnvironmentMismatch,
    DependencyError,
    ConfigurationError,
}

/// Validation warning types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationWarningType {
    DeprecatedField,
    NonOptimalSetting,
    PerformanceImpact,
    SecurityRecommendation,
    BestPracticeViolation,
}

/// Validation severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Validation score (0-100)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationScore {
    pub overall: u8,
    pub security: u8,
    pub performance: u8,
    pub reliability: u8,
    pub maintainability: u8,
}

/// Validation rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub name: String,
    pub field_pattern: String,
    pub validation_type: ValidationType,
    pub severity: ValidationSeverity,
    pub environment_filter: Vec<Environment>,
    pub enabled: bool,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Validation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationType {
    Required,
    Regex(String),
    MinLength(u32),
    MaxLength(u32),
    MinValue(f64),
    MaxValue(f64),
    Range { min: f64, max: f64 },
    Enum(Vec<String>),
    Url,
    Email,
    PasswordStrength,
    Custom(Box<dyn Fn(&str) -> bool>),
}

impl ConfigValidator {
    /// Create a new configuration validator
    pub fn new(environment: Environment) -> Self {
        let security = ConfigSecurity::new(environment.clone());
        let security_policy = SecurityPolicy::for_environment(environment.clone());
        
        Self {
            security,
            environment,
            security_policy,
        }
    }

    /// Validate configuration comprehensively
    pub fn validate_config(&self, config: &HashMap<String, serde_json::Value>) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        // Run all validation checks
        self.validate_required_fields(config, &mut errors);
        self.validate_security_policies(config, &mut errors, &mut warnings);
        self.validate_environment_settings(config, &mut errors, &mut warnings);
        self.validate_network_settings(config, &mut errors, &mut warnings);
        self.validate_database_settings(config, &mut errors, &mut warnings);
        self.validate_authentication_settings(config, &mut errors, &mut warnings);
        self.validate_performance_settings(config, &mut warnings, &mut suggestions);
        self.validate_custom_rules(config, &mut errors, &mut warnings);

        // Calculate validation score
        let score = self.calculate_validation_score(&errors, &warnings);

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            suggestions,
            score,
        }
    }

    /// Validate required fields
    fn validate_required_fields(&self, config: &HashMap<String, serde_json::Value>, errors: &mut Vec<ValidationError>) {
        let required_fields = match self.environment {
            Environment::Development => vec![
                "database.url",
                "api.bind_address",
            ],
            Environment::Testing => vec![
                "database.url",
                "api.bind_address",
                "api.auth.jwt_secret",
            ],
            Environment::Staging => vec![
                "database.url",
                "api.bind_address",
                "api.auth.jwt_secret",
                "api.auth.enabled",
            ],
            Environment::Production => vec![
                "database.url",
                "api.bind_address",
                "api.auth.jwt_secret",
                "api.auth.enabled",
                "api.rate_limiting.enabled",
            ],
        };

        for field in required_fields {
            if !config.contains_key(field) {
                errors.push(ValidationError {
                    field_path: field.to_string(),
                    error_type: ValidationErrorType::RequiredFieldMissing,
                    message: format!("Required field '{}' is missing", field),
                    severity: ValidationSeverity::Error,
                    fix_suggestion: Some(format!("Add the '{}' field to your configuration", field)),
                });
            }
        }
    }

    /// Validate security policies
    fn validate_security_policies(&self, config: &HashMap<String, serde_json::Value>, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        // Check for insecure defaults in production
        if self.environment == Environment::Production {
            if let Some(cors_origins) = config.get("api.cors_origins") {
                if let Some(origins) = cors_origins.as_array() {
                    for origin in origins {
                        if let Some(origin_str) = origin.as_str() {
                            if origin_str == "*" {
                                errors.push(ValidationError {
                                    field_path: "api.cors_origins".to_string(),
                                    error_type: ValidationErrorType::SecurityViolation,
                                    message: "CORS wildcard origin '*' is not allowed in production".to_string(),
                                    severity: ValidationSeverity::Critical,
                                    fix_suggestion: Some("Specify allowed origins explicitly or disable CORS in production".to_string()),
                                });
                            }
                        }
                    }
                }
            }

            // Check for hardcoded secrets
            for (key, value) in config {
                if self.security.is_sensitive_key(key) {
                    if let Some(value_str) = value.as_str() {
                        if value_str.contains("test") || 
                           value_str.contains("default") || 
                           value_str.contains("example") ||
                           value_str.len() < 12 {
                            errors.push(ValidationError {
                                field_path: key.clone(),
                                error_type: ValidationErrorType::SecurityViolation,
                                message: format!("Potential insecure default value for sensitive field '{}'", key),
                                severity: ValidationSeverity::Critical,
                                fix_suggestion: Some("Use a strong, randomly generated secret".to_string()),
                            });
                        }
                    }
                }
            }
        }

        // Validate authentication settings
        if let Some(auth_enabled) = config.get("api.auth.enabled") {
            if let Some(enabled) = auth_enabled.as_bool() {
                if self.environment != Environment::Development && !enabled {
                    errors.push(ValidationError {
                        field_path: "api.auth.enabled".to_string(),
                        error_type: ValidationErrorType::SecurityViolation,
                        message: "Authentication must be enabled in this environment".to_string(),
                        severity: ValidationSeverity::Error,
                        fix_suggestion: Some("Enable authentication for secure access".to_string()),
                    });
                }
            }
        }
    }

    /// Validate environment-specific settings
    fn validate_environment_settings(&self, config: &HashMap<String, serde_json::Value>, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        // Check bind address
        if let Some(bind_addr) = config.get("api.bind_address") {
            if let Some(addr) = bind_addr.as_str() {
                if addr.starts_with("0.0.0.0") && self.environment == Environment::Development {
                    warnings.push(ValidationWarning {
                        field_path: "api.bind_address".to_string(),
                        message: "Binding to 0.0.0.0 is not recommended in development".to_string(),
                        warning_type: ValidationWarningType::SecurityRecommendation,
                        suggestion: Some("Use 127.0.0.1 for development environments".to_string()),
                    });
                }

                if !addr.starts_with("0.0.0.0") && self.environment == Environment::Production {
                    errors.push(ValidationError {
                        field_path: "api.bind_address".to_string(),
                        error_type: ValidationErrorType::EnvironmentMismatch,
                        message: "Production services should bind to 0.0.0.0 for external access".to_string(),
                        severity: ValidationSeverity::Error,
                        fix_suggestion: Some("Change bind address to 0.0.0.0:port".to_string()),
                    });
                }
            }
        }

        // Check log level
        if let Some(log_level) = config.get("observability.log_level") {
            if let Some(level) = log_level.as_str() {
                if level == "debug" && self.environment == Environment::Production {
                    warnings.push(ValidationWarning {
                        field_path: "observability.log_level".to_string(),
                        message: "Debug logging is not recommended in production".to_string(),
                        warning_type: ValidationWarningType::PerformanceImpact,
                        suggestion: Some("Use 'info' or 'warn' log level in production".to_string()),
                    });
                }
            }
        }
    }

    /// Validate network settings
    fn validate_network_settings(&self, config: &HashMap<String, serde_json::Value>, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        // Validate port numbers
        if let Some(bind_addr) = config.get("api.bind_address") {
            if let Some(addr) = bind_addr.as_str() {
                if let Some(port) = addr.split(':').last() {
                    if let Ok(port_num) = port.parse::<u16>() {
                        if port_num == 0 {
                            errors.push(ValidationError {
                                field_path: "api.bind_address".to_string(),
                                error_type: ValidationErrorType::InvalidFormat,
                                message: "Port number cannot be 0".to_string(),
                                severity: ValidationSeverity::Error,
                                fix_suggestion: Some("Use a valid port number (1-65535)".to_string()),
                            });
                        } else if port_num < 1024 {
                            warnings.push(ValidationWarning {
                                field_path: "api.bind_address".to_string(),
                                message: "Using privileged port (< 1024)".to_string(),
                                warning_type: ValidationWarningType::SecurityRecommendation,
                                suggestion: Some("Consider using a non-privileged port (> 1024)".to_string()),
                            });
                        }
                    }
                }
            }
        }

        // Validate timeouts
        if let Some(timeout) = config.get("api.request_timeout_seconds") {
            if let Some(timeout_val) = timeout.as_u64() {
                if timeout_val == 0 {
                    errors.push(ValidationError {
                        field_path: "api.request_timeout_seconds".to_string(),
                        error_type: ValidationErrorType::OutOfRange,
                        message: "Request timeout must be greater than 0".to_string(),
                        severity: ValidationSeverity::Error,
                        fix_suggestion: Some("Set a reasonable timeout (e.g., 30 seconds)".to_string()),
                    });
                } else if timeout_val > 300 {
                    warnings.push(ValidationWarning {
                        field_path: "api.request_timeout_seconds".to_string(),
                        message: "Very long request timeout may cause performance issues".to_string(),
                        warning_type: ValidationWarningType::PerformanceImpact,
                        suggestion: Some("Consider reducing timeout to 60-120 seconds".to_string()),
                    });
                }
            }
        }
    }

    /// Validate database settings
    fn validate_database_settings(&self, config: &HashMap<String, serde_json::Value>, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        if let Some(db_url) = config.get("database.url") {
            if let Some(url_str) = db_url.as_str() {
                // Check if using PostgreSQL
                if url_str.starts_with("postgresql://") {
                    if let Err(e) = Url::parse(url_str) {
                        errors.push(ValidationError {
                            field_path: "database.url".to_string(),
                            error_type: ValidationErrorType::InvalidFormat,
                            message: format!("Invalid database URL: {}", e),
                            severity: ValidationSeverity::Error,
                            fix_suggestion: Some("Use a valid PostgreSQL connection URL".to_string()),
                        });
                    } else {
                        // Check for insecure connection
                        if !url_str.starts_with("postgresql://") && self.environment == Environment::Production {
                            warnings.push(ValidationWarning {
                                field_path: "database.url".to_string(),
                                message: "Consider using SSL for database connections in production".to_string(),
                                warning_type: ValidationWarningType::SecurityRecommendation,
                                suggestion: Some("Use postgresql:// with SSL parameters".to_string()),
                            });
                        }
                    }
                }
            }
        }

        // Validate connection pool settings
        if let Some(max_conn) = config.get("database.max_connections") {
            if let Some(max_val) = max_conn.as_u64() {
                if max_val == 0 {
                    errors.push(ValidationError {
                        field_path: "database.max_connections".to_string(),
                        error_type: ValidationErrorType::OutOfRange,
                        message: "Max connections must be greater than 0".to_string(),
                        severity: ValidationSeverity::Error,
                        fix_suggestion: Some("Set a reasonable connection pool size".to_string()),
                    });
                } else if max_val > 100 {
                    warnings.push(ValidationWarning {
                        field_path: "database.max_connections".to_string(),
                        message: "Very large connection pool may cause resource issues".to_string(),
                        warning_type: ValidationWarningType::PerformanceImpact,
                        suggestion: Some("Consider reducing max connections to 50 or less".to_string()),
                    });
                }
            }
        }
    }

    /// Validate authentication settings
    fn validate_authentication_settings(&self, config: &HashMap<String, serde_json::Value>, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        if let Some(jwt_secret) = config.get("api.auth.jwt_secret") {
            if let Some(secret) = jwt_secret.as_str() {
                if secret.len() < 32 {
                    errors.push(ValidationError {
                        field_path: "api.auth.jwt_secret".to_string(),
                        error_type: ValidationErrorType::SecurityViolation,
                        message: "JWT secret is too short (minimum 32 characters)".to_string(),
                        severity: ValidationSeverity::Critical,
                        fix_suggestion: Some("Use a longer, randomly generated secret".to_string()),
                    });
                }

                // Check for default/weak secrets
                if secret.contains("default") || secret.contains("test") || secret.len() < 16 {
                    errors.push(ValidationError {
                        field_path: "api.auth.jwt_secret".to_string(),
                        error_type: ValidationErrorType::SecurityViolation,
                        message: "JWT secret appears to be weak or default".to_string(),
                        severity: ValidationSeverity::Critical,
                        fix_suggestion: Some("Generate a strong, random JWT secret".to_string()),
                    });
                }
            }
        }

        // Validate JWT expiration
        if let Some(expiration) = config.get("api.auth.jwt_expiration_hours") {
            if let Some(exp_val) = expiration.as_u64() {
                if exp_val == 0 {
                    errors.push(ValidationError {
                        field_path: "api.auth.jwt_expiration_hours".to_string(),
                        error_type: ValidationErrorType::OutOfRange,
                        message: "JWT expiration must be greater than 0".to_string(),
                        severity: ValidationSeverity::Error,
                        fix_suggestion: Some("Set a reasonable JWT expiration time".to_string()),
                    });
                } else if exp_val > 168 { // 7 days
                    warnings.push(ValidationWarning {
                        field_path: "api.auth.jwt_expiration_hours".to_string(),
                        message: "Very long JWT expiration time may reduce security".to_string(),
                        warning_type: ValidationWarningType::SecurityRecommendation,
                        suggestion: Some("Consider reducing JWT expiration to 24-48 hours".to_string()),
                    });
                }
            }
        }
    }

    /// Validate performance settings
    fn validate_performance_settings(&self, config: &HashMap<String, serde_json::Value>, warnings: &mut Vec<ValidationWarning>, suggestions: &mut Vec<String>) {
        // Check for potential performance issues
        if let Some(max_concurrent) = config.get("dispatcher.max_concurrent_dispatches") {
            if let Some(max_val) = max_concurrent.as_u64() {
                if max_val > 1000 {
                    warnings.push(ValidationWarning {
                        field_path: "dispatcher.max_concurrent_dispatches".to_string(),
                        message: "Very high concurrent dispatches may cause performance issues".to_string(),
                        warning_type: ValidationWarningType::PerformanceImpact,
                        suggestion: Some("Consider reducing to 500 or less".to_string()),
                    });
                }
            }
        }

        // Suggest performance optimizations
        if let Some(tracing_enabled) = config.get("observability.tracing_enabled") {
            if let Some(enabled) = tracing_enabled.as_bool() {
                if enabled {
                    suggestions.push("Consider sampling traces in high-traffic environments".to_string());
                }
            }
        }
    }

    /// Validate custom rules
    fn validate_custom_rules(&self, config: &HashMap<String, serde_json::Value>, errors: &mut Vec<ValidationError>, warnings: &mut Vec<ValidationWarning>) {
        let rules = self.get_validation_rules();

        for rule in rules {
            if !rule.enabled || !rule.environment_filter.contains(&self.environment) {
                continue;
            }

            // Find matching fields
            for (field_path, value) in config {
                if self.field_matches_pattern(field_path, &rule.field_pattern) {
                    if let Some(value_str) = value.as_str() {
                        let is_valid = match &rule.validation_type {
                            ValidationType::Required => !value_str.is_empty(),
                            ValidationType::Regex(pattern) => {
                                let re = Regex::new(pattern).unwrap_or_else(|_| Regex::new(r".*").unwrap());
                                re.is_match(value_str)
                            },
                            ValidationType::MinLength(min) => value_str.len() >= *min as usize,
                            ValidationType::MaxLength(max) => value_str.len() <= *max as usize,
                            ValidationType::MinValue(min) => value_str.parse::<f64>().map(|v| v >= *min).unwrap_or(false),
                            ValidationType::MaxValue(max) => value_str.parse::<f64>().map(|v| v <= *max).unwrap_or(false),
                            ValidationType::Range { min, max } => {
                                value_str.parse::<f64>().map(|v| v >= *min && v <= *max).unwrap_or(false)
                            },
                            ValidationType::Enum(options) => options.contains(&value_str.to_string()),
                            ValidationType::Url => Url::parse(value_str).is_ok(),
                            ValidationType::Email => {
                                let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
                                email_regex.is_match(value_str)
                            },
                            ValidationType::PasswordStrength => {
                                // Check for password strength
                                value_str.len() >= 8 &&
                                value_str.chars().any(|c| c.is_ascii_uppercase()) &&
                                value_str.chars().any(|c| c.is_ascii_lowercase()) &&
                                value_str.chars().any(|c| c.is_ascii_digit()) &&
                                value_str.chars().any(|c| !c.is_alphanumeric())
                            },
                            ValidationType::Custom(_) => true, // Custom validation not implemented in this example
                        };

                        if !is_valid {
                            let issue = ValidationError {
                                field_path: field_path.clone(),
                                error_type: ValidationErrorType::ConfigurationError,
                                message: rule.message.clone(),
                                severity: rule.severity.clone(),
                                fix_suggestion: rule.suggestion.clone(),
                            };

                            if matches!(rule.severity, ValidationSeverity::Error | ValidationSeverity::Critical) {
                                errors.push(issue);
                            } else {
                                warnings.push(ValidationWarning {
                                    field_path: field_path.clone(),
                                    message: rule.message.clone(),
                                    warning_type: ValidationWarningType::BestPracticeViolation,
                                    suggestion: rule.suggestion.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get validation rules for the current environment
    fn get_validation_rules(&self) -> Vec<ValidationRule> {
        vec![
            ValidationRule {
                name: "strong_password".to_string(),
                field_pattern: r"password|secret|key".to_string(),
                validation_type: ValidationType::PasswordStrength,
                severity: ValidationSeverity::Warning,
                environment_filter: vec![Environment::Production, Environment::Staging],
                enabled: true,
                message: "Password should be at least 8 characters with mixed case, numbers, and symbols".to_string(),
                suggestion: Some("Use a strong password generator".to_string()),
            },
            ValidationRule {
                name: "secure_url".to_string(),
                field_pattern: r".*url$".to_string(),
                validation_type: ValidationType::Url,
                severity: ValidationSeverity::Error,
                environment_filter: vec![Environment::Production, Environment::Staging],
                enabled: true,
                message: "URL must be valid".to_string(),
                suggestion: Some("Use a valid URL format".to_string()),
            },
            ValidationRule {
                name: "reasonable_timeout".to_string(),
                field_pattern: r".*timeout.*".to_string(),
                validation_type: ValidationType::Range { min: 1.0, max: 300.0 },
                severity: ValidationSeverity::Warning,
                environment_filter: vec![Environment::Production, Environment::Staging, Environment::Testing],
                enabled: true,
                message: "Timeout should be between 1 and 300 seconds".to_string(),
                suggestion: Some("Use a reasonable timeout value".to_string()),
            },
        ]
    }

    /// Check if field path matches pattern
    fn field_matches_pattern(&self, field_path: &str, pattern: &str) -> bool {
        let re = Regex::new(pattern).unwrap_or_else(|_| Regex::new(r".*").unwrap());
        re.is_match(field_path)
    }

    /// Calculate validation score
    fn calculate_validation_score(&self, errors: &[ValidationError], warnings: &[ValidationWarning]) -> ValidationScore {
        let error_weight = 10;
        let warning_weight = 2;
        
        let total_deductions = errors.len() * error_weight + warnings.len() * warning_weight;
        let max_score = 100;
        let overall = (max_score - total_deductions as u8).max(0);

        // Calculate category scores
        let security_errors = errors.iter().filter(|e| matches!(e.error_type, ValidationErrorType::SecurityViolation)).count();
        let security_warnings = warnings.iter().filter(|w| matches!(w.warning_type, ValidationWarningType::SecurityRecommendation)).count();
        let security_score = (100 - (security_errors * error_weight + security_warnings * warning_weight) as u8).max(0);

        let performance_issues = warnings.iter().filter(|w| matches!(w.warning_type, ValidationWarningType::PerformanceImpact)).count();
        let performance_score = (100 - (performance_issues * warning_weight) as u8).max(0);

        let reliability_errors = errors.iter().filter(|e| 
            matches!(e.error_type, ValidationErrorType::RequiredFieldMissing | ValidationErrorType::DependencyError)
        ).count();
        let reliability_score = (100 - (reliability_errors * error_weight) as u8).max(0);

        let maintainability_issues = warnings.iter().filter(|w| 
            matches!(w.warning_type, ValidationWarningType::BestPracticeViolation | ValidationWarningType::DeprecatedField)
        ).count();
        let maintainability_score = (100 - (maintainability_issues * warning_weight) as u8).max(0);

        ValidationScore {
            overall,
            security: security_score,
            performance: performance_score,
            reliability: reliability_score,
            maintainability: maintainability_score,
        }
    }

    /// Validate configuration file
    pub fn validate_config_file(&self, file_path: &Path) -> ConfigResult<ValidationResult> {
        // Load configuration
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| ConfigError::File(format!("Failed to read config file: {}", e)))?;

        let config: HashMap<String, serde_json::Value> = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Parse(format!("Failed to parse config file: {}", e)))?;

        Ok(self.validate_config(&config))
    }

    /// Get security recommendations
    pub fn get_security_recommendations(&self, config: &HashMap<String, serde_json::Value>) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Check for common security issues
        if !config.contains_key("api.auth.enabled") {
            recommendations.push("Enable authentication for API access".to_string());
        }

        if let Some(cors_enabled) = config.get("api.cors_enabled") {
            if let Some(enabled) = cors_enabled.as_bool() {
                if enabled && self.environment == Environment::Production {
                    recommendations.push("Disable CORS or restrict to specific origins in production".to_string());
                }
            }
        }

        if let Some(log_level) = config.get("observability.log_level") {
            if let Some(level) = log_level.as_str() {
                if level == "debug" && self.environment == Environment::Production {
                    recommendations.push("Reduce log level to 'info' or 'warn' in production".to_string());
                }
            }
        }

        recommendations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_config_validator_creation() {
        let validator = ConfigValidator::new(Environment::Development);
        assert_eq!(validator.environment, Environment::Development);
    }

    #[test]
    fn test_validate_empty_config() {
        let validator = ConfigValidator::new(Environment::Production);
        let config = HashMap::new();
        
        let result = validator.validate_config(&config);
        
        assert!(!result.is_valid);
        assert!(result.errors.len() > 0);
        assert!(result.errors.iter().any(|e| e.field_path == "database.url"));
    }

    #[test]
    fn test_validate_security_violations() {
        let validator = ConfigValidator::new(Environment::Production);
        
        let mut config = HashMap::new();
        config.insert("database.url".to_string(), serde_json::Value::String("postgresql://localhost/db".to_string()));
        config.insert("api.bind_address".to_string(), serde_json::Value::String("127.0.0.1:8080".to_string()));
        config.insert("api.cors_origins".to_string(), serde_json::Value::Array(vec![
            serde_json::Value::String("*".to_string())
        ]));
        config.insert("api.auth.jwt_secret".to_string(), serde_json::Value::String("test_secret".to_string()));
        
        let result = validator.validate_config(&config);
        
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.message.contains("CORS wildcard")));
        assert!(result.errors.iter().any(|e| e.message.contains("insecure default")));
    }

    #[test]
    fn test_validate_score_calculation() {
        let validator = ConfigValidator::new(Environment::Production);
        
        let mut config = HashMap::new();
        config.insert("database.url".to_string(), serde_json::Value::String("postgresql://localhost/db".to_string()));
        config.insert("api.bind_address".to_string(), serde_json::Value::String("0.0.0.0:8443".to_string()));
        config.insert("api.auth.enabled".to_string(), serde_json::Value::Bool(true));
        config.insert("api.auth.jwt_secret".to_string(), serde_json::Value::String("strong_random_jwt_secret_1234567890".to_string()));
        
        let result = validator.validate_config(&config);
        
        assert!(result.is_valid);
        assert!(result.score.overall >= 80);
        assert!(result.score.security >= 80);
    }

    #[test]
    fn test_security_recommendations() {
        let validator = ConfigValidator::new(Environment::Production);
        
        let mut config = HashMap::new();
        config.insert("database.url".to_string(), serde_json::Value::String("postgresql://localhost/db".to_string()));
        config.insert("api.cors_enabled".to_string(), serde_json::Value::Bool(true));
        config.insert("observability.log_level".to_string(), serde_json::Value::String("debug".to_string()));
        
        let recommendations = validator.get_security_recommendations(&config);
        
        assert!(recommendations.len() > 0);
        assert!(recommendations.iter().any(|r| r.contains("CORS")));
        assert!(recommendations.iter().any(|r| r.contains("log level")));
    }
}