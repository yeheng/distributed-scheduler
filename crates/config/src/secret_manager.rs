use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use ring::rand::{SecureRandom, SystemRandom};
use base64::{Engine as _, engine::general_purpose};

use crate::{ConfigError, ConfigResult, Environment};

/// Secret management system for handling secure storage and rotation of secrets
pub struct SecretManager {
    secrets: Arc<RwLock<HashMap<String, SecretEntry>>>,
    encryption_key: Vec<u8>,
    environment: Environment,
    rotation_interval: Duration,
}

/// Secret entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretEntry {
    pub name: String,
    pub encrypted_value: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub rotation_count: u32,
    pub version: u32,
    pub secret_type: SecretType,
}

/// Secret types for different handling strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretType {
    JwtSecret,
    DatabasePassword,
    ApiKey,
    EncryptionKey,
    Certificate,
    OAuthSecret,
    Custom(String),
}

/// Secret rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationPolicy {
    pub enabled: bool,
    pub interval_days: u32,
    pub warning_days_before: u32,
    pub auto_rotate: bool,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_days: 90, // 90 days default
            warning_days_before: 7,
            auto_rotate: false,
        }
    }
}

/// Secret validation result
#[derive(Debug, Clone)]
pub struct SecretValidation {
    pub is_valid: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub expires_in: Option<Duration>,
}

impl SecretManager {
    /// Create a new secret manager
    pub async fn new(environment: Environment) -> ConfigResult<Self> {
        let encryption_key = Self::load_encryption_key()?;
        
        Ok(Self {
            secrets: Arc::new(RwLock::new(HashMap::new())),
            encryption_key,
            environment,
            rotation_interval: Duration::days(90),
        })
    }

    /// Load encryption key from environment or secure storage
    fn load_encryption_key() -> ConfigResult<Vec<u8>> {
        // For testing, use a fixed key if in test mode
        if cfg!(test) {
            return Ok(b"12345678901234567890123456789012".to_vec());
        }

        env::var("SECRET_MANAGER_KEY")
            .ok()
            .or_else(|| env::var("CONFIG_ENCRYPTION_KEY").ok())
            .map(|key| {
                let decoded = general_purpose::STANDARD.decode(&key)
                    .unwrap_or_else(|_| key.clone().into_bytes());
                if decoded.len() != 32 {
                    panic!("Encryption key must be exactly 32 bytes (256 bits)");
                }
                decoded
            })
            .ok_or_else(|| ConfigError::Security(
                "SECRET_MANAGER_KEY environment variable not set".to_string()
            ))
    }

    /// Store a secret securely
    pub async fn store_secret(
        &self,
        name: &str,
        value: &str,
        secret_type: SecretType,
        expires_in_days: Option<u32>,
    ) -> ConfigResult<()> {
        let encrypted_value = self.encrypt_value(value)?;
        let now = Utc::now();
        let expires_at = expires_in_days.map(|days| now + Duration::days(days as i64));

        let entry = SecretEntry {
            name: name.to_string(),
            encrypted_value,
            created_at: now,
            updated_at: now,
            expires_at,
            rotation_count: 0,
            version: 1,
            secret_type,
        };

        let mut secrets = self.secrets.write().await;
        secrets.insert(name.to_string(), entry);

        Ok(())
    }

    /// Retrieve a secret
    pub async fn get_secret(&self, name: &str) -> ConfigResult<Option<String>> {
        let secrets = self.secrets.read().await;
        
        match secrets.get(name) {
            Some(entry) => {
                // Check if secret is expired
                if let Some(expires_at) = entry.expires_at {
                    if Utc::now() > expires_at {
                        return Err(ConfigError::Security(format!(
                            "Secret '{}' has expired at {}", name, expires_at
                        )));
                    }
                }

                let decrypted = self.decrypt_value(&entry.encrypted_value)?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    /// Rotate a secret
    pub async fn rotate_secret(&self, name: &str) -> ConfigResult<String> {
        let mut secrets = self.secrets.write().await;
        
        match secrets.get_mut(name) {
            Some(entry) => {
                // Generate new secret value based on type
                let new_value = match entry.secret_type {
                    SecretType::JwtSecret => self.generate_jwt_secret(),
                    SecretType::DatabasePassword => self.generate_password(32),
                    SecretType::ApiKey => self.generate_api_key(),
                    SecretType::EncryptionKey => self.generate_encryption_key(),
                    SecretType::Certificate => {
                        return Err(ConfigError::Security(
                            "Certificate rotation requires manual intervention".to_string()
                        ))
                    }
                    SecretType::OAuthSecret => self.generate_oauth_secret(),
                    SecretType::Custom(_) => self.generate_password(32),
                }?;

                // Update the entry
                entry.encrypted_value = self.encrypt_value(&new_value)?;
                entry.updated_at = Utc::now();
                entry.rotation_count += 1;
                entry.version += 1;

                // Extend expiration if set
                if let Some(expires_at) = entry.expires_at {
                    entry.expires_at = Some(expires_at + Duration::days(90));
                }

                Ok(new_value)
            }
            None => Err(ConfigError::Security(format!("Secret '{}' not found", name))),
        }
    }

    /// Validate all secrets
    pub async fn validate_all_secrets(&self) -> ConfigResult<SecretValidation> {
        let secrets = self.secrets.read().await;
        let mut issues = Vec::new();
        let mut warnings = Vec::new();
        let mut min_expires_in = None;

        let now = Utc::now();

        for (name, entry) in secrets.iter() {
            // Check expiration
            if let Some(expires_at) = entry.expires_at {
                let expires_in = expires_at - now;
                
                if expires_in <= Duration::zero() {
                    issues.push(format!("Secret '{}' has expired", name));
                } else if expires_in <= Duration::days(7) {
                    warnings.push(format!("Secret '{}' expires in 7 days", name));
                }

                if min_expires_in.is_none() || expires_in < min_expires_in.unwrap() {
                    min_expires_in = Some(expires_in);
                }
            }

            // Check for weak secrets
            if let Ok(decrypted) = self.decrypt_value(&entry.encrypted_value) {
                if decrypted.len() < 8 {
                    issues.push(format!("Secret '{}' is too short", name));
                }
                if decrypted.chars().all(|c| c.is_ascii_alphanumeric()) {
                    warnings.push(format!("Secret '{}' could be stronger", name));
                }
            }
        }

        Ok(SecretValidation {
            is_valid: issues.is_empty(),
            issues,
            warnings,
            expires_in: min_expires_in,
        })
    }

    /// Generate a secure JWT secret
    fn generate_jwt_secret(&self) -> ConfigResult<String> {
        self.generate_password(64)
    }

    /// Generate a secure password
    fn generate_password(&self, length: usize) -> ConfigResult<String> {
        if length < 12 {
            return Err(ConfigError::Security(
                "Password length must be at least 12 characters".to_string()
            ));
        }

        let mut bytes = vec![0u8; length];
        let rng = SystemRandom::new();
        rng.fill(&mut bytes)
            .map_err(|e| ConfigError::Security(format!("Failed to generate secure random: {}", e)))?;

        // Use a mix of character types for better security
        let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()";
        let mut result = String::with_capacity(length);

        for &byte in &bytes {
            result.push(char::from(charset[byte as usize % charset.len()]));
        }

        Ok(result)
    }

    /// Generate an API key
    fn generate_api_key(&self) -> ConfigResult<String> {
        let prefix = "sk_";
        let key_part = self.generate_password(32)?;
        Ok(format!("{}{}", prefix, key_part))
    }

    /// Generate an encryption key
    fn generate_encryption_key(&self) -> ConfigResult<String> {
        let mut key = [0u8; 32];
        let rng = SystemRandom::new();
        rng.fill(&mut key)
            .map_err(|e| ConfigError::Security(format!("Failed to generate encryption key: {}", e)))?;
        
        Ok(general_purpose::STANDARD.encode(&key))
    }

    /// Generate an OAuth secret
    fn generate_oauth_secret(&self) -> ConfigResult<String> {
        self.generate_password(64)
    }

    /// Encrypt a value using AES-256-GCM
    fn encrypt_value(&self, value: &str) -> ConfigResult<String> {
        if value.is_empty() {
            return Ok(String::new());
        }

        use ring::{aead, aead::NonceSequence};
        
        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        let rng = SystemRandom::new();
        rng.fill(&mut nonce_bytes).map_err(|e| {
            ConfigError::Security(format!("Failed to generate nonce: {}", e))
        })?;

        // Create sealing key
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &self.encryption_key)
            .map_err(|e| ConfigError::Security(format!("Failed to create encryption key: {}", e)))?;
        
        // Simple nonce sequence
        struct SingleNonce([u8; 12]);
        impl NonceSequence for SingleNonce {
            fn advance(&mut self) -> Result<ring::aead::Nonce, ring::error::Unspecified> {
                Ok(ring::aead::Nonce::assume_unique_for_key(self.0))
            }
        }
        
        let nonce_seq = SingleNonce(nonce_bytes);
        let mut sealing_key: aead::SealingKey<SingleNonce> = aead::BoundKey::new(unbound_key, nonce_seq);

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

    /// Decrypt a value using AES-256-GCM
    fn decrypt_value(&self, encrypted_value: &str) -> ConfigResult<String> {
        if encrypted_value.is_empty() {
            return Ok(String::new());
        }

        use ring::{aead, aead::NonceSequence};
        
        let data = general_purpose::STANDARD.decode(encrypted_value)
            .map_err(|e| ConfigError::Security(format!("Base64 decode failed: {}", e)))?;

        if data.len() < 12 {
            return Err(ConfigError::Security("Invalid encrypted data length".to_string()));
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = data.split_at(12);

        // Create opening key
        let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &self.encryption_key)
            .map_err(|e| ConfigError::Security(format!("Failed to create decryption key: {}", e)))?;
        
        // Simple nonce sequence
        struct SingleNonce([u8; 12]);
        impl NonceSequence for SingleNonce {
            fn advance(&mut self) -> Result<ring::aead::Nonce, ring::error::Unspecified> {
                Ok(ring::aead::Nonce::assume_unique_for_key(self.0))
            }
        }
        
        let nonce_seq = SingleNonce(nonce_bytes.try_into().map_err(|_| {
            ConfigError::Security("Invalid nonce length".to_string())
        })?);
        let mut opening_key: aead::OpeningKey<SingleNonce> = aead::BoundKey::new(unbound_key, nonce_seq);

        // Decrypt the value
        let mut in_out = ciphertext.to_vec();
        let aad = aead::Aad::empty();
        let plaintext = opening_key.open_in_place(aad, &mut in_out)
            .map_err(|e| ConfigError::Security(format!("Decryption failed: {}", e)))?;

        Ok(String::from_utf8(plaintext.to_vec())
            .map_err(|e| ConfigError::Security(format!("UTF-8 conversion failed: {}", e)))?)
    }

    /// Load secrets from persistent storage
    pub async fn load_from_file(&self, file_path: &Path) -> ConfigResult<()> {
        if !file_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(file_path)
            .map_err(|e| ConfigError::File(format!("Failed to read secrets file: {}", e)))?;

        let secrets: HashMap<String, SecretEntry> = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Parse(format!("Failed to parse secrets file: {}", e)))?;

        let mut current_secrets = self.secrets.write().await;
        *current_secrets = secrets;

        Ok(())
    }

    /// Save secrets to persistent storage
    pub async fn save_to_file(&self, file_path: &Path) -> ConfigResult<()> {
        let secrets = self.secrets.read().await;
        
        let content = serde_json::to_string_pretty(&*secrets)
            .map_err(|e| ConfigError::Security(format!("Failed to serialize secrets: {}", e)))?;

        fs::write(file_path, content)
            .map_err(|e| ConfigError::File(format!("Failed to write secrets file: {}", e)))?;

        Ok(())
    }

    /// List all secret names
    pub async fn list_secrets(&self) -> Vec<String> {
        let secrets = self.secrets.read().await;
        secrets.keys().cloned().collect()
    }

    /// Delete a secret
    pub async fn delete_secret(&self, name: &str) -> ConfigResult<bool> {
        let mut secrets = self.secrets.write().await;
        Ok(secrets.remove(name).is_some())
    }

    /// Get secret metadata without decrypting
    pub async fn get_secret_metadata(&self, name: &str) -> Option<SecretEntry> {
        let secrets = self.secrets.read().await;
        secrets.get(name).cloned()
    }

    /// Check if a secret exists
    pub async fn has_secret(&self, name: &str) -> bool {
        let secrets = self.secrets.read().await;
        secrets.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use temp_env;

    #[tokio::test]
    async fn test_secret_manager_creation() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let manager = SecretManager::new(Environment::Development).await;
            assert!(manager.is_ok());
        });
    }

    #[tokio::test]
    async fn test_store_and_retrieve_secret() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let manager = SecretManager::new(Environment::Development).await.unwrap();
            
            let result = manager.store_secret(
                "test_secret",
                "test_value",
                SecretType::ApiKey,
                None,
            ).await;
            assert!(result.is_ok());

            let retrieved = manager.get_secret("test_secret").await;
            assert!(retrieved.is_ok());
            assert_eq!(retrieved.unwrap().unwrap(), "test_value");
        });
    }

    #[tokio::test]
    async fn test_secret_rotation() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let manager = SecretManager::new(Environment::Development).await.unwrap();
            
            manager.store_secret(
                "test_jwt",
                "old_secret",
                SecretType::JwtSecret,
                None,
            ).await.unwrap();

            let new_secret = manager.rotate_secret("test_jwt").await.unwrap();
            assert_ne!(new_secret, "old_secret");
            assert!(new_secret.len() >= 64);

            // Verify the secret was actually updated
            let retrieved = manager.get_secret("test_jwt").await.unwrap();
            assert_eq!(retrieved.unwrap(), new_secret);
        });
    }

    #[tokio::test]
    async fn test_secret_validation() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let manager = SecretManager::new(Environment::Development).await.unwrap();
            
            // Store a weak secret
            manager.store_secret(
                "weak_secret",
                "short",
                SecretType::ApiKey,
                None,
            ).await.unwrap();

            let validation = manager.validate_all_secrets().await.unwrap();
            assert!(!validation.is_valid);
            assert!(validation.issues.iter().any(|issue| issue.contains("too short")));
        });
    }

    #[tokio::test]
    async fn test_file_persistence() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let manager = SecretManager::new(Environment::Development).await.unwrap();
            
            manager.store_secret(
                "persistent_secret",
                "test_value",
                SecretType::ApiKey,
                None,
            ).await.unwrap();

            // Save to file
            let temp_file = NamedTempFile::new().unwrap();
            let file_path = temp_file.path();
            manager.save_to_file(file_path).await.unwrap();

            // Create new manager and load
            let manager2 = SecretManager::new(Environment::Development).await.unwrap();
            manager2.load_from_file(file_path).await.unwrap();

            // Verify secret was loaded
            let retrieved = manager2.get_secret("persistent_secret").await.unwrap();
            assert_eq!(retrieved.unwrap(), "test_value");
        });
    }

    #[tokio::test]
    async fn test_secret_expiration() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let manager = SecretManager::new(Environment::Development).await.unwrap();
            
            // Store an expired secret
            manager.store_secret(
                "expired_secret",
                "test_value",
                SecretType::ApiKey,
                Some(-1), // Expired yesterday
            ).await.unwrap();

            let result = manager.get_secret("expired_secret").await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("expired"));
        });
    }

    #[tokio::test]
    async fn test_secret_deletion() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let manager = SecretManager::new(Environment::Development).await.unwrap();
            
            manager.store_secret(
                "to_delete",
                "test_value",
                SecretType::ApiKey,
                None,
            ).await.unwrap();

            assert!(manager.has_secret("to_delete").await);

            let deleted = manager.delete_secret("to_delete").await.unwrap();
            assert!(deleted);
            assert!(!manager.has_secret("to_delete").await);
        });
    }
}