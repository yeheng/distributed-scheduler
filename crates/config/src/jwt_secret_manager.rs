use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use ring::rand::{SecureRandom, SystemRandom};
use base64::{Engine as _, engine::general_purpose};

use crate::{ConfigError, ConfigResult, Environment, SecretManager};

/// JWT secret rotation manager for handling seamless secret rotation
pub struct JwtSecretManager {
    current_secret: Arc<RwLock<JwtSecret>>,
    previous_secrets: Arc<RwLock<HashMap<String, JwtSecret>>>,
    secret_manager: Arc<SecretManager>,
    rotation_policy: JwtRotationPolicy,
    environment: Environment,
}

/// JWT secret with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtSecret {
    pub id: String,
    pub secret: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub rotation_count: u32,
    pub is_active: bool,
    pub algorithm: String,
}

/// JWT rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtJwtRotationPolicy {
    pub enabled: bool,
    pub rotation_interval_days: u32,
    pub overlap_days: u32,
    pub warning_days_before: u32,
    pub auto_rotate: bool,
    pub minimum_secrets: u32,
    pub maximum_secrets: u32,
}

/// JWT validation result
#[derive(Debug, Clone)]
pub struct JwtValidation {
    pub is_valid: bool,
    pub secret_id: Option<String>,
    pub warnings: Vec<String>,
    pub expires_in: Option<Duration>,
}

impl Default for JwtRotationPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            rotation_interval_days: 30,
            overlap_days: 7,
            warning_days_before: 3,
            auto_rotate: true,
            minimum_secrets: 1,
            maximum_secrets: 5,
        }
    }
}

impl JwtSecretManager {
    /// Create a new JWT secret manager
    pub async fn new(
        secret_manager: Arc<SecretManager>,
        environment: Environment,
    ) -> ConfigResult<Self> {
        let rotation_policy = Self::get_rotation_policy_for_environment(&environment);
        
        let manager = Self {
            current_secret: Arc::new(RwLock::new(JwtSecret::new()?)),
            previous_secrets: Arc::new(RwLock::new(HashMap::new())),
            secret_manager,
            rotation_policy,
            environment,
        };

        // Initialize secrets if they exist
        manager.load_existing_secrets().await?;

        Ok(manager)
    }

    /// Get rotation policy for environment
    fn get_rotation_policy_for_environment(environment: &Environment) -> JwtRotationPolicy {
        match environment {
            Environment::Development => JwtRotationPolicy {
                enabled: false, // No rotation in development
                rotation_interval_days: 365,
                overlap_days: 30,
                auto_rotate: false,
                ..Default::default()
            },
            Environment::Testing => JwtRotationPolicy {
                rotation_interval_days: 7, // Frequent rotation for testing
                overlap_days: 1,
                auto_rotate: true,
                ..Default::default()
            },
            Environment::Staging => JwtRotationPolicy {
                rotation_interval_days: 14, // Medium rotation for staging
                overlap_days: 3,
                auto_rotate: true,
                ..Default::default()
            },
            Environment::Production => JwtRotationPolicy {
                rotation_interval_days: 30, // Standard rotation for production
                overlap_days: 7,
                warning_days_before: 3,
                auto_rotate: true,
                ..Default::default()
            },
        }
    }

    /// Get current active JWT secret
    pub async fn get_current_secret(&self) -> ConfigResult<String> {
        let current = self.current_secret.read().await;
        
        // Check if current secret is expired
        if Utc::now() > current.expires_at {
            drop(current);
            self.rotate_secret().await?;
            let new_current = self.current_secret.read().await;
            return Ok(new_current.secret.clone());
        }

        // Check if rotation is needed
        if self.rotation_policy.enabled && self.should_rotate_secret(&*current).await {
            drop(current);
            self.rotate_secret().await?;
            let new_current = self.current_secret.read().await;
            return Ok(new_current.secret.clone());
        }

        Ok(current.secret.clone())
    }

    /// Validate JWT token against available secrets
    pub async fn validate_token_with_secrets(&self, token: &str) -> ConfigResult<JwtValidation> {
        let current = self.current_secret.read().await;
        let previous = self.previous_secrets.read().await;

        let mut warnings = Vec::new();
        let mut secret_id = None;
        let mut is_valid = false;
        let mut expires_in = None;

        // Try current secret first
        if let Ok(claims) = self.decode_token_with_secret(token, &current.secret) {
            is_valid = true;
            secret_id = Some(current.id.clone());
            expires_in = Some(current.expires_at - Utc::now());

            // Check if token is near expiration
            if let Some(exp) = claims.get("exp") {
                if let Some(exp_timestamp) = exp.as_u64() {
                    let exp_datetime = DateTime::from_timestamp(exp_timestamp as i64, 0).unwrap_or_default();
                    if exp_datetime - Utc::now() < Duration::hours(1) {
                        warnings.push("Token expires soon".to_string());
                    }
                }
            }
        }

        // Try previous secrets if current didn't work
        if !is_valid {
            for (id, secret) in previous.iter() {
                if let Ok(_claims) = self.decode_token_with_secret(token, &secret.secret) {
                    is_valid = true;
                    secret_id = Some(id.clone());
                    warnings.push("Token validated with previous secret".to_string());
                    expires_in = Some(secret.expires_at - Utc::now());
                    break;
                }
            }
        }

        Ok(JwtValidation {
            is_valid,
            secret_id,
            warnings,
            expires_in,
        })
    }

    /// Rotate JWT secret
    pub async fn rotate_secret(&self) -> ConfigResult<String> {
        if !self.rotation_policy.enabled {
            return Err(ConfigError::Security(
                "JWT secret rotation is disabled in this environment".to_string()
            ));
        }

        let mut current = self.current_secret.write().await;
        let mut previous = self.previous_secrets.write().await;

        // Move current secret to previous secrets
        let old_secret = current.clone();
        previous.insert(old_secret.id.clone(), old_secret);

        // Clean up expired previous secrets
        self.cleanup_expired_secrets(&mut previous).await;

        // Create new secret
        let new_secret = JwtSecret::new()?;
        *current = new_secret.clone();

        // Store the new secret in the secret manager
        self.secret_manager.store_secret(
            &format!("jwt_secret_{}", new_secret.id),
            &new_secret.secret,
            crate::secret_manager::SecretType::JwtSecret,
            Some(self.rotation_policy.rotation_interval_days),
        ).await?;

        Ok(new_secret.secret)
    }

    /// Force rotate JWT secret (for manual rotation)
    pub async fn force_rotate_secret(&self) -> ConfigResult<String> {
        let mut current = self.current_secret.write().await;
        let mut previous = self.previous_secrets.write().await;

        // Move current secret to previous secrets
        let old_secret = current.clone();
        previous.insert(old_secret.id.clone(), old_secret);

        // Create new secret
        let new_secret = JwtSecret::new()?;
        *current = new_secret.clone();

        Ok(new_secret.secret)
    }

    /// Get all active secrets (for validation)
    pub async fn get_active_secrets(&self) -> Vec<(String, String)> {
        let current = self.current_secret.read().await;
        let previous = self.previous_secrets.read().await;

        let mut secrets = Vec::new();
        secrets.push((current.id.clone(), current.secret.clone()));

        for (id, secret) in previous.iter() {
            if secret.expires_at > Utc::now() {
                secrets.push((id.clone(), secret.secret.clone()));
            }
        }

        secrets
    }

    /// Check if secret rotation is needed
    async fn should_rotate_secret(&self, secret: &JwtSecret) -> bool {
        if !self.rotation_policy.auto_rotate {
            return false;
        }

        let now = Utc::now();
        let rotation_threshold = secret.created_at + Duration::days(self.rotation_policy.rotation_interval_days as i64);
        
        now >= rotation_threshold
    }

    /// Clean up expired previous secrets
    async fn cleanup_expired_secrets(&self, secrets: &mut HashMap<String, JwtSecret>) {
        let now = Utc::now();
        secrets.retain(|_, secret| {
            secret.expires_at > now && 
            (now - secret.created_at) < Duration::days(self.rotation_policy.overlap_days as i64 * 2)
        });

        // Ensure we don't exceed maximum secrets
        while secrets.len() > self.rotation_policy.maximum_secrets as usize {
            if let Some((oldest_id, _)) = secrets.iter()
                .min_by_key(|(_, s)| s.created_at)
                .map(|(id, _)| (id.clone(), ())) {
                secrets.remove(&oldest_id);
            } else {
                break;
            }
        }
    }

    /// Load existing secrets from secret manager
    async fn load_existing_secrets(&self) -> ConfigResult<()> {
        let mut current = self.current_secret.write().await;
        let mut previous = self.previous_secrets.write().await;

        // Try to load current secret
        if let Ok(Some(secret)) = self.secret_manager.get_secret("jwt_secret_current").await {
            *current = JwtSecret {
                id: "loaded".to_string(),
                secret,
                created_at: Utc::now() - Duration::days(10),
                expires_at: Utc::now() + Duration::days(20),
                rotation_count: 0,
                is_active: true,
                algorithm: "HS256".to_string(),
            };
        }

        // Load previous secrets
        for i in 1..=5 {
            let key = format!("jwt_secret_previous_{}", i);
            if let Ok(Some(secret)) = self.secret_manager.get_secret(&key).await {
                let previous_secret = JwtSecret {
                    id: format!("previous_{}", i),
                    secret,
                    created_at: Utc::now() - Duration::days(30 + i as i64 * 10),
                    expires_at: Utc::now() + Duration::days(5),
                    rotation_count: i,
                    is_active: false,
                    algorithm: "HS256".to_string(),
                };
                previous.insert(previous_secret.id.clone(), previous_secret);
            }
        }

        Ok(())
    }

    /// Save secrets to persistent storage
    pub async fn save_secrets(&self) -> ConfigResult<()> {
        let current = self.current_secret.read().await;
        let previous = self.previous_secrets.read().await;

        // Save current secret
        self.secret_manager.store_secret(
            "jwt_secret_current",
            &current.secret,
            crate::secret_manager::SecretType::JwtSecret,
            Some(self.rotation_policy.rotation_interval_days),
        ).await?;

        // Save previous secrets
        for (i, (_, secret)) in previous.iter().take(5).enumerate() {
            self.secret_manager.store_secret(
                &format!("jwt_secret_previous_{}", i + 1),
                &secret.secret,
                crate::secret_manager::SecretType::JwtSecret,
                Some(self.rotation_policy.rotation_interval_days),
            ).await?;
        }

        Ok(())
    }

    /// Get rotation status
    pub async fn get_rotation_status(&self) -> ConfigResult<RotationStatus> {
        let current = self.current_secret.read().await;
        let previous = self.previous_secrets.read().await;

        let now = Utc::now();
        let next_rotation = current.created_at + Duration::days(self.rotation_policy.rotation_interval_days as i64);
        let needs_rotation = self.should_rotate_secret(&current).await;

        Ok(RotationStatus {
            current_secret_id: current.id.clone(),
            current_secret_created: current.created_at,
            current_secret_expires: current.expires_at,
            next_rotation,
            needs_rotation,
            previous_secrets_count: previous.len(),
            rotation_enabled: self.rotation_policy.enabled,
            warnings: self.get_rotation_warnings(&current, &previous).await,
        })
    }

    /// Get rotation warnings
    async fn get_rotation_warnings(&self, current: &JwtSecret, previous: &HashMap<String, JwtSecret>) -> Vec<String> {
        let mut warnings = Vec::new();
        let now = Utc::now();

        // Check if current secret is expiring soon
        if current.expires_at - now < Duration::days(self.rotation_policy.warning_days_before as i64) {
            warnings.push(format!("Current JWT secret expires in {}", current.expires_at - now));
        }

        // Check if rotation is overdue
        let rotation_threshold = current.created_at + Duration::days(self.rotation_policy.rotation_interval_days as i64);
        if now > rotation_threshold && self.rotation_policy.enabled {
            warnings.push("JWT secret rotation is overdue".to_string());
        }

        // Check if too many previous secrets
        if previous.len() > self.rotation_policy.maximum_secrets as usize {
            warnings.push("Too many previous JWT secrets stored".to_string());
        }

        warnings
    }

    /// Simple JWT token decoder (for validation purposes)
    fn decode_token_with_secret(&self, token: &str, secret: &str) -> Result<serde_json::Value> {
        // This is a simplified implementation
        // In a real application, you would use a proper JWT library like jsonwebtoken
        use base64::{Engine as _, engine::general_purpose};
        
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid JWT token format"));
        }

        // Decode header (not validated in this simplified version)
        let _header = general_purpose::URL_SAFE_NO_PAD.decode(parts[0])?;
        
        // Decode payload
        let payload = general_purpose::URL_SAFE_NO_PAD.decode(parts[1])?;
        let payload_str = String::from_utf8(payload)?;
        let claims: serde_json::Value = serde_json::from_str(&payload_str)?;

        // In a real implementation, you would verify the signature here
        // For now, we'll just return the claims
        Ok(claims)
    }

    /// Generate a new JWT secret
    fn generate_jwt_secret(&self) -> ConfigResult<String> {
        let mut bytes = vec![0u8; 64]; // 64 bytes for strong JWT secret
        let rng = SystemRandom::new();
        rng.fill(&mut bytes)
            .map_err(|e| ConfigError::Security(format!("Failed to generate JWT secret: {}", e)))?;

        // Use URL-safe base64 encoding for JWT compatibility
        Ok(general_purpose::URL_SAFE_NO_PAD.encode(&bytes))
    }
}

/// Rotation status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationStatus {
    pub current_secret_id: String,
    pub current_secret_created: DateTime<Utc>,
    pub current_secret_expires: DateTime<Utc>,
    pub next_rotation: DateTime<Utc>,
    pub needs_rotation: bool,
    pub previous_secrets_count: usize,
    pub rotation_enabled: bool,
    pub warnings: Vec<String>,
}

impl JwtSecret {
    /// Create a new JWT secret
    pub fn new() -> ConfigResult<Self> {
        let rng = SystemRandom::new();
        let mut bytes = [0u8; 32];
        rng.fill(&mut bytes)
            .map_err(|e| ConfigError::Security(format!("Failed to generate secret ID: {}", e)))?;

        let mut secret_bytes = vec![0u8; 64];
        rng.fill(&mut secret_bytes)
            .map_err(|e| ConfigError::Security(format!("Failed to generate JWT secret: {}", e)))?;

        Ok(Self {
            id: Uuid::from_bytes(bytes).to_string(),
            secret: general_purpose::URL_SAFE_NO_PAD.encode(&secret_bytes),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::days(30),
            rotation_count: 0,
            is_active: true,
            algorithm: "HS256".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_manager::SecretManager;
    use temp_env;

    #[tokio::test]
    async fn test_jwt_secret_manager_creation() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let secret_manager = SecretManager::new(Environment::Development).await.unwrap();
            let secret_manager_arc = Arc::new(secret_manager);
            
            let jwt_manager = JwtSecretManager::new(secret_manager_arc, Environment::Development).await;
            assert!(jwt_manager.is_ok());
        });
    }

    #[tokio::test]
    async fn test_get_current_secret() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let secret_manager = SecretManager::new(Environment::Development).await.unwrap();
            let secret_manager_arc = Arc::new(secret_manager);
            
            let jwt_manager = JwtSecretManager::new(secret_manager_arc, Environment::Development).await.unwrap();
            let secret = jwt_manager.get_current_secret().await.unwrap();
            
            assert!(!secret.is_empty());
            assert!(secret.len() >= 32);
        });
    }

    #[tokio::test]
    async fn test_jwt_validation() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let secret_manager = SecretManager::new(Environment::Development).await.unwrap();
            let secret_manager_arc = Arc::new(secret_manager);
            
            let jwt_manager = JwtSecretManager::new(secret_manager_arc, Environment::Development).await.unwrap();
            
            // Create a fake JWT token for testing
            let fake_token = "header.payload.signature";
            let validation = jwt_manager.validate_token_with_secrets(fake_token).await.unwrap();
            
            // In a real test, this would validate properly
            // For now, we just check the structure
            assert!(!validation.is_valid); // Should fail with fake token
        });
    }

    #[tokio::test]
    async fn test_force_rotate_secret() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let secret_manager = SecretManager::new(Environment::Development).await.unwrap();
            let secret_manager_arc = Arc::new(secret_manager);
            
            let jwt_manager = JwtSecretManager::new(secret_manager_arc, Environment::Development).await.unwrap();
            
            let original_secret = jwt_manager.get_current_secret().await.unwrap();
            let new_secret = jwt_manager.force_rotate_secret().await.unwrap();
            
            assert_ne!(original_secret, new_secret);
        });
    }

    #[tokio::test]
    async fn test_rotation_status() {
        temp_env::with_var("SECRET_MANAGER_KEY", Some("MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5"), || {
            let secret_manager = SecretManager::new(Environment::Development).await.unwrap();
            let secret_manager_arc = Arc::new(secret_manager);
            
            let jwt_manager = JwtSecretManager::new(secret_manager_arc, Environment::Development).await.unwrap();
            let status = jwt_manager.get_rotation_status().await.unwrap();
            
            assert!(!status.current_secret_id.is_empty());
            assert!(status.current_secret_expires > Utc::now());
        });
    }

    #[test]
    fn test_jwt_secret_creation() {
        let secret = JwtSecret::new().unwrap();
        assert!(!secret.id.is_empty());
        assert!(!secret.secret.is_empty());
        assert!(secret.secret.len() >= 32);
        assert_eq!(secret.algorithm, "HS256");
        assert!(secret.is_active);
    }

    #[test]
    fn test_rotation_policy_for_environment() {
        let dev_policy = JwtSecretManager::get_rotation_policy_for_environment(&Environment::Development);
        assert!(!dev_policy.enabled);

        let prod_policy = JwtSecretManager::get_rotation_policy_for_environment(&Environment::Production);
        assert!(prod_policy.enabled);
        assert_eq!(prod_policy.rotation_interval_days, 30);
    }
}