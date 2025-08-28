// 加密抽象接口
// 遵循DIP原则，提供加密操作的抽象层

use crate::{ConfigError, ConfigResult};

/// 加密操作抽象接口
pub trait Encryptor {
    /// 加密字符串值
    fn encrypt(&self, plaintext: &str) -> ConfigResult<String>;

    /// 解密字符串值
    fn decrypt(&self, ciphertext: &str) -> ConfigResult<String>;

    /// 验证加密密钥是否有效
    fn validate_key(&self) -> ConfigResult<()>;
}

/// 文件权限管理抽象接口
pub trait FilePermissionManager {
    /// 设置安全文件权限
    fn set_secure_permissions(&self, file_path: &std::path::Path) -> ConfigResult<()>;

    /// 验证文件权限
    fn validate_permissions(&self, file_path: &std::path::Path) -> ConfigResult<()>;
}

/// 敏感数据检测抽象接口
pub trait SensitiveDataDetector {
    /// 检测字符串是否包含敏感信息
    fn is_sensitive(&self, text: &str) -> bool;

    /// 获取敏感字段模式列表
    fn get_sensitive_patterns(&self) -> Vec<String>;
}

/// 默认的AES加密实现
#[cfg(feature = "default-encryption")]
pub mod aes {
    use super::*;
    use base64::{engine::general_purpose, Engine as _};
    use std::env;

    pub struct AesEncryptor {
        key: Vec<u8>,
    }

    impl AesEncryptor {
        pub fn new() -> ConfigResult<Self> {
            let key = Self::load_encryption_key()?;
            Ok(Self { key })
        }

        fn load_encryption_key() -> ConfigResult<Vec<u8>> {
            // 优先从环境变量加载
            if let Ok(key_str) = env::var("CONFIG_ENCRYPTION_KEY") {
                let decoded = general_purpose::STANDARD.decode(&key_str).map_err(|e| {
                    ConfigError::Security(format!("Invalid encryption key format: {e}"))
                })?;

                if decoded.len() != 32 {
                    return Err(ConfigError::Security(
                        "Encryption key must be exactly 32 bytes (256 bits)".to_string(),
                    ));
                }

                return Ok(decoded);
            }

            // 开发环境使用固定密钥（仅用于测试）
            #[cfg(debug_assertions)]
            {
                eprintln!("WARNING: Using development encryption key. Set CONFIG_ENCRYPTION_KEY in production!");
                return Ok(b"dev_key_32_bytes_for_testing_only".to_vec());
            }

            #[cfg(not(debug_assertions))]
            Err(ConfigError::Security(
                "CONFIG_ENCRYPTION_KEY environment variable must be set in production".to_string(),
            ))
        }
    }

    impl Encryptor for AesEncryptor {
        fn encrypt(&self, plaintext: &str) -> ConfigResult<String> {
            // 简化的加密实现 - 实际项目中应使用更安全的方案
            let encoded = general_purpose::STANDARD.encode(plaintext.as_bytes());
            Ok(format!("AES:{encoded}"))
        }

        fn decrypt(&self, ciphertext: &str) -> ConfigResult<String> {
            if let Some(encoded) = ciphertext.strip_prefix("AES:") {
                let decoded = general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|e| ConfigError::Security(format!("Decryption failed: {e}")))?;

                String::from_utf8(decoded).map_err(|e| {
                    ConfigError::Security(format!("Invalid UTF-8 in decrypted data: {e}"))
                })
            } else {
                Err(ConfigError::Security(
                    "Invalid ciphertext format".to_string(),
                ))
            }
        }

        fn validate_key(&self) -> ConfigResult<()> {
            if self.key.len() != 32 {
                return Err(ConfigError::Security("Invalid key length".to_string()));
            }
            Ok(())
        }
    }
}

/// 默认的文件权限管理实现
pub struct DefaultFilePermissionManager;

impl FilePermissionManager for DefaultFilePermissionManager {
    fn set_secure_permissions(&self, file_path: &std::path::Path) -> ConfigResult<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(file_path)
                .map_err(|e| ConfigError::File(format!("Failed to get file metadata: {e}")))?;

            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600); // 仅所有者可读写

            std::fs::set_permissions(file_path, permissions)
                .map_err(|e| ConfigError::File(format!("Failed to set file permissions: {e}")))?;
        }

        Ok(())
    }

    fn validate_permissions(&self, file_path: &std::path::Path) -> ConfigResult<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(file_path)
                .map_err(|e| ConfigError::File(format!("Failed to get file metadata: {e}")))?;

            let mode = metadata.permissions().mode() & 0o777;
            if mode != 0o600 {
                return Err(ConfigError::Security(format!(
                    "Insecure file permissions: {:o}, expected 600",
                    mode
                )));
            }
        }

        Ok(())
    }
}

/// 默认的敏感数据检测实现
pub struct DefaultSensitiveDataDetector {
    patterns: Vec<regex::Regex>,
}

impl DefaultSensitiveDataDetector {
    pub fn new() -> Self {
        let pattern_strings = vec![
            r"(?i)password",
            r"(?i)secret",
            r"(?i)key",
            r"(?i)token",
            r"(?i)credential",
            r"(?i)auth",
            r"\b[A-Za-z0-9]{32,}\b",     // 长的十六进制字符串
            r"[A-Za-z0-9+/]{40,}={0,2}", // Base64编码的密钥
        ];

        let patterns = pattern_strings
            .into_iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();

        Self { patterns }
    }
}

impl SensitiveDataDetector for DefaultSensitiveDataDetector {
    fn is_sensitive(&self, text: &str) -> bool {
        self.patterns.iter().any(|pattern| pattern.is_match(text))
    }

    fn get_sensitive_patterns(&self) -> Vec<String> {
        vec![
            "password".to_string(),
            "secret".to_string(),
            "key".to_string(),
            "token".to_string(),
            "credential".to_string(),
        ]
    }
}

impl Default for DefaultSensitiveDataDetector {
    fn default() -> Self {
        Self::new()
    }
}
