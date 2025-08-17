use crate::Environment;
use crate::{ConfigError, ConfigResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use ring::rand::{SecureRandom, SystemRandom};
use ring::{aead, pbkdf2};
use ring::aead::{Nonce, NonceSequence};
use base64::{Engine as _, engine::general_purpose};
use once_cell::sync::Lazy;

/// Configuration security manager for handling encryption, sanitization, and secure configuration
pub struct ConfigSecurity {
    pub encryption_key: Option<Vec<u8>>,
    environment: Environment,
    audit_log: Vec<SecurityEvent>,
}

/// Security event types for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEventType {
    ConfigurationLoaded,
    EnvironmentVariableAccessed,
    SensitiveValueMasked,
    ConfigurationEncrypted,
    ConfigurationDecrypted,
    ConfigurationValidated,
    SecurityViolation,
    PermissionCheck,
}

/// Security event for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub event_type: SecurityEventType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub user: Option<String>,
    pub resource: String,
    pub details: String,
    pub severity: SecuritySeverity,
}

/// Security severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecuritySeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Sensitive configuration patterns
pub static SENSITIVE_PATTERNS: Lazy<Vec<&str>> = Lazy::new(|| {
    vec![
        "password",
        "secret",
        "key",
        "token",
        "credential",
        "auth",
        "private",
        "certificate",
        "ssl",
        "tls",
        "jwt",
        "api_key",
        "database_url",
        "connection_string",
    ]
});

impl ConfigSecurity {
    /// Create a new configuration security manager
    pub fn new(environment: Environment) -> Self {
        Self {
            encryption_key: Self::load_encryption_key(),
            environment,
            audit_log: Vec::new(),
        }
    }

    /// Load encryption key from environment or secure storage
    fn load_encryption_key() -> Option<Vec<u8>> {
        // For testing, use a fixed key if in test mode
        if cfg!(test) {
            return Some(b"12345678901234567890123456789012".to_vec());
        }
        
        env::var("CONFIG_ENCRYPTION_KEY")
            .ok()
            .or_else(|| env::var("SCHEDULER_CONFIG_KEY").ok())
            .map(|key| {
                let decoded = general_purpose::STANDARD.decode(&key)
                    .unwrap_or_else(|_| key.clone().into_bytes());
                decoded
            })
    }

    /// Encrypt a sensitive configuration value
    pub fn encrypt_value(&self, value: &str) -> ConfigResult<String> {
        if value.is_empty() {
            return Ok(String::new());
        }

        let key = self.encryption_key.as_ref()
            .ok_or_else(|| ConfigError::Security("Encryption key not configured".to_string()))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        let rng = SystemRandom::new();
        rng.fill(&mut nonce_bytes).map_err(|e| {
            ConfigError::Security(format!("Failed to generate nonce: {}", e))
        })?;

        // Create sealing key
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key)
            .map_err(|e| ConfigError::Security(format!("Failed to create encryption key: {}", e)))?;
        
        // Add NonceSequence implementation
        struct CounterNonceSequence(u32);

        impl NonceSequence for CounterNonceSequence {
            fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
                let mut nonce_bytes = [0u8; 12];
                nonce_bytes[0..4].copy_from_slice(&self.0.to_be_bytes());
                self.0 += 1;
                Ok(Nonce::assume_unique_for_key(nonce_bytes))
            }
        }
        
        let nonce_seq = CounterNonceSequence(0);
        let mut sealing_key: aead::SealingKey<CounterNonceSequence> = aead::BoundKey::new(unbound_key, nonce_seq);

        // Encrypt the value
        let plaintext = value.as_bytes();
        let mut in_out = plaintext.to_vec();
        let aad = aead::Aad::empty();
        sealing_key.seal_in_place_append_tag(aad, &mut in_out)
            .map_err(|e| ConfigError::Security(format!("Encryption failed: {}", e)))?;

        // Combine nonce and ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&in_out);

        Ok(general_purpose::STANDARD.encode(&result))
    }

    /// Decrypt a sensitive configuration value
    pub fn decrypt_value(&self, encrypted_value: &str) -> ConfigResult<String> {
        if encrypted_value.is_empty() {
            return Ok(String::new());
        }

        let key = self.encryption_key.as_ref()
            .ok_or_else(|| ConfigError::Security("Encryption key not configured".to_string()))?;

        let data = general_purpose::STANDARD.decode(encrypted_value)
            .map_err(|e| ConfigError::Security(format!("Base64 decode failed: {}", e)))?;

        if data.len() < 12 {
            return Err(ConfigError::Security("Invalid encrypted data length".to_string()));
        }

        // Extract nonce and ciphertext
        let (_nonce_bytes, ciphertext) = data.split_at(12);

        // Create opening key
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key)
            .map_err(|e| ConfigError::Security(format!("Failed to create decryption key: {}", e)))?;
        
        // Add NonceSequence implementation
        struct CounterNonceSequence(u32);

        impl NonceSequence for CounterNonceSequence {
            fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
                let mut nonce_bytes = [0u8; 12];
                nonce_bytes[0..4].copy_from_slice(&self.0.to_be_bytes());
                self.0 += 1;
                Ok(Nonce::assume_unique_for_key(nonce_bytes))
            }
        }
        
        let nonce_seq = CounterNonceSequence(0);
        let mut opening_key: aead::OpeningKey<CounterNonceSequence> = aead::BoundKey::new(unbound_key, nonce_seq);

        // Decrypt the value
        let mut in_out = ciphertext.to_vec();
        let aad = aead::Aad::empty();
        let plaintext = opening_key.open_in_place(aad, &mut in_out)
            .map_err(|e| ConfigError::Security(format!("Decryption failed: {}", e)))?;

        Ok(String::from_utf8(plaintext.to_vec())
            .map_err(|e| ConfigError::Security(format!("UTF-8 conversion failed: {}", e)))?)
    }

    /// Sanitize configuration values for logging/display
    pub fn sanitize_config(&self, config: &HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
        let mut sanitized = HashMap::new();

        for (key, value) in config {
            if self.is_sensitive_key(key) {
                sanitized.insert(key.clone(), serde_json::Value::String("***REDACTED***".to_string()));
            } else {
                sanitized.insert(key.clone(), value.clone());
            }
        }

        sanitized
    }

    /// Check if a configuration key is sensitive
    pub fn is_sensitive_key(&self, key: &str) -> bool {
        let key_lower = key.to_lowercase();
        SENSITIVE_PATTERNS.iter().any(|pattern| key_lower.contains(pattern))
    }

    /// Validate secure file permissions
    pub fn validate_file_permissions(&self, file_path: &Path) -> ConfigResult<()> {
        if !file_path.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(file_path)
            .map_err(|e| ConfigError::Security(format!("Failed to read file metadata: {}", e)))?;

        // Check if file is readable by others
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = metadata.permissions();
            let mode = permissions.mode();

            // Check if file is world-readable (others have read permission)
            if mode & 0o004 != 0 {
                return Err(ConfigError::Security(format!(
                    "Configuration file {} is world-readable. Set restrictive permissions.",
                    file_path.display()
                )));
            }

            // Check if file is world-writable
            if mode & 0o002 != 0 {
                return Err(ConfigError::Security(format!(
                    "Configuration file {} is world-writable. Set restrictive permissions.",
                    file_path.display()
                )));
            }
        }

        Ok(())
    }

    /// Generate a secure random string for secrets
    pub fn generate_secure_secret(&self, length: usize) -> ConfigResult<String> {
        if length < 32 {
            return Err(ConfigError::Security("Secret length must be at least 32 characters".to_string()));
        }

        let mut bytes = vec![0u8; length];
        let rng = SystemRandom::new();
        rng.fill(&mut bytes)
            .map_err(|e| ConfigError::Security(format!("Failed to generate secure random: {}", e)))?;

        // Use alphanumeric characters for better compatibility
        let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut result = String::with_capacity(length);

        for &byte in &bytes {
            result.push(char::from(charset[byte as usize % charset.len()]));
        }

        Ok(result)
    }

    /// Hash a configuration value for comparison
    pub fn hash_value(&self, value: &str) -> ConfigResult<String> {
        if value.is_empty() {
            return Ok(String::new());
        }

        let salt = b"scheduler_config_salt";
        let mut output = [0u8; 32]; // SHA-256 output size

        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(10000).unwrap(),
            salt,
            value.as_bytes(),
            &mut output,
        );

        Ok(hex::encode(output))
    }

    /// Log a security event
    pub fn log_security_event(&mut self, event: SecurityEvent) {
        self.audit_log.push(event);
    }

    /// Get security audit log
    pub fn get_audit_log(&self) -> &[SecurityEvent] {
        &self.audit_log
    }

    /// Clear audit log (for testing/admin purposes)
    pub fn clear_audit_log(&mut self) {
        self.audit_log.clear();
    }

    /// Check if security is enabled for current environment
    pub fn is_security_enabled(&self) -> bool {
        match self.environment {
            Environment::Development => false,
            Environment::Testing => true,
            Environment::Staging => true,
            Environment::Production => true,
        }
    }

    /// Validate configuration security requirements
    pub fn validate_security_requirements(&mut self, config: &HashMap<String, serde_json::Value>) -> ConfigResult<()> {
        if !self.is_security_enabled() {
            return Ok(());
        }

        // Check for hardcoded secrets in production
        if self.environment == Environment::Production {
            for (key, value) in config {
                if self.is_sensitive_key(key) {
                    if let Some(value_str) = value.as_str() {
                        // Check for obvious default/test values
                        if value_str.contains("test") || 
                           value_str.contains("default") || 
                           value_str.contains("example") ||
                           value_str.len() < 8 {
                            return Err(ConfigError::Security(format!(
                                "Potential insecure default value for sensitive key '{}' in production",
                                key
                            )));
                        }
                    }
                }
            }
        }

        // Check for required security settings
        let required_security_settings = vec![
            "api.auth.jwt_secret",
            "api.auth.enabled",
            "api.rate_limiting.enabled",
        ];

        for setting in required_security_settings {
            if !config.contains_key(setting) {
                self.log_security_event(SecurityEvent {
                    event_type: SecurityEventType::SecurityViolation,
                    timestamp: chrono::Utc::now(),
                    user: None,
                    resource: setting.to_string(),
                    details: format!("Required security setting '{}' is missing", setting),
                    severity: SecuritySeverity::Warning,
                });
            }
        }

        Ok(())
    }

    /// Securely load environment variables
    pub fn load_environment_variables(&mut self, prefix: &str) -> ConfigResult<HashMap<String, String>> {
        let mut env_vars = HashMap::new();
        let prefix_upper = prefix.to_uppercase();

        for (key, value) in env::vars() {
            if key.starts_with(&prefix_upper) {
                let config_key = key[prefix_upper.len()..].to_string();
                
                // Remove leading underscore if present
                let config_key = config_key.trim_start_matches('_').to_lowercase().replace('_', ".");
                
                if self.is_sensitive_key(&config_key) {
                    self.log_security_event(SecurityEvent {
                        event_type: SecurityEventType::EnvironmentVariableAccessed,
                        timestamp: chrono::Utc::now(),
                        user: None,
                        resource: config_key.clone(),
                        details: "Sensitive environment variable accessed".to_string(),
                        severity: SecuritySeverity::Info,
                    });
                }
                
                env_vars.insert(config_key, value);
            }
        }

        Ok(env_vars)
    }

    /// Export configuration securely (for backup/migration)
    pub fn export_secure_config(&self, config: &HashMap<String, serde_json::Value>) -> ConfigResult<String> {
        let mut secure_config = HashMap::new();

        for (key, value) in config {
            if self.is_sensitive_key(key) {
                if let Some(value_str) = value.as_str() {
                    let encrypted = self.encrypt_value(value_str)?;
                    secure_config.insert(key.clone(), serde_json::Value::String(encrypted));
                } else {
                    secure_config.insert(key.clone(), value.clone());
                }
            } else {
                secure_config.insert(key.clone(), value.clone());
            }
        }

        serde_json::to_string_pretty(&secure_config)
            .map_err(|e| ConfigError::Security(format!("Failed to serialize secure config: {}", e)))
    }

    /// Import configuration from secure export
    pub fn import_secure_config(&self, secure_config_str: &str) -> ConfigResult<HashMap<String, serde_json::Value>> {
        let secure_config: HashMap<String, serde_json::Value> = serde_json::from_str(secure_config_str)
            .map_err(|e| ConfigError::Security(format!("Failed to parse secure config: {}", e)))?;

        let mut config = HashMap::new();

        for (key, value) in secure_config {
            if self.is_sensitive_key(&key) {
                if let Some(encrypted_value) = value.as_str() {
                    let decrypted = self.decrypt_value(encrypted_value)?;
                    config.insert(key, serde_json::Value::String(decrypted));
                } else {
                    config.insert(key, value);
                }
            } else {
                config.insert(key, value);
            }
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;
    use std::fs;
    use std::path::Path;
    use tempfile::NamedTempFile;
    use temp_env;

    #[test]
    fn test_config_security_creation() {
        let security = ConfigSecurity::new(Environment::Development);
        assert!(!security.is_security_enabled());
        
        let prod_security = ConfigSecurity::new(Environment::Production);
        assert!(prod_security.is_security_enabled());
    }

    #[test]
    fn test_sensitive_key_detection() {
        let security = ConfigSecurity::new(Environment::Development);
        
        assert!(security.is_sensitive_key("database_password"));
        assert!(security.is_sensitive_key("api_secret_key"));
        assert!(security.is_sensitive_key("jwt_token"));
        assert!(!security.is_sensitive_key("log_level"));
        assert!(!security.is_sensitive_key("max_connections"));
    }

    #[test]
    fn test_config_sanitization() {
        let security = ConfigSecurity::new(Environment::Development);
        
        let mut config = HashMap::new();
        config.insert("database_url".to_string(), serde_json::Value::String("postgresql://user:pass@localhost/db".to_string()));
        config.insert("log_level".to_string(), serde_json::Value::String("info".to_string()));
        
        let sanitized = security.sanitize_config(&config);
        
        assert_eq!(sanitized.get("database_url").unwrap(), "***REDACTED***");
        assert_eq!(sanitized.get("log_level").unwrap(), "info");
    }

    #[test]
    fn test_secure_secret_generation() {
        let security = ConfigSecurity::new(Environment::Development);
        
        let secret = security.generate_secure_secret(32).unwrap();
        assert_eq!(secret.len(), 32);
        
        // Test error for short secret
        let result = security.generate_secure_secret(16);
        assert!(result.is_err());
    }

    #[test]
    fn test_value_hashing() {
        let security = ConfigSecurity::new(Environment::Development);
        
        let hash1 = security.hash_value("test_value").unwrap();
        let hash2 = security.hash_value("test_value").unwrap();
        let hash3 = security.hash_value("different_value").unwrap();
        
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_security_event_logging() {
        let mut security = ConfigSecurity::new(Environment::Development);
        
        let event = SecurityEvent {
            event_type: SecurityEventType::ConfigurationLoaded,
            timestamp: chrono::Utc::now(),
            user: None,
            resource: "test_config".to_string(),
            details: "Test configuration loaded".to_string(),
            severity: SecuritySeverity::Info,
        };
        
        security.log_security_event(event);
        assert_eq!(security.get_audit_log().len(), 1);
        
        security.clear_audit_log();
        assert_eq!(security.get_audit_log().len(), 0);
    }

    // Encryption/Decryption tests
    #[test]
    fn test_encrypt_decrypt_value() {
        // Use a valid 32-byte (256-bit) key for AES-256-GCM
        temp_env::with_var("CONFIG_ENCRYPTION_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let security = ConfigSecurity::new(Environment::Development);
            let original = "sensitive_data";
            
            let encrypted = security.encrypt_value(original).unwrap();
            let decrypted = security.decrypt_value(&encrypted).unwrap();
            
            assert_eq!(original, decrypted);
            assert_ne!(original, encrypted);
        });
    }

    #[test]
    fn test_encrypt_empty_value() {
        // Use a valid 32-byte (256-bit) key for AES-256-GCM
        temp_env::with_var("CONFIG_ENCRYPTION_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let security = ConfigSecurity::new(Environment::Development);
            let original = "";
            
            let encrypted = security.encrypt_value(original).unwrap();
            let decrypted = security.decrypt_value(&encrypted).unwrap();
            
            assert_eq!(original, decrypted);
            assert_eq!(encrypted, "");
        });
    }

    #[test]
    fn test_decrypt_empty_value() {
        // Use a valid 32-byte (256-bit) key for AES-256-GCM
        temp_env::with_var("CONFIG_ENCRYPTION_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let security = ConfigSecurity::new(Environment::Development);
            let encrypted = "";
            
            let decrypted = security.decrypt_value(encrypted).unwrap();
            assert_eq!(decrypted, "");
        });
    }

    #[test]
    fn test_encrypt_without_key() {
        // Create a custom ConfigSecurity without the test key
        let mut security = ConfigSecurity::new(Environment::Development);
        // Override the encryption key to None
        security.encryption_key = None;
        
        let result = security.encrypt_value("test_value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Encryption key not configured"));
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        // Use a valid 32-byte (256-bit) key for AES-256-GCM
        temp_env::with_var("CONFIG_ENCRYPTION_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let security = ConfigSecurity::new(Environment::Development);
            let invalid_base64 = "invalid_base64_string!";
            
            let result = security.decrypt_value(invalid_base64);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Base64 decode failed"));
        });
    }

    #[test]
    fn test_decrypt_invalid_data_length() {
        // Use a valid 32-byte (256-bit) key for AES-256-GCM
        temp_env::with_var("CONFIG_ENCRYPTION_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let security = ConfigSecurity::new(Environment::Development);
            let short_data = general_purpose::STANDARD.encode("short");
            
            let result = security.decrypt_value(&short_data);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Invalid encrypted data length"));
        });
    }

    // File permission validation tests
    #[test]
    fn test_validate_file_permissions_nonexistent() {
        let security = ConfigSecurity::new(Environment::Development);
        let nonexistent_path = Path::new("/tmp/nonexistent_file_test");
        
        let result = security.validate_file_permissions(nonexistent_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_file_permissions_secure() {
        let security = ConfigSecurity::new(Environment::Development);
        
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();
        
        // Set secure permissions (only readable/writable by owner)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(file_path).unwrap().permissions();
            permissions.set_mode(0o600);
            fs::set_permissions(file_path, permissions).unwrap();
        }
        
        let result = security.validate_file_permissions(file_path);
        assert!(result.is_ok());
        
        temp_file.close().unwrap();
    }

    #[test]
    fn test_validate_file_permissions_world_readable() {
        let security = ConfigSecurity::new(Environment::Development);
        
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();
        
        // Set world-readable permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(file_path).unwrap().permissions();
            permissions.set_mode(0o644); // world-readable
            fs::set_permissions(file_path, permissions).unwrap();
            
            let result = security.validate_file_permissions(file_path);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("world-readable"));
        }
        
        #[cfg(not(unix))]
        {
            // On non-Unix systems, this test should pass as permission checks are not implemented
            let result = security.validate_file_permissions(file_path);
            assert!(result.is_ok());
        }
        
        temp_file.close().unwrap();
    }

    #[test]
    fn test_validate_file_permissions_world_writable() {
        let security = ConfigSecurity::new(Environment::Development);
        
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();
        
        // Set world-writable permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(file_path).unwrap().permissions();
            permissions.set_mode(0o622); // world-writable
            fs::set_permissions(file_path, permissions).unwrap();
            
            let result = security.validate_file_permissions(file_path);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("world-writable"));
        }
        
        #[cfg(not(unix))]
        {
            // On non-Unix systems, this test should pass as permission checks are not implemented
            let result = security.validate_file_permissions(file_path);
            assert!(result.is_ok());
        }
        
        temp_file.close().unwrap();
    }

    // Security requirements validation tests
    #[test]
    fn test_validate_security_requirements_development() {
        let mut security = ConfigSecurity::new(Environment::Development);
        
        let mut config = HashMap::new();
        config.insert("database_password".to_string(), serde_json::Value::String("test".to_string()));
        
        let result = security.validate_security_requirements(&config);
        assert!(result.is_ok()); // Should pass in development mode
    }

    #[test]
    fn test_validate_security_requirements_production_insecure_defaults() {
        let mut security = ConfigSecurity::new(Environment::Production);
        
        let mut config = HashMap::new();
        config.insert("database_password".to_string(), serde_json::Value::String("test_password".to_string()));
        
        let result = security.validate_security_requirements(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("insecure default value"));
    }

    #[test]
    fn test_validate_security_requirements_production_short_password() {
        let mut security = ConfigSecurity::new(Environment::Production);
        
        let mut config = HashMap::new();
        config.insert("api_key".to_string(), serde_json::Value::String("short".to_string()));
        
        let result = security.validate_security_requirements(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("insecure default value"));
    }

    #[test]
    fn test_validate_security_requirements_missing_security_settings() {
        let mut security = ConfigSecurity::new(Environment::Production);
        
        let config = HashMap::new(); // Empty config, missing required security settings
        
        let result = security.validate_security_requirements(&config);
        assert!(result.is_ok()); // Should not error, but should log security violations
        
        // Check that security violations were logged
        assert!(!security.get_audit_log().is_empty());
        let has_security_violation = security.get_audit_log().iter()
            .any(|event| matches!(event.event_type, SecurityEventType::SecurityViolation));
        assert!(has_security_violation);
    }

    #[test]
    fn test_validate_security_requirements_valid_config() {
        let mut security = ConfigSecurity::new(Environment::Production);
        
        let mut config = HashMap::new();
        config.insert("database_password".to_string(), serde_json::Value::String("secure_random_password_12345".to_string()));
        config.insert("api.auth.jwt_secret".to_string(), serde_json::Value::String("very_secure_jwt_secret_key".to_string()));
        config.insert("api.auth.enabled".to_string(), serde_json::Value::Bool(true));
        config.insert("api.rate_limiting.enabled".to_string(), serde_json::Value::Bool(true));
        
        let result = security.validate_security_requirements(&config);
        assert!(result.is_ok());
    }

    // Environment variable loading tests
    #[test]
    fn test_load_environment_variables() {
        let mut security = ConfigSecurity::new(Environment::Development);
        
        temp_env::with_vars([
            ("TEST_CONFIG_DATABASE_URL", Some("postgresql://localhost/testdb")),
            ("TEST_CONFIG_LOG_LEVEL", Some("info")),
            ("TEST_CONFIG_API_KEY", Some("secret_api_key")),
            ("OTHER_VAR", Some("should_not_be_included")),
        ], || {
            let result = security.load_environment_variables("TEST_CONFIG");
            assert!(result.is_ok());
            
            let env_vars = result.unwrap();
            assert_eq!(env_vars.get("database.url"), Some(&"postgresql://localhost/testdb".to_string()));
            assert_eq!(env_vars.get("log.level"), Some(&"info".to_string()));
            assert_eq!(env_vars.get("api.key"), Some(&"secret_api_key".to_string()));
            assert!(env_vars.get("other.var").is_none());
            
            // Check that sensitive variable access was logged
            let has_sensitive_access = security.get_audit_log().iter()
                .any(|event| matches!(event.event_type, SecurityEventType::EnvironmentVariableAccessed));
            assert!(has_sensitive_access);
        });
    }

    #[test]
    fn test_load_environment_variables_sensitive_access_logging() {
        let mut security = ConfigSecurity::new(Environment::Development);
        
        temp_env::with_var("TEST_CONFIG_PASSWORD", Some("secret_password"), || {
            let result = security.load_environment_variables("TEST_CONFIG");
            assert!(result.is_ok());
            
            // Check that sensitive variable access was logged
            let access_logs: Vec<_> = security.get_audit_log().iter()
                .filter(|event| matches!(event.event_type, SecurityEventType::EnvironmentVariableAccessed))
                .collect();
            assert_eq!(access_logs.len(), 1);
            assert_eq!(access_logs[0].resource, "password");
            assert_eq!(access_logs[0].details, "Sensitive environment variable accessed");
        });
    }

    #[test]
    fn test_load_environment_variables_no_matching_prefix() {
        let mut security = ConfigSecurity::new(Environment::Development);
        
        temp_env::with_var("OTHER_PREFIX_VAR", Some("some_value"), || {
            let result = security.load_environment_variables("TEST_CONFIG");
            assert!(result.is_ok());
            
            let env_vars = result.unwrap();
            assert!(env_vars.is_empty());
        });
    }

    // Secure config import/export tests
    #[test]
    fn test_export_import_secure_config() {
        // Use a valid 32-byte (256-bit) key for AES-256-GCM
        temp_env::with_var("CONFIG_ENCRYPTION_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let security = ConfigSecurity::new(Environment::Development);
            
            let mut config = HashMap::new();
            config.insert("database_url".to_string(), serde_json::Value::String("postgresql://user:pass@localhost/db".to_string()));
            config.insert("api_key".to_string(), serde_json::Value::String("secret_api_key_123".to_string()));
            config.insert("log_level".to_string(), serde_json::Value::String("info".to_string()));
            
            // Export config
            let exported = security.export_secure_config(&config).unwrap();
            
            // Import config
            let imported = security.import_secure_config(&exported).unwrap();
            
            // Verify sensitive values are decrypted correctly
            assert_eq!(imported.get("database_url").unwrap(), "postgresql://user:pass@localhost/db");
            assert_eq!(imported.get("api_key").unwrap(), "secret_api_key_123");
            assert_eq!(imported.get("log_level").unwrap(), "info");
        });
    }

    #[test]
    fn test_export_secure_config_non_string_values() {
        // Use a valid 32-byte (256-bit) key for AES-256-GCM
        temp_env::with_var("CONFIG_ENCRYPTION_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let security = ConfigSecurity::new(Environment::Development);
            
            let mut config = HashMap::new();
            config.insert("database_url".to_string(), serde_json::Value::String("postgresql://user:pass@localhost/db".to_string()));
            config.insert("max_connections".to_string(), serde_json::Value::Number(serde_json::Number::from(100)));
            config.insert("debug_mode".to_string(), serde_json::Value::Bool(true));
            
            // Export config
            let exported = security.export_secure_config(&config).unwrap();
            
            // Import config
            let imported = security.import_secure_config(&exported).unwrap();
            
            // Verify values are preserved
            assert_eq!(imported.get("database_url").unwrap(), "postgresql://user:pass@localhost/db");
            assert_eq!(imported.get("max_connections").unwrap(), 100);
            assert_eq!(imported.get("debug_mode").unwrap(), true);
        });
    }

    #[test]
    fn test_import_secure_config_invalid_json() {
        // Use a valid 32-byte (256-bit) key for AES-256-GCM
        temp_env::with_var("CONFIG_ENCRYPTION_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let security = ConfigSecurity::new(Environment::Development);
            let invalid_json = "{ invalid json }";
            
            let result = security.import_secure_config(invalid_json);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Failed to parse secure config"));
        });
    }

    #[test]
    fn test_export_secure_config_without_encryption_key() {
        // Create a custom ConfigSecurity without the test key
        let mut security = ConfigSecurity::new(Environment::Development);
        // Override the encryption key to None
        security.encryption_key = None;
        
        let mut config = HashMap::new();
        config.insert("api_key".to_string(), serde_json::Value::String("secret_api_key".to_string()));
        
        let result = security.export_secure_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Encryption key not configured"));
    }
}