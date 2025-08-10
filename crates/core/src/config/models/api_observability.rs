use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_jwt_secret() -> String {
    "your-secret-key-change-this-in-production".to_string()
}

fn default_jwt_expiration_hours() -> i64 {
    24
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub cors_enabled: bool,
    pub cors_origins: Vec<String>,
    pub request_timeout_seconds: u64,
    pub max_request_size_mb: usize,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiration_hours")]
    pub jwt_expiration_hours: i64,
    #[serde(default)]
    pub api_keys: HashMap<String, ApiKeyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    pub name: String,
    pub permissions: Vec<String>,
    pub is_active: bool,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        let mut api_keys = HashMap::new();
        api_keys.insert(
            "default-admin-key-hash".to_string(), // This should be a hashed version
            ApiKeyConfig {
                name: "default-admin".to_string(),
                permissions: vec!["Admin".to_string()],
                is_active: true,
                created_at: None,
                expires_at: None,
            },
        );

        Self {
            enabled: false,
            jwt_secret: "your-secret-key-change-this-in-production".to_string(),
            jwt_expiration_hours: 24,
            api_keys,
        }
    }
}

impl ApiConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.bind_address.is_empty() {
            return Err(anyhow::anyhow!("绑定地址不能为空"));
        }
        if !self.bind_address.contains(':') {
            return Err(anyhow::anyhow!("绑定地址格式无效，应为 host:port"));
        }

        if self.request_timeout_seconds == 0 {
            return Err(anyhow::anyhow!("请求超时时间必须大于0"));
        }

        if self.max_request_size_mb == 0 {
            return Err(anyhow::anyhow!("最大请求大小必须大于0"));
        }
        self.auth.validate()?;

        Ok(())
    }
}

impl AuthConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.enabled {
            if self.jwt_secret.is_empty() {
                return Err(anyhow::anyhow!("JWT密钥不能为空"));
            }

            if self.jwt_secret.len() < 32 {
                return Err(anyhow::anyhow!("JWT密钥长度应至少32字符"));
            }

            if self.jwt_expiration_hours <= 0 {
                return Err(anyhow::anyhow!("JWT过期时间必须大于0"));
            }
            for (hash, key_config) in &self.api_keys {
                if hash.is_empty() {
                    return Err(anyhow::anyhow!("API密钥哈希不能为空"));
                }

                if key_config.name.is_empty() {
                    return Err(anyhow::anyhow!("API密钥名称不能为空"));
                }

                if key_config.permissions.is_empty() {
                    return Err(anyhow::anyhow!("API密钥权限不能为空"));
                }
                let valid_permissions = [
                    "TaskRead",
                    "TaskWrite",
                    "TaskDelete",
                    "WorkerRead",
                    "WorkerWrite",
                    "SystemRead",
                    "SystemWrite",
                    "Admin",
                ];

                for permission in &key_config.permissions {
                    if !valid_permissions.contains(&permission.as_str()) {
                        return Err(anyhow::anyhow!(
                            "无效的权限: {}，支持的权限: {:?}",
                            permission,
                            valid_permissions
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub tracing_enabled: bool,
    pub metrics_enabled: bool,
    pub metrics_endpoint: String,
    pub log_level: String,
    pub jaeger_endpoint: Option<String>,
}

impl ObservabilityConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.log_level.to_lowercase().as_str()) {
            return Err(anyhow::anyhow!(
                "无效的日志级别: {}，支持的级别: {:?}",
                self.log_level,
                valid_log_levels
            ));
        }

        if self.metrics_endpoint.is_empty() {
            return Err(anyhow::anyhow!("指标端点不能为空"));
        }

        if !self.metrics_endpoint.starts_with('/') {
            return Err(anyhow::anyhow!("指标端点必须以'/'开头"));
        }

        Ok(())
    }
}
