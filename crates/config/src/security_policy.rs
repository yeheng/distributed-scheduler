use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::{ConfigError, ConfigResult, Environment};

/// Environment-specific security policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub environment: Environment,
    pub cors_policy: CorsPolicy,
    pub auth_policy: AuthPolicy,
    pub encryption_policy: EncryptionPolicy,
    pub network_policy: NetworkPolicy,
    pub audit_policy: AuditPolicy,
    pub secret_policy: SecretPolicy,
    pub validation_policy: ValidationPolicy,
}

/// CORS configuration policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsPolicy {
    pub enabled: bool,
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub exposed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age_seconds: u32,
}

impl Default for CorsPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_origins: vec![],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
            ],
            allowed_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
            exposed_headers: vec![],
            allow_credentials: false,
            max_age_seconds: 3600,
        }
    }
}

/// Authentication and authorization policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthPolicy {
    pub enabled: bool,
    pub jwt_policy: JwtPolicy,
    pub api_key_policy: ApiKeyPolicy,
    pub session_policy: SessionPolicy,
    pub rate_limiting: RateLimitingPolicy,
}

/// JWT security policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtPolicy {
    pub required: bool,
    pub algorithm: String,
    pub minimum_key_length: usize,
    pub expiration_hours: u32,
    pub issuer: String,
    pub audience: String,
    pub refresh_enabled: bool,
    pub refresh_expiration_hours: u32,
}

impl Default for JwtPolicy {
    fn default() -> Self {
        Self {
            required: true,
            algorithm: "HS256".to_string(),
            minimum_key_length: 32,
            expiration_hours: 24,
            issuer: "scheduler-system".to_string(),
            audience: "scheduler-api".to_string(),
            refresh_enabled: true,
            refresh_expiration_hours: 168, // 7 days
        }
    }
}

/// API key policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyPolicy {
    pub enabled: bool,
    pub required_prefix: String,
    pub minimum_length: usize,
    pub rotation_days: u32,
    pub allow_multiple_keys: bool,
}

impl Default for ApiKeyPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            required_prefix: "sk_".to_string(),
            minimum_length: 32,
            rotation_days: 90,
            allow_multiple_keys: true,
        }
    }
}

/// Session management policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPolicy {
    pub timeout_minutes: u32,
    pub concurrent_sessions: u32,
    pub ip_binding: bool,
    pub user_agent_binding: bool,
}

impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            timeout_minutes: 60,
            concurrent_sessions: 3,
            ip_binding: true,
            user_agent_binding: true,
        }
    }
}

/// Rate limiting policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingPolicy {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub ip_based: bool,
    pub user_based: bool,
    pub endpoint_specific: bool,
}

impl Default for RateLimitingPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: 60,
            burst_size: 10,
            ip_based: true,
            user_based: true,
            endpoint_specific: true,
        }
    }
}

impl Default for AuthPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            jwt_policy: JwtPolicy::default(),
            api_key_policy: ApiKeyPolicy::default(),
            session_policy: SessionPolicy::default(),
            rate_limiting: RateLimitingPolicy::default(),
        }
    }
}

/// Encryption policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionPolicy {
    pub enabled: bool,
    pub algorithm: String,
    pub key_rotation_days: u32,
    pub encrypt_at_rest: bool,
    pub encrypt_in_transit: bool,
    pub sensitive_fields: Vec<String>,
}

impl Default for EncryptionPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            algorithm: "AES-256-GCM".to_string(),
            key_rotation_days: 90,
            encrypt_at_rest: true,
            encrypt_in_transit: true,
            sensitive_fields: vec![
                "password".to_string(),
                "secret".to_string(),
                "key".to_string(),
                "token".to_string(),
                "credential".to_string(),
                "certificate".to_string(),
            ],
        }
    }
}

/// Network security policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub bind_address: String,
    pub tls_enabled: bool,
    pub tls_version_min: String,
    pub allowed_ip_ranges: Vec<String>,
    pub blocked_ip_ranges: Vec<String>,
    pub max_connections: u32,
    pub connection_timeout_seconds: u32,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".to_string(),
            tls_enabled: false,
            tls_version_min: "1.2".to_string(),
            allowed_ip_ranges: vec![],
            blocked_ip_ranges: vec![],
            max_connections: 1000,
            connection_timeout_seconds: 30,
        }
    }
}

/// Audit and logging policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPolicy {
    pub enabled: bool,
    pub log_security_events: bool,
    pub log_auth_events: bool,
    pub log_data_access: bool,
    pub retention_days: u32,
    pub include_sensitive_data: bool,
    pub real_time_alerts: bool,
}

impl Default for AuditPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            log_security_events: true,
            log_auth_events: true,
            log_data_access: false,
            retention_days: 365,
            include_sensitive_data: false,
            real_time_alerts: false,
        }
    }
}

/// Secret management policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretPolicy {
    pub encryption_required: bool,
    pub minimum_length: usize,
    pub complexity_required: bool,
    pub rotation_days: u32,
    pub history_retention: u32,
    pub prevent_reuse: bool,
    pub expiration_warning_days: u32,
}

impl Default for SecretPolicy {
    fn default() -> Self {
        Self {
            encryption_required: true,
            minimum_length: 16,
            complexity_required: true,
            rotation_days: 90,
            history_retention: 5,
            prevent_reuse: true,
            expiration_warning_days: 7,
        }
    }
}

/// Configuration validation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationPolicy {
    pub strict_mode: bool,
    pub validate_on_startup: bool,
    pub validate_on_change: bool,
    pub required_fields: Vec<String>,
    pub forbidden_fields: Vec<String>,
    pub custom_rules: Vec<ValidationRule>,
}

/// Custom validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub name: String,
    pub field_pattern: String,
    pub validation_type: ValidationType,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Validation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationType {
    Regex(String),
    MinLength(u32),
    MaxLength(u32),
    Range { min: f64, max: f64 },
    Enum(Vec<String>),
    Custom(String),
}

impl Default for ValidationPolicy {
    fn default() -> Self {
        Self {
            strict_mode: false,
            validate_on_startup: true,
            validate_on_change: true,
            required_fields: vec!["database.url".to_string(), "api.bind_address".to_string()],
            forbidden_fields: vec![
                "api.auth.jwt_secret".to_string(),
                "database.password".to_string(),
            ],
            custom_rules: vec![],
        }
    }
}

impl SecurityPolicy {
    /// Create security policy for specific environment
    pub fn for_environment(environment: Environment) -> Self {
        match environment {
            Environment::Development => Self::development_policy(),
            Environment::Testing => Self::testing_policy(),
            Environment::Staging => Self::staging_policy(),
            Environment::Production => Self::production_policy(),
        }
    }

    /// Development environment policy (relaxed security for development)
    pub fn development_policy() -> Self {
        Self {
            environment: Environment::Development,
            cors_policy: CorsPolicy {
                enabled: true,
                allowed_origins: vec!["*".to_string()],
                allow_credentials: true,
                ..CorsPolicy::default()
            },
            auth_policy: AuthPolicy {
                enabled: false, // Auth disabled in development
                rate_limiting: RateLimitingPolicy {
                    enabled: false,
                    ..RateLimitingPolicy::default()
                },
                ..AuthPolicy::default()
            },
            encryption_policy: EncryptionPolicy {
                enabled: false, // Encryption disabled in development
                ..EncryptionPolicy::default()
            },
            network_policy: NetworkPolicy {
                bind_address: "127.0.0.1:8080".to_string(),
                tls_enabled: false,
                ..Default::default()
            },
            audit_policy: AuditPolicy {
                enabled: true,
                include_sensitive_data: true, // Allow sensitive data in dev logs
                ..Default::default()
            },
            secret_policy: SecretPolicy {
                encryption_required: false,
                minimum_length: 8,
                complexity_required: false,
                rotation_days: 365, // No rotation in dev
                ..Default::default()
            },
            validation_policy: ValidationPolicy {
                strict_mode: false,
                validate_on_startup: false,
                ..Default::default()
            },
        }
    }

    /// Testing environment policy (balanced for testing)
    pub fn testing_policy() -> Self {
        Self {
            environment: Environment::Testing,
            cors_policy: CorsPolicy {
                enabled: true,
                allowed_origins: vec!["http://localhost:*".to_string()],
                allow_credentials: true,
                ..Default::default()
            },
            auth_policy: AuthPolicy {
                enabled: true,
                jwt_policy: JwtPolicy {
                    expiration_hours: 1, // Short expiration for testing
                    ..Default::default()
                },
                ..Default::default()
            },
            encryption_policy: EncryptionPolicy {
                enabled: true,
                ..Default::default()
            },
            network_policy: NetworkPolicy {
                bind_address: "127.0.0.1:8080".to_string(),
                tls_enabled: false,
                ..Default::default()
            },
            audit_policy: AuditPolicy {
                enabled: true,
                real_time_alerts: true,
                ..Default::default()
            },
            secret_policy: SecretPolicy {
                minimum_length: 12,
                ..Default::default()
            },
            validation_policy: ValidationPolicy {
                strict_mode: true,
                validate_on_startup: true,
                ..Default::default()
            },
        }
    }

    /// Staging environment policy (production-like for testing)
    pub fn staging_policy() -> Self {
        Self {
            environment: Environment::Staging,
            cors_policy: CorsPolicy {
                enabled: true,
                allowed_origins: vec!["https://staging.example.com".to_string()],
                allow_credentials: true,
                ..Default::default()
            },
            auth_policy: AuthPolicy {
                enabled: true,
                ..Default::default()
            },
            encryption_policy: EncryptionPolicy {
                enabled: true,
                ..Default::default()
            },
            network_policy: NetworkPolicy {
                bind_address: "0.0.0.0:8080".to_string(),
                tls_enabled: true,
                ..Default::default()
            },
            audit_policy: AuditPolicy {
                enabled: true,
                real_time_alerts: true,
                ..Default::default()
            },
            secret_policy: SecretPolicy {
                minimum_length: 16,
                ..Default::default()
            },
            validation_policy: ValidationPolicy {
                strict_mode: true,
                validate_on_startup: true,
                ..Default::default()
            },
        }
    }

    /// Production environment policy (maximum security)
    pub fn production_policy() -> Self {
        Self {
            environment: Environment::Production,
            cors_policy: CorsPolicy {
                enabled: false, // CORS disabled in production
                ..Default::default()
            },
            auth_policy: AuthPolicy {
                enabled: true,
                jwt_policy: JwtPolicy {
                    required: true,
                    minimum_key_length: 64,
                    expiration_hours: 8, // Short expiration for security
                    refresh_expiration_hours: 24,
                    ..Default::default()
                },
                api_key_policy: ApiKeyPolicy {
                    minimum_length: 64,
                    rotation_days: 30,
                    ..Default::default()
                },
                rate_limiting: RateLimitingPolicy {
                    requests_per_minute: 30, // More restrictive in production
                    ..Default::default()
                },
                ..Default::default()
            },
            encryption_policy: EncryptionPolicy {
                enabled: true,
                key_rotation_days: 30, // More frequent rotation
                ..Default::default()
            },
            network_policy: NetworkPolicy {
                bind_address: "0.0.0.0:8443".to_string(), // HTTPS port
                tls_enabled: true,
                tls_version_min: "1.3".to_string(),
                max_connections: 500,
                connection_timeout_seconds: 15,
                ..Default::default()
            },
            audit_policy: AuditPolicy {
                enabled: true,
                log_security_events: true,
                log_auth_events: true,
                log_data_access: true,
                retention_days: 730, // 2 years retention
                real_time_alerts: true,
                ..Default::default()
            },
            secret_policy: SecretPolicy {
                encryption_required: true,
                minimum_length: 32,
                complexity_required: true,
                rotation_days: 30,
                history_retention: 10,
                prevent_reuse: true,
                expiration_warning_days: 3,
            },
            validation_policy: ValidationPolicy {
                strict_mode: true,
                validate_on_startup: true,
                validate_on_change: true,
                required_fields: vec![
                    "database.url".to_string(),
                    "api.bind_address".to_string(),
                    "api.auth.jwt_secret".to_string(),
                    "message_queue.url".to_string(),
                ],
                forbidden_fields: vec![
                    "api.auth.jwt_secret".to_string(),
                    "database.password".to_string(),
                    "api.keys".to_string(),
                ],
                custom_rules: vec![
                    ValidationRule {
                        name: "strong_password".to_string(),
                        field_pattern: r"password|secret|key".to_string(),
                        validation_type: ValidationType::Regex(r"^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[@$!%*?&])[A-Za-z\d@$!%*?&]{12,}$".to_string()),
                        parameters: HashMap::new(),
                    },
                    ValidationRule {
                        name: "secure_url".to_string(),
                        field_pattern: r".*url$".to_string(),
                        validation_type: ValidationType::Regex(r"^https://".to_string()),
                        parameters: HashMap::new(),
                    },
                ],
            },
        }
    }

    /// Load security policy from file
    pub fn from_file(file_path: &Path) -> ConfigResult<Self> {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| ConfigError::File(format!("Failed to read security policy file: {e}")))?;

        let policy: SecurityPolicy = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Parse(format!("Failed to parse security policy: {e}")))?;

        Ok(policy)
    }

    /// Save security policy to file
    pub fn to_file(&self, file_path: &Path) -> ConfigResult<()> {
        let content = serde_json::to_string_pretty(self).map_err(|e| {
            ConfigError::Security(format!("Failed to serialize security policy: {e}"))
        })?;

        std::fs::write(file_path, content)
            .map_err(|e| ConfigError::File(format!("Failed to write security policy file: {e}")))?;

        Ok(())
    }

    /// Validate configuration against this policy
    pub fn validate_config(
        &self,
        config: &HashMap<String, serde_json::Value>,
    ) -> ConfigResult<Vec<String>> {
        let mut violations = Vec::new();

        // Check required fields
        for field in &self.validation_policy.required_fields {
            if !config.contains_key(field) {
                violations.push(format!("Required field '{field}' is missing"));
            }
        }

        // Check forbidden fields
        for field in &self.validation_policy.forbidden_fields {
            if config.contains_key(field) {
                violations.push(format!("Forbidden field '{field}' is present"));
            }
        }

        // Validate CORS settings
        if let Some(cors_origins) = config.get("api.cors_origins") {
            if let Some(origins) = cors_origins.as_array() {
                for origin in origins {
                    if let Some(origin_str) = origin.as_str() {
                        if origin_str == "*" && self.environment == Environment::Production {
                            violations.push(
                                "CORS wildcard origin '*' is not allowed in production".to_string(),
                            );
                        }
                    }
                }
            }
        }

        // Validate secret fields
        for (key, value) in config {
            if self
                .encryption_policy
                .sensitive_fields
                .iter()
                .any(|field| key.contains(field))
            {
                if let Some(value_str) = value.as_str() {
                    if value_str.len() < self.secret_policy.minimum_length {
                        violations.push(format!(
                            "Secret field '{}' is too short (min {} chars)",
                            key, self.secret_policy.minimum_length
                        ));
                    }
                }
            }
        }

        // Validate network settings
        if let Some(bind_addr) = config.get("api.bind_address") {
            if let Some(addr) = bind_addr.as_str() {
                if addr.starts_with("0.0.0.0") && self.environment == Environment::Development {
                    violations
                        .push("Binding to 0.0.0.0 is not recommended in development".to_string());
                }
            }
        }

        // Validate authentication settings
        if self.auth_policy.enabled {
            if let Some(auth_enabled) = config.get("api.auth.enabled") {
                if let Some(enabled) = auth_enabled.as_bool() {
                    if !enabled {
                        violations
                            .push("Authentication must be enabled in this environment".to_string());
                    }
                }
            }
        }

        Ok(violations)
    }

    /// Check if a field should be encrypted
    pub fn should_encrypt(&self, field_name: &str) -> bool {
        if !self.encryption_policy.enabled {
            return false;
        }

        self.encryption_policy
            .sensitive_fields
            .iter()
            .any(|field| field_name.contains(field))
    }

    /// Get security recommendations
    pub fn get_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        match self.environment {
            Environment::Development => {
                recommendations
                    .push("Consider enabling authentication for API testing".to_string());
                recommendations.push("Use environment-specific configurations".to_string());
            }
            Environment::Testing => {
                recommendations.push("Enable encryption for sensitive test data".to_string());
                recommendations.push("Implement comprehensive audit logging".to_string());
            }
            Environment::Staging => {
                recommendations.push("Use production-like security settings".to_string());
                recommendations.push("Enable TLS for all communications".to_string());
            }
            Environment::Production => {
                recommendations.push("Enable all security features".to_string());
                recommendations.push("Implement intrusion detection".to_string());
                recommendations
                    .push("Use hardware security modules for key management".to_string());
            }
        }

        recommendations
    }

    /// Merge with another policy (for customization)
    pub fn merge(&self, other: &SecurityPolicy) -> SecurityPolicy {
        SecurityPolicy {
            environment: self.environment,
            cors_policy: if other.cors_policy.allowed_origins
                != CorsPolicy::default().allowed_origins
            {
                other.cors_policy.clone()
            } else {
                self.cors_policy.clone()
            },
            auth_policy: other.auth_policy.clone(),
            encryption_policy: other.encryption_policy.clone(),
            network_policy: other.network_policy.clone(),
            audit_policy: other.audit_policy.clone(),
            secret_policy: other.secret_policy.clone(),
            validation_policy: other.validation_policy.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_development_policy() {
        let policy = SecurityPolicy::development_policy();
        assert_eq!(policy.environment, Environment::Development);
        assert!(policy.cors_policy.enabled);
        assert!(!policy.auth_policy.enabled);
        assert!(!policy.encryption_policy.enabled);
    }

    #[test]
    fn test_production_policy() {
        let policy = SecurityPolicy::production_policy();
        assert_eq!(policy.environment, Environment::Production);
        assert!(!policy.cors_policy.enabled);
        assert!(policy.auth_policy.enabled);
        assert!(policy.encryption_policy.enabled);
        assert!(policy.network_policy.tls_enabled);
    }

    #[test]
    fn test_config_validation() {
        let policy = SecurityPolicy::production_policy();

        let mut config = HashMap::new();
        config.insert(
            "api.cors_origins".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String("*".to_string())]),
        );
        config.insert(
            "api.auth.enabled".to_string(),
            serde_json::Value::Bool(false),
        );

        let violations = policy.validate_config(&config).unwrap();
        assert!(violations
            .iter()
            .any(|v| v.contains("CORS wildcard origin")));
        assert!(violations
            .iter()
            .any(|v| v.contains("Authentication must be enabled")));
    }

    #[test]
    fn test_encryption_field_detection() {
        let policy = SecurityPolicy::production_policy();
        assert!(policy.should_encrypt("database_password"));
        assert!(policy.should_encrypt("api_secret_key"));
        assert!(!policy.should_encrypt("log_level"));
        assert!(!policy.should_encrypt("max_connections"));
    }

    #[test]
    fn test_security_recommendations() {
        let dev_policy = SecurityPolicy::development_policy();
        let prod_policy = SecurityPolicy::production_policy();

        let dev_recs = dev_policy.get_recommendations();
        let prod_recs = prod_policy.get_recommendations();

        assert!(dev_recs
            .iter()
            .any(|r| r.contains("authentication for API testing")));
        assert!(prod_recs
            .iter()
            .any(|r| r.contains("hardware security modules")));
    }

    #[test]
    fn test_policy_merge() {
        let base = SecurityPolicy::development_policy();
        let mut custom = SecurityPolicy::development_policy();
        custom.cors_policy.allowed_origins = vec!["https://example.com".to_string()];

        let merged = base.merge(&custom);
        assert_eq!(
            merged.cors_policy.allowed_origins,
            vec!["https://example.com"]
        );
    }
}
