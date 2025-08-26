// 基础配置验证
// 简化的验证系统，遵循YAGNI原则，只保留必要功能

use crate::{ConfigError, ConfigResult, Environment};
use std::collections::HashMap;

/// Trait for configuration validation (compatible with models)
pub trait ConfigValidator {
    fn validate(&self) -> ConfigResult<()>;
}

/// 简化的验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

/// 验证错误
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field_path: String,
    pub message: String,
    pub severity: ValidationSeverity,
}

/// 验证警告
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub field_path: String,
    pub message: String,
}

/// 验证严重性级别
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    Error,
    Critical,
}

/// 基础配置验证器
pub struct BasicConfigValidator {
    environment: Environment,
}

impl BasicConfigValidator {
    pub fn new(environment: Environment) -> Self {
        Self { environment }
    }

    /// 验证配置
    pub fn validate(&self, config: &HashMap<String, serde_json::Value>) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 验证必需字段
        self.validate_required_fields(config, &mut errors);
        
        // 验证安全设置
        self.validate_security_settings(config, &mut errors, &mut warnings);
        
        // 验证环境特定设置
        self.validate_environment_settings(config, &mut errors, &mut warnings);

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// 验证必需字段
    fn validate_required_fields(
        &self,
        config: &HashMap<String, serde_json::Value>,
        errors: &mut Vec<ValidationError>,
    ) {
        let required_fields = self.get_required_fields();

        for field in required_fields {
            if !self.field_exists(config, &field) {
                errors.push(ValidationError {
                    field_path: field.clone(),
                    message: format!("Required field '{field}' is missing"),
                    severity: ValidationSeverity::Error,
                });
            }
        }
    }

    /// 获取环境所需的必需字段
    fn get_required_fields(&self) -> Vec<String> {
        match self.environment {
            Environment::Development => vec![
                "database.url".to_string(),
                "api.bind_address".to_string(),
            ],
            Environment::Testing => vec![
                "database.url".to_string(),
                "api.bind_address".to_string(),
                "api.auth.jwt_secret".to_string(),
            ],
            Environment::Staging | Environment::Production => vec![
                "database.url".to_string(),
                "api.bind_address".to_string(),
                "api.auth.jwt_secret".to_string(),
                "api.auth.enabled".to_string(),
            ],
        }
    }

    /// 验证安全设置
    fn validate_security_settings(
        &self,
        config: &HashMap<String, serde_json::Value>,
        errors: &mut Vec<ValidationError>,
        warnings: &mut Vec<ValidationWarning>,
    ) {
        // 生产环境必须启用认证
        if matches!(self.environment, Environment::Production | Environment::Staging) {
            if let Some(auth_enabled) = self.get_nested_value(config, "api.auth.enabled") {
                if !auth_enabled.as_bool().unwrap_or(false) {
                    errors.push(ValidationError {
                        field_path: "api.auth.enabled".to_string(),
                        message: "Authentication must be enabled in production/staging".to_string(),
                        severity: ValidationSeverity::Critical,
                    });
                }
            }
        }

        // 验证JWT密钥强度
        if let Some(jwt_secret) = self.get_nested_value(config, "api.auth.jwt_secret") {
            if let Some(secret_str) = jwt_secret.as_str() {
                if secret_str.len() < 32 {
                    errors.push(ValidationError {
                        field_path: "api.auth.jwt_secret".to_string(),
                        message: "JWT secret must be at least 32 characters long".to_string(),
                        severity: ValidationSeverity::Error,
                    });
                }
                
                if secret_str.contains("dev") || secret_str.contains("test") {
                    if matches!(self.environment, Environment::Production | Environment::Staging) {
                        errors.push(ValidationError {
                            field_path: "api.auth.jwt_secret".to_string(),
                            message: "Development/test secrets cannot be used in production".to_string(),
                            severity: ValidationSeverity::Critical,
                        });
                    } else {
                        warnings.push(ValidationWarning {
                            field_path: "api.auth.jwt_secret".to_string(),
                            message: "Using development/test secret".to_string(),
                        });
                    }
                }
            }
        }

        // 生产环境CORS检查
        if self.environment == Environment::Production {
            if let Some(cors_origins) = self.get_nested_value(config, "api.cors_origins") {
                if let Some(origins) = cors_origins.as_array() {
                    for origin in origins {
                        if let Some(origin_str) = origin.as_str() {
                            if origin_str == "*" {
                                errors.push(ValidationError {
                                    field_path: "api.cors_origins".to_string(),
                                    message: "Wildcard CORS origin '*' is not allowed in production".to_string(),
                                    severity: ValidationSeverity::Critical,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// 验证环境特定设置
    fn validate_environment_settings(
        &self,
        config: &HashMap<String, serde_json::Value>,
        _errors: &mut Vec<ValidationError>,
        warnings: &mut Vec<ValidationWarning>,
    ) {
        // 验证绑定地址
        if let Some(bind_addr) = self.get_nested_value(config, "api.bind_address") {
            if let Some(addr_str) = bind_addr.as_str() {
                match self.environment {
                    Environment::Development => {
                        if addr_str.starts_with("0.0.0.0") {
                            warnings.push(ValidationWarning {
                                field_path: "api.bind_address".to_string(),
                                message: "Binding to 0.0.0.0 in development may be insecure".to_string(),
                            });
                        }
                    }
                    Environment::Production => {
                        if !addr_str.starts_with("0.0.0.0") {
                            warnings.push(ValidationWarning {
                                field_path: "api.bind_address".to_string(),
                                message: "Consider binding to 0.0.0.0 for external access in production".to_string(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        // 验证日志级别
        if let Some(log_level) = self.get_nested_value(config, "observability.log_level") {
            if let Some(level_str) = log_level.as_str() {
                if level_str == "debug" && self.environment == Environment::Production {
                    warnings.push(ValidationWarning {
                        field_path: "observability.log_level".to_string(),
                        message: "Debug logging may impact performance in production".to_string(),
                    });
                }
            }
        }

        // 验证数据库连接池大小
        if let Some(max_conn) = self.get_nested_value(config, "database.max_connections") {
            if let Some(max_val) = max_conn.as_u64() {
                match self.environment {
                    Environment::Production => {
                        if max_val < 20 {
                            warnings.push(ValidationWarning {
                                field_path: "database.max_connections".to_string(),
                                message: "Low connection pool size may limit performance in production".to_string(),
                            });
                        }
                    }
                    Environment::Development => {
                        if max_val > 10 {
                            warnings.push(ValidationWarning {
                                field_path: "database.max_connections".to_string(),
                                message: "High connection pool size may be unnecessary in development".to_string(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// 检查字段是否存在（支持嵌套路径）
    fn field_exists(&self, config: &HashMap<String, serde_json::Value>, field_path: &str) -> bool {
        self.get_nested_value(config, field_path).is_some()
    }

    /// 获取嵌套值
    fn get_nested_value<'a>(&self, config: &'a HashMap<String, serde_json::Value>, path: &str) -> Option<&'a serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = config.get(parts[0])?;

        for part in &parts[1..] {
            current = current.get(part)?;
        }

        Some(current)
    }
}

/// 验证工具函数
pub struct ValidationUtils;

impl ValidationUtils {
    /// 验证字符串非空
    pub fn validate_not_empty(value: &str, field_name: &str) -> ConfigResult<()> {
        if value.trim().is_empty() {
            return Err(ConfigError::Validation(format!(
                "{field_name} cannot be empty"
            )));
        }
        Ok(())
    }

    /// 验证端口号
    pub fn validate_port(port: u16) -> ConfigResult<()> {
        if port == 0 {
            return Err(ConfigError::Validation("Port cannot be 0".to_string()));
        }
        Ok(())
    }

    /// 验证超时值（向后兼容的方法名）
    pub fn validate_timeout_seconds(timeout_seconds: u64) -> ConfigResult<()> {
        Self::validate_timeout(timeout_seconds, "timeout")
    }

    /// 验证超时值
    pub fn validate_timeout(timeout_seconds: u64, field_name: &str) -> ConfigResult<()> {
        if timeout_seconds == 0 {
            return Err(ConfigError::Validation(format!(
                "{field_name} must be greater than 0"
            )));
        }
        if timeout_seconds > 3600 {
            return Err(ConfigError::Validation(format!(
                "{field_name} must be less than or equal to 3600 seconds"
            )));
        }
        Ok(())
    }

    /// 验证计数值（向后兼容的重载）
    pub fn validate_count_simple(count: usize, field_name: &str) -> ConfigResult<()> {
        Self::validate_count(count, field_name, 10000)
    }

    /// 验证URL格式
    pub fn validate_url(url: &str, field_name: &str) -> ConfigResult<()> {
        Self::validate_not_empty(url, field_name)?;
        
        if !url.contains("://") {
            return Err(ConfigError::Validation(format!(
                "{field_name} must be a valid URL with protocol"
            )));
        }
        
        Ok(())
    }

    /// 验证计数值
    pub fn validate_count(count: usize, field_name: &str, max_value: usize) -> ConfigResult<()> {
        if count == 0 {
            return Err(ConfigError::Validation(format!(
                "{field_name} must be greater than 0"
            )));
        }
        if count > max_value {
            return Err(ConfigError::Validation(format!(
                "{field_name} must be less than or equal to {max_value}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_validation() {
        let validator = BasicConfigValidator::new(Environment::Development);
        let config = HashMap::from([
            ("database".to_string(), json!({
                "url": "postgresql://localhost/test"
            })),
            ("api".to_string(), json!({
                "bind_address": "127.0.0.1:8080"
            })),
        ]);

        let result = validator.validate(&config);
        if !result.is_valid {
            println!("Validation errors: {:?}", result.errors);
        }
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_missing_required_field() {
        let validator = BasicConfigValidator::new(Environment::Production);
        let config = HashMap::from([
            ("database".to_string(), json!({
                "url": "postgresql://localhost/test"
            })),
        ]);

        let result = validator.validate(&config);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validation_utils() {
        assert!(ValidationUtils::validate_not_empty("test", "field").is_ok());
        assert!(ValidationUtils::validate_not_empty("", "field").is_err());
        
        assert!(ValidationUtils::validate_port(8080).is_ok());
        assert!(ValidationUtils::validate_port(0).is_err());
        
        assert!(ValidationUtils::validate_timeout(30, "timeout").is_ok());
        assert!(ValidationUtils::validate_timeout(0, "timeout").is_err());
        assert!(ValidationUtils::validate_timeout(3601, "timeout").is_err());
    }
}