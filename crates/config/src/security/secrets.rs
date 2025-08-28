// 简化的密钥管理系统
// 统一管理JWT密钥和其他敏感配置，消除重复代码

use crate::{ConfigError, ConfigResult, Environment};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Duration, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 密钥类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SecretType {
    JwtSecret,
    DatabasePassword,
    ApiKey,
    EncryptionKey,
}

/// 密钥条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretEntry {
    pub secret_type: SecretType,
    pub value: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub version: u32,
}

/// 简化的密钥管理器
pub struct SimpleSecretManager {
    secrets: Arc<RwLock<HashMap<SecretType, SecretEntry>>>,
    environment: Environment,
}

impl SimpleSecretManager {
    /// 创建新的密钥管理器
    pub fn new(environment: Environment) -> Self {
        Self {
            secrets: Arc::new(RwLock::new(HashMap::new())),
            environment,
        }
    }

    /// 存储密钥
    pub async fn store_secret(
        &self,
        secret_type: SecretType,
        value: String,
        expires_in_days: Option<i64>,
    ) -> ConfigResult<()> {
        self.validate_secret(&secret_type, &value)?;

        let expires_at = expires_in_days.map(|days| Utc::now() + Duration::days(days));

        let entry = SecretEntry {
            secret_type: secret_type.clone(),
            value,
            created_at: Utc::now(),
            expires_at,
            version: 1,
        };

        let mut secrets = self.secrets.write().await;
        secrets.insert(secret_type, entry);

        Ok(())
    }

    /// 获取密钥
    pub async fn get_secret(&self, secret_type: &SecretType) -> ConfigResult<Option<String>> {
        let secrets = self.secrets.read().await;

        match secrets.get(secret_type) {
            Some(entry) => {
                // 检查是否过期
                if let Some(expires_at) = entry.expires_at {
                    if Utc::now() > expires_at {
                        return Err(ConfigError::Security(format!(
                            "Secret {:?} has expired",
                            secret_type
                        )));
                    }
                }

                Ok(Some(entry.value.clone()))
            }
            None => Ok(None),
        }
    }

    /// 生成新密钥
    pub async fn generate_secret(&self, secret_type: SecretType) -> ConfigResult<String> {
        let new_value = match secret_type {
            SecretType::JwtSecret => self.generate_jwt_secret()?,
            SecretType::DatabasePassword => self.generate_password(32)?,
            SecretType::ApiKey => self.generate_api_key()?,
            SecretType::EncryptionKey => self.generate_encryption_key()?,
        };

        // 存储生成的密钥
        let expires_days = match secret_type {
            SecretType::JwtSecret => Some(90),      // JWT密钥90天过期
            SecretType::EncryptionKey => Some(365), // 加密密钥1年过期
            _ => None,                              // 其他密钥不自动过期
        };

        self.store_secret(secret_type, new_value.clone(), expires_days)
            .await?;

        Ok(new_value)
    }

    /// 轮换密钥
    pub async fn rotate_secret(&self, secret_type: &SecretType) -> ConfigResult<String> {
        let new_value = match secret_type {
            SecretType::JwtSecret => self.generate_jwt_secret()?,
            SecretType::DatabasePassword => self.generate_password(32)?,
            SecretType::ApiKey => self.generate_api_key()?,
            SecretType::EncryptionKey => self.generate_encryption_key()?,
        };

        let mut secrets = self.secrets.write().await;

        if let Some(entry) = secrets.get_mut(secret_type) {
            entry.value = new_value.clone();
            entry.created_at = Utc::now();
            entry.version += 1;

            // 重新设置过期时间
            match secret_type {
                SecretType::JwtSecret => {
                    entry.expires_at = Some(Utc::now() + Duration::days(90));
                }
                SecretType::EncryptionKey => {
                    entry.expires_at = Some(Utc::now() + Duration::days(365));
                }
                _ => {}
            }
        } else {
            return Err(ConfigError::Security(format!(
                "Secret {:?} not found for rotation",
                secret_type
            )));
        }

        Ok(new_value)
    }

    /// 验证密钥是否有效
    fn validate_secret(&self, secret_type: &SecretType, value: &str) -> ConfigResult<()> {
        match secret_type {
            SecretType::JwtSecret => {
                if value.len() < 32 {
                    return Err(ConfigError::Validation(
                        "JWT secret must be at least 32 characters long".to_string(),
                    ));
                }

                // 生产环境不允许明显的测试密钥
                if matches!(
                    self.environment,
                    Environment::Production | Environment::Staging
                ) {
                    let test_patterns = ["test", "dev", "demo", "example", "password", "secret"];
                    let lower_value = value.to_lowercase();
                    for pattern in &test_patterns {
                        if lower_value.contains(pattern) {
                            return Err(ConfigError::Security(format!(
                                "JWT secret contains test pattern '{}' which is not allowed in production",
                                pattern
                            )));
                        }
                    }
                }
            }
            SecretType::DatabasePassword => {
                if value.len() < 12 {
                    return Err(ConfigError::Validation(
                        "Database password must be at least 12 characters long".to_string(),
                    ));
                }
            }
            SecretType::ApiKey => {
                if value.len() < 16 {
                    return Err(ConfigError::Validation(
                        "API key must be at least 16 characters long".to_string(),
                    ));
                }
            }
            SecretType::EncryptionKey => {
                // 加密密钥必须是32字节的base64编码
                if let Ok(decoded) = general_purpose::STANDARD.decode(value) {
                    if decoded.len() != 32 {
                        return Err(ConfigError::Validation(
                            "Encryption key must be 32 bytes when base64 decoded".to_string(),
                        ));
                    }
                } else {
                    return Err(ConfigError::Validation(
                        "Encryption key must be valid base64".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// 生成JWT密钥
    fn generate_jwt_secret(&self) -> ConfigResult<String> {
        let mut key = vec![0u8; 64]; // 512位密钥
        SystemRandom::new()
            .fill(&mut key)
            .map_err(|_| ConfigError::Security("Failed to generate random key".to_string()))?;

        Ok(general_purpose::STANDARD.encode(key))
    }

    /// 生成密码
    fn generate_password(&self, length: usize) -> ConfigResult<String> {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                abcdefghijklmnopqrstuvwxyz\
                                0123456789\
                                !@#$%^&*";

        let mut password = vec![0u8; length];
        SystemRandom::new()
            .fill(&mut password)
            .map_err(|_| ConfigError::Security("Failed to generate random password".to_string()))?;

        let password: String = password
            .iter()
            .map(|&byte| CHARSET[byte as usize % CHARSET.len()] as char)
            .collect();

        Ok(password)
    }

    /// 生成API密钥
    fn generate_api_key(&self) -> ConfigResult<String> {
        let mut key = vec![0u8; 32]; // 256位密钥
        SystemRandom::new()
            .fill(&mut key)
            .map_err(|_| ConfigError::Security("Failed to generate random API key".to_string()))?;

        Ok(format!("ak_{}", general_purpose::STANDARD.encode(key)))
    }

    /// 生成加密密钥
    fn generate_encryption_key(&self) -> ConfigResult<String> {
        let mut key = vec![0u8; 32]; // 256位密钥
        SystemRandom::new()
            .fill(&mut key)
            .map_err(|_| ConfigError::Security("Failed to generate encryption key".to_string()))?;

        Ok(general_purpose::STANDARD.encode(key))
    }

    /// 检查密钥是否即将过期
    pub async fn check_expiring_secrets(&self, days_before_expiry: i64) -> Vec<SecretType> {
        let secrets = self.secrets.read().await;
        let threshold = Utc::now() + Duration::days(days_before_expiry);

        secrets
            .iter()
            .filter_map(|(secret_type, entry)| {
                if let Some(expires_at) = entry.expires_at {
                    if expires_at <= threshold {
                        Some(secret_type.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// 获取所有密钥状态
    pub async fn get_secrets_status(&self) -> HashMap<SecretType, SecretStatus> {
        let secrets = self.secrets.read().await;
        let now = Utc::now();

        secrets
            .iter()
            .map(|(secret_type, entry)| {
                let status = if let Some(expires_at) = entry.expires_at {
                    if now > expires_at {
                        SecretStatus::Expired
                    } else if now + Duration::days(7) > expires_at {
                        SecretStatus::ExpiringSoon
                    } else {
                        SecretStatus::Valid
                    }
                } else {
                    SecretStatus::Valid
                };

                (secret_type.clone(), status)
            })
            .collect()
    }
}

/// 密钥状态
#[derive(Debug, Clone, PartialEq)]
pub enum SecretStatus {
    Valid,
    ExpiringSoon,
    Expired,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_secret_management() {
        let manager = SimpleSecretManager::new(Environment::Development);

        // 生成JWT密钥
        let jwt_secret = manager
            .generate_secret(SecretType::JwtSecret)
            .await
            .unwrap();
        assert!(jwt_secret.len() > 32);

        // 获取密钥
        let retrieved = manager.get_secret(&SecretType::JwtSecret).await.unwrap();
        assert_eq!(retrieved.unwrap(), jwt_secret);

        // 轮换密钥
        let new_secret = manager.rotate_secret(&SecretType::JwtSecret).await.unwrap();
        assert_ne!(new_secret, jwt_secret);
    }

    #[tokio::test]
    async fn test_secret_validation() {
        let manager = SimpleSecretManager::new(Environment::Production);

        // 测试无效的JWT密钥
        let result = manager
            .store_secret(SecretType::JwtSecret, "test".to_string(), None)
            .await;
        assert!(result.is_err());

        // 测试包含测试模式的密钥
        let result = manager
            .store_secret(
                SecretType::JwtSecret,
                "this_is_a_test_secret_key_that_should_not_be_used".to_string(),
                None,
            )
            .await;
        assert!(result.is_err());
    }
}
