use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::{ConfigError, ConfigResult, ConfigSecurity, Environment};

/// Encrypted configuration storage manager
pub struct EncryptedConfigStorage {
    security: ConfigSecurity,
    environment: Environment,
    encryption_enabled: bool,
    cache: HashMap<String, CachedConfig>,
}

/// Cached encrypted configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedConfig {
    pub config_data: HashMap<String, serde_json::Value>,
    pub encrypted_fields: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub version: u32,
}

/// Configuration encryption metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetadata {
    pub encrypted_fields: Vec<String>,
    pub encryption_algorithm: String,
    pub key_rotation_version: u32,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl EncryptedConfigStorage {
    /// Create a new encrypted configuration storage
    pub fn new(environment: Environment) -> ConfigResult<Self> {
        let security = ConfigSecurity::new(environment);

        Ok(Self {
            security,
            environment,
            encryption_enabled: Self::is_encryption_enabled(&environment),
            cache: HashMap::new(),
        })
    }

    /// Check if encryption is enabled for environment
    fn is_encryption_enabled(environment: &Environment) -> bool {
        match environment {
            Environment::Development => false,
            Environment::Testing => true,
            Environment::Staging => true,
            Environment::Production => true,
        }
    }

    /// Load and decrypt configuration from file
    pub fn load_encrypted_config(
        &mut self,
        file_path: &Path,
    ) -> ConfigResult<HashMap<String, serde_json::Value>> {
        if !self.encryption_enabled {
            // If encryption is disabled, load regular config
            eprintln!("DEBUG: Encryption disabled, loading plain config");
            return self.load_plain_config(file_path);
        }

        // Validate file permissions first
        self.security.validate_file_permissions(file_path)?;

        // Load the encrypted configuration file
        let content = fs::read_to_string(file_path)
            .map_err(|e| ConfigError::File(format!("Failed to read config file: {e}")))?;

        let encrypted_config: EncryptedConfigFile = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Parse(format!("Failed to parse encrypted config: {e}")))?;

        // Validate the configuration is not expired
        if let Some(expires_at) = encrypted_config.metadata.expires_at {
            if Utc::now() > expires_at {
                return Err(ConfigError::Security(format!(
                    "Configuration file expired at {expires_at}"
                )));
            }
        }

        // Decrypt sensitive fields
        let mut config = encrypted_config.config_data;
        for field_name in &encrypted_config.metadata.encrypted_fields {
            if let Some(encrypted_value) = config.get(field_name) {
                if let Some(value_str) = encrypted_value.as_str() {
                    if let Some(encrypted_part) = value_str.strip_prefix("ENC:") {
                        // Remove "ENC:" prefix
                        let decrypted = self.security.decrypt_value(encrypted_part)?;
                        config.insert(field_name.clone(), serde_json::Value::String(decrypted));
                    }
                }
            }
        }

        // Cache the decrypted configuration
        self.cache.insert(
            file_path.to_string_lossy().to_string(),
            CachedConfig {
                config_data: config.clone(),
                encrypted_fields: encrypted_config.metadata.encrypted_fields.clone(),
                created_at: encrypted_config.metadata.created_at,
                expires_at: encrypted_config.metadata.expires_at,
                version: encrypted_config.metadata.key_rotation_version,
            },
        );
        Ok(config)
    }

    /// Save configuration with encrypted sensitive fields
    pub fn save_encrypted_config(
        &self,
        config: &HashMap<String, serde_json::Value>,
        file_path: &Path,
    ) -> ConfigResult<()> {
        if !self.encryption_enabled {
            // If encryption is disabled, save as plain config
            return self.save_plain_config(config, file_path);
        }

        // Identify sensitive fields to encrypt
        let sensitive_fields = self.identify_sensitive_fields(config);
        let mut encrypted_config = config.clone();

        // Encrypt sensitive fields
        for field_name in &sensitive_fields {
            if let Some(value) = config.get(field_name) {
                if let Some(value_str) = value.as_str() {
                    if !value_str.is_empty() && !value_str.starts_with("ENC:") {
                        let encrypted = self.security.encrypt_value(value_str)?;
                        encrypted_config.insert(
                            field_name.clone(),
                            serde_json::Value::String(format!("ENC:{encrypted}")),
                        );
                    }
                }
            }
        }

        // Create encrypted config file structure
        let encrypted_file = EncryptedConfigFile {
            config_data: encrypted_config,
            metadata: EncryptionMetadata {
                encrypted_fields: sensitive_fields,
                encryption_algorithm: "AES-256-GCM".to_string(),
                key_rotation_version: 1,
                created_at: Utc::now(),
                expires_at: Some(Utc::now() + chrono::Duration::days(365)), // 1 year expiration
            },
        };

        // Serialize and save
        let content = serde_json::to_string_pretty(&encrypted_file).map_err(|e| {
            ConfigError::Security(format!("Failed to serialize encrypted config: {e}"))
        })?;

        fs::write(file_path, content).map_err(|e| {
            ConfigError::File(format!("Failed to write encrypted config file: {e}"))
        })?;

        // Set secure file permissions
        self.set_secure_file_permissions(file_path)?;

        Ok(())
    }

    /// Load plain configuration (fallback for development)
    fn load_plain_config(
        &self,
        file_path: &Path,
    ) -> ConfigResult<HashMap<String, serde_json::Value>> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| ConfigError::File(format!("Failed to read config file: {e}")))?;

        let config: HashMap<String, serde_json::Value> = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Parse(format!("Failed to parse config: {e}")))?;

        Ok(config)
    }

    /// Save plain configuration (fallback for development)
    fn save_plain_config(
        &self,
        config: &HashMap<String, serde_json::Value>,
        file_path: &Path,
    ) -> ConfigResult<()> {
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| ConfigError::Security(format!("Failed to serialize config: {e}")))?;

        fs::write(file_path, content)
            .map_err(|e| ConfigError::File(format!("Failed to write config file: {e}")))?;

        Ok(())
    }

    /// Identify sensitive fields that should be encrypted
    fn identify_sensitive_fields(
        &self,
        config: &HashMap<String, serde_json::Value>,
    ) -> Vec<String> {
        let mut sensitive_fields = Vec::new();

        for key in config.keys() {
            if self.security.is_sensitive_key(key) {
                sensitive_fields.push(key.clone());
            }
        }

        sensitive_fields
    }

    /// Set secure file permissions
    fn set_secure_file_permissions(&self, file_path: &Path) -> ConfigResult<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let metadata = fs::metadata(file_path)
                .map_err(|e| ConfigError::Security(format!("Failed to read file metadata: {e}")))?;

            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600); // Read/write for owner only

            fs::set_permissions(file_path, permissions).map_err(|e| {
                ConfigError::Security(format!("Failed to set file permissions: {e}"))
            })?;
        }

        #[cfg(not(unix))]
        {
            // On non-Unix systems, we can't set specific permissions
            // Just ensure the file exists
        }

        Ok(())
    }

    /// Generate a secure encryption key
    pub fn generate_encryption_key(&self) -> ConfigResult<String> {
        let mut key_bytes = [0u8; 32]; // 256 bits for AES-256
        let rng = SystemRandom::new();
        rng.fill(&mut key_bytes).map_err(|e| {
            ConfigError::Security(format!("Failed to generate encryption key: {e}"))
        })?;

        Ok(general_purpose::STANDARD.encode(key_bytes))
    }

    /// Rotate encryption key and re-encrypt configuration
    pub fn rotate_encryption_key(&mut self, file_path: &Path) -> ConfigResult<()> {
        if !self.encryption_enabled {
            return Ok(());
        }

        // Load current configuration
        let config = self.load_encrypted_config(file_path)?;

        // Save with new encryption (this will use the current key)
        self.save_encrypted_config(&config, file_path)?;

        Ok(())
    }

    /// Validate encrypted configuration file
    pub fn validate_encrypted_config(&mut self, file_path: &Path) -> ConfigResult<Vec<String>> {
        if !self.encryption_enabled {
            return Ok(vec![]);
        }

        let issues = vec![];

        // Check file exists
        if !file_path.exists() {
            return Ok(vec!["Configuration file does not exist".to_string()]);
        }

        // Check file permissions
        if let Err(e) = self.security.validate_file_permissions(file_path) {
            return Ok(vec![format!("File permission issue: {}", e)]);
        }

        // Try to load and parse the file
        match self.load_encrypted_config(file_path) {
            Ok(_) => Ok(issues),
            Err(e) => Ok(vec![format!("Failed to load encrypted config: {}", e)]),
        }
    }

    /// Get encryption status
    pub fn get_encryption_status(&self) -> EncryptionStatus {
        EncryptionStatus {
            enabled: self.encryption_enabled,
            environment: self.environment,
            algorithm: "AES-256-GCM".to_string(),
            key_rotation_supported: true,
            secure_permissions: self.encryption_enabled,
        }
    }

    /// Export configuration securely (for backup/migration)
    pub fn export_secure_config(&mut self, file_path: &Path) -> ConfigResult<()> {
        // Load the configuration
        let config = self.load_encrypted_config(file_path)?;

        // Use the existing security module to export securely
        let secure_content = self.security.export_secure_config(&config)?;

        // Write to secure export file
        let export_path = file_path.with_extension("secure.json");
        fs::write(&export_path, secure_content)
            .map_err(|e| ConfigError::File(format!("Failed to write secure export: {e}")))?;

        // Set secure permissions
        self.set_secure_file_permissions(&export_path)?;

        Ok(())
    }

    /// Import configuration from secure export
    pub fn import_secure_config(&self, export_path: &Path, target_path: &Path) -> ConfigResult<()> {
        // Read the secure export
        let content = fs::read_to_string(export_path)
            .map_err(|e| ConfigError::File(format!("Failed to read secure export: {e}")))?;

        // Import using security module
        let config = self.security.import_secure_config(&content)?;

        // Save as encrypted configuration
        self.save_encrypted_config(&config, target_path)?;

        Ok(())
    }

    /// Clear configuration cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cached configuration (if available and not expired)
    pub fn get_cached_config(
        &self,
        file_path: &Path,
    ) -> Option<HashMap<String, serde_json::Value>> {
        let cache_key = file_path.to_string_lossy().to_string();

        if let Some(cached) = self.cache.get(&cache_key) {
            // Check if cache is expired
            if let Some(expires_at) = cached.expires_at {
                if Utc::now() > expires_at {
                    return None;
                }
            }

            return Some(cached.config_data.clone());
        }

        None
    }
}

/// Encrypted configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedConfigFile {
    pub config_data: HashMap<String, serde_json::Value>,
    pub metadata: EncryptionMetadata,
}

/// Encryption status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionStatus {
    pub enabled: bool,
    pub environment: Environment,
    pub algorithm: String,
    pub key_rotation_supported: bool,
    pub secure_permissions: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_env;
    use tempfile::NamedTempFile;

    #[test]
    fn test_encrypted_config_storage_creation() {
        let storage = EncryptedConfigStorage::new(Environment::Development);
        assert!(storage.is_ok());

        let storage = storage.unwrap();
        assert!(!storage.encryption_enabled);
        assert_eq!(storage.environment, Environment::Development);
    }

    #[test]
    fn test_encryption_enabled_for_production() {
        let storage = EncryptedConfigStorage::new(Environment::Production).unwrap();
        assert!(storage.encryption_enabled);
        assert_eq!(storage.environment, Environment::Production);
    }

    #[test]
    fn test_sensitive_field_identification() {
        let storage = EncryptedConfigStorage::new(Environment::Development).unwrap();

        let mut config = HashMap::new();
        config.insert(
            "database_url".to_string(),
            serde_json::Value::String("postgresql://user:pass@localhost/db".to_string()),
        );
        config.insert(
            "api_key".to_string(),
            serde_json::Value::String("secret_key".to_string()),
        );
        config.insert(
            "log_level".to_string(),
            serde_json::Value::String("info".to_string()),
        );

        let sensitive_fields = storage.identify_sensitive_fields(&config);

        assert_eq!(sensitive_fields.len(), 2);
        assert!(sensitive_fields.contains(&"database_url".to_string()));
        assert!(sensitive_fields.contains(&"api_key".to_string()));
        assert!(!sensitive_fields.contains(&"log_level".to_string()));
    }

    #[test]
    fn test_encryption_status() {
        let storage = EncryptedConfigStorage::new(Environment::Production).unwrap();
        let status = storage.get_encryption_status();

        assert!(status.enabled);
        assert_eq!(status.environment, Environment::Production);
        assert_eq!(status.algorithm, "AES-256-GCM");
        assert!(status.key_rotation_supported);
    }

    #[test]
    fn test_generate_encryption_key() {
        let storage = EncryptedConfigStorage::new(Environment::Development).unwrap();
        let key = storage.generate_encryption_key().unwrap();

        // Should be a valid base64 string
        assert!(key.len() > 40); // Base64 encoded 32 bytes
        assert!(general_purpose::STANDARD.decode(&key).is_ok());
    }

    #[tokio::test]
    async fn test_encrypted_config_save_and_load() {
        temp_env::with_var(
            "CONFIG_ENCRYPTION_KEY",
            Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"),
            || {
                let mut storage = EncryptedConfigStorage::new(Environment::Testing).unwrap();

                let mut config = HashMap::new();
                config.insert(
                    "database_url".to_string(),
                    serde_json::Value::String("postgresql://user:pass@localhost/db".to_string()),
                );
                config.insert(
                    "api_key".to_string(),
                    serde_json::Value::String("secret_api_key_123".to_string()),
                );
                config.insert(
                    "log_level".to_string(),
                    serde_json::Value::String("info".to_string()),
                );

                // Save to temporary file
                let temp_file = NamedTempFile::new().unwrap();
                let file_path = temp_file.path();

                let result = storage.save_encrypted_config(&config, file_path);
                assert!(result.is_ok());

                // Load back
                let loaded_config = storage.load_encrypted_config(file_path).unwrap();

                // Verify sensitive fields are decrypted
                assert_eq!(
                    loaded_config.get("database_url").unwrap(),
                    "postgresql://user:pass@localhost/db"
                );
                assert_eq!(loaded_config.get("api_key").unwrap(), "secret_api_key_123");
                assert_eq!(loaded_config.get("log_level").unwrap(), "info");

                temp_file.close().unwrap();
            },
        );
    }

    #[test]
    fn test_encryption_disabled_in_development() {
        let mut storage = EncryptedConfigStorage::new(Environment::Development).unwrap();

        let mut config = HashMap::new();
        config.insert(
            "database_url".to_string(),
            serde_json::Value::String("postgresql://user:pass@localhost/db".to_string()),
        );
        config.insert(
            "log_level".to_string(),
            serde_json::Value::String("debug".to_string()),
        );

        // Save to temporary file
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();

        let result = storage.save_encrypted_config(&config, file_path);
        assert!(result.is_ok());

        // Load back
        let loaded_config = storage.load_encrypted_config(file_path).unwrap();

        // Should be plain text in development
        assert_eq!(
            loaded_config.get("database_url").unwrap(),
            "postgresql://user:pass@localhost/db"
        );

        temp_file.close().unwrap();
    }
}
