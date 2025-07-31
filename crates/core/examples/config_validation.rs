//! 配置验证示例
//!
//! 此示例展示了如何使用配置验证框架来确保配置的正确性和完整性。
//! 它演示了自定义验证器、类型检查、必需字段验证等功能。

use scheduler_core::config::manager::{ConfigBuilder, ConfigSource};
use scheduler_core::config::validation::{
    ConfigValidator, ConfigValidationError, BasicConfigValidator, SchemaValidator, ValidatorRegistry
};
use serde_json::Value;

/// 自定义数据库验证器
struct DatabaseValidator {
    name: String,
}

impl DatabaseValidator {
    fn new() -> Self {
        Self {
            name: "DatabaseValidator".to_string(),
        }
    }

    /// 验证数据库URL格式
    fn validate_database_url(&self, url: &str) -> Result<(), ConfigValidationError> {
        if !url.starts_with("postgresql://") && !url.starts_with("postgres://") {
            return Err(ConfigValidationError::ValidationFailed {
                field: "database.url".to_string(),
                message: "数据库URL必须以 'postgresql://' 或 'postgres://' 开头".to_string(),
            });
        }

        if url.len() < 20 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "database.url".to_string(),
                message: "数据库URL长度不能少于20个字符".to_string(),
            });
        }

        Ok(())
    }

    /// 验证连接池配置
    fn validate_pool_config(&self, pool_size: u32, max_connections: u32) -> Result<(), ConfigValidationError> {
        if pool_size == 0 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "database.pool_size".to_string(),
                message: "连接池大小必须大于0".to_string(),
            });
        }

        if pool_size > max_connections {
            return Err(ConfigValidationError::ValidationFailed {
                field: "database.pool_size".to_string(),
                message: format!("连接池大小 ({}) 不能大于最大连接数 ({})", pool_size, max_connections),
            });
        }

        Ok(())
    }

    /// 验证超时配置
    fn validate_timeout_config(&self, timeout_seconds: u64) -> Result<(), ConfigValidationError> {
        if timeout_seconds < 5 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "database.timeout_seconds".to_string(),
                message: "超时时间不能少于5秒".to_string(),
            });
        }

        if timeout_seconds > 3600 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "database.timeout_seconds".to_string(),
                message: "超时时间不能超过3600秒（1小时）".to_string(),
            });
        }

        Ok(())
    }
}

impl ConfigValidator for DatabaseValidator {
    fn validate(&self, config: &Value) -> Result<(), ConfigValidationError> {
        // 验证数据库配置部分存在
        let database = config.get("database")
            .ok_or_else(|| ConfigValidationError::RequiredFieldMissing {
                field: "database".to_string(),
            })?;

        // 验证数据库URL
        if let Some(url_value) = database.get("url") {
            if let Some(url) = url_value.as_str() {
                self.validate_database_url(url)?;
            } else {
                return Err(ConfigValidationError::TypeMismatch {
                    field: "database.url".to_string(),
                    expected: "string".to_string(),
                    actual: "other".to_string(),
                });
            }
        } else {
            return Err(ConfigValidationError::RequiredFieldMissing {
                field: "database.url".to_string(),
            });
        }

        // 验证连接池配置
        let pool_size = database.get("pool_size")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ConfigValidationError::RequiredFieldMissing {
                field: "database.pool_size".to_string(),
            })?;

        let max_connections = database.get("max_connections")
            .and_then(|v| v.as_u64())
            .unwrap_or(pool_size * 2); // 默认值为pool_size的2倍

        self.validate_pool_config(pool_size as u32, max_connections as u32)?;

        // 验证超时配置
        if let Some(timeout_value) = database.get("timeout_seconds") {
            if let Some(timeout) = timeout_value.as_u64() {
                self.validate_timeout_config(timeout)?;
            } else {
                return Err(ConfigValidationError::TypeMismatch {
                    field: "database.timeout_seconds".to_string(),
                    expected: "number".to_string(),
                    actual: "other".to_string(),
                });
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 服务器配置验证器
struct ServerValidator {
    name: String,
}

impl ServerValidator {
    fn new() -> Self {
        Self {
            name: "ServerValidator".to_string(),
        }
    }

    /// 验证端口号
    fn validate_port(&self, port: u16) -> Result<(), ConfigValidationError> {
        if port < 1024 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "server.port".to_string(),
                message: "端口号不能小于1024（系统保留端口）".to_string(),
            });
        }

        if port > 65535 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "server.port".to_string(),
                message: "端口号不能大于65535".to_string(),
            });
        }

        Ok(())
    }

    /// 验证主机地址
    fn validate_host(&self, host: &str) -> Result<(), ConfigValidationError> {
        if host.is_empty() {
            return Err(ConfigValidationError::ValidationFailed {
                field: "server.host".to_string(),
                message: "主机地址不能为空".to_string(),
            });
        }

        if host.len() > 253 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "server.host".to_string(),
                message: "主机地址长度不能超过253个字符".to_string(),
            });
        }

        // 简单的主机名格式验证
        if !host.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == ':') {
            return Err(ConfigValidationError::ValidationFailed {
                field: "server.host".to_string(),
                message: "主机地址包含非法字符".to_string(),
            });
        }

        Ok(())
    }
}

impl ConfigValidator for ServerValidator {
    fn validate(&self, config: &Value) -> Result<(), ConfigValidationError> {
        // 验证服务器配置部分存在
        let server = config.get("server")
            .ok_or_else(|| ConfigValidationError::RequiredFieldMissing {
                field: "server".to_string(),
            })?;

        // 验证主机地址
        if let Some(host_value) = server.get("host") {
            if let Some(host) = host_value.as_str() {
                self.validate_host(host)?;
            } else {
                return Err(ConfigValidationError::TypeMismatch {
                    field: "server.host".to_string(),
                    expected: "string".to_string(),
                    actual: "other".to_string(),
                });
            }
        } else {
            return Err(ConfigValidationError::RequiredFieldMissing {
                field: "server.host".to_string(),
            });
        }

        // 验证端口号
        if let Some(port_value) = server.get("port") {
            if let Some(port) = port_value.as_u64() {
                self.validate_port(port as u16)?;
            } else {
                return Err(ConfigValidationError::TypeMismatch {
                    field: "server.port".to_string(),
                    expected: "number".to_string(),
                    actual: "other".to_string(),
                });
            }
        } else {
            return Err(ConfigValidationError::RequiredFieldMissing {
                field: "server.port".to_string(),
            });
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 应用配置验证器
struct AppValidator {
    name: String,
}

impl AppValidator {
    fn new() -> Self {
        Self {
            name: "AppValidator".to_string(),
        }
    }

    /// 验证应用名称
    fn validate_app_name(&self, name: &str) -> Result<(), ConfigValidationError> {
        if name.is_empty() {
            return Err(ConfigValidationError::ValidationFailed {
                field: "app.name".to_string(),
                message: "应用名称不能为空".to_string(),
            });
        }

        if name.len() > 100 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "app.name".to_string(),
                message: "应用名称长度不能超过100个字符".to_string(),
            });
        }

        // 应用名称只能包含字母、数字、连字符和下划线
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(ConfigValidationError::ValidationFailed {
                field: "app.name".to_string(),
                message: "应用名称只能包含字母、数字、连字符和下划线".to_string(),
            });
        }

        Ok(())
    }

    /// 验证版本号格式
    fn validate_version(&self, version: &str) -> Result<(), ConfigValidationError> {
        if version.is_empty() {
            return Err(ConfigValidationError::ValidationFailed {
                field: "app.version".to_string(),
                message: "版本号不能为空".to_string(),
            });
        }

        // 简单的语义版本验证
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return Err(ConfigValidationError::ValidationFailed {
                field: "app.version".to_string(),
                message: "版本号格式必须为 MAJOR.MINOR.PATCH".to_string(),
            });
        }

        for part in parts {
            if part.is_empty() || part.chars().any(|c| !c.is_ascii_digit()) {
                return Err(ConfigValidationError::ValidationFailed {
                    field: "app.version".to_string(),
                    message: "版本号的每个部分必须为数字".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl ConfigValidator for AppValidator {
    fn validate(&self, config: &Value) -> Result<(), ConfigValidationError> {
        // 验证应用配置部分存在
        let app = config.get("app")
            .ok_or_else(|| ConfigValidationError::RequiredFieldMissing {
                field: "app".to_string(),
            })?;

        // 验证应用名称
        if let Some(name_value) = app.get("name") {
            if let Some(name) = name_value.as_str() {
                self.validate_app_name(name)?;
            } else {
                return Err(ConfigValidationError::TypeMismatch {
                    field: "app.name".to_string(),
                    expected: "string".to_string(),
                    actual: "other".to_string(),
                });
            }
        } else {
            return Err(ConfigValidationError::RequiredFieldMissing {
                field: "app.name".to_string(),
            });
        }

        // 验证版本号
        if let Some(version_value) = app.get("version") {
            if let Some(version) = version_value.as_str() {
                self.validate_version(version)?;
            } else {
                return Err(ConfigValidationError::TypeMismatch {
                    field: "app.version".to_string(),
                    expected: "string".to_string(),
                    actual: "other".to_string(),
                });
            }
        } else {
            return Err(ConfigValidationError::RequiredFieldMissing {
                field: "app.version".to_string(),
            });
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 配置验证示例 ===\n");

    // 1. 创建有效的配置
    println!("1. 创建有效配置:");
    let valid_config = serde_json::json!({
        "app": {
            "name": "MyApp",
            "version": "1.0.0",
            "description": "A sample application"
        },
        "server": {
            "host": "localhost",
            "port": 8080,
            "debug": true,
            "workers": 4
        },
        "database": {
            "url": "postgresql://localhost:5432/mydb",
            "pool_size": 10,
            "max_connections": 20,
            "timeout_seconds": 30
        },
        "features": {
            "logging": true,
            "metrics": true,
            "tracing": false
        }
    });

    // 2. 创建无效的配置
    println!("\n2. 创建无效配置:");
    let invalid_config = serde_json::json!({
        "app": {
            "name": "",  // 空名称
            "version": "invalid-version"  // 无效版本号
        },
        "server": {
            "host": "",  // 空主机
            "port": 80,  // 系统保留端口
            "debug": "not-a-boolean"  // 类型错误
        },
        "database": {
            "url": "invalid-url",  // 无效URL
            "pool_size": 0,  // 无效池大小
            "timeout_seconds": 2  // 超时时间太短
        }
    });

    // 3. 创建基本验证器
    println!("\n3. 创建基本验证器:");
    
    let basic_validator = BasicConfigValidator::new("BasicValidator".to_string())
        .required_field("app.name".to_string(), "string".to_string())
        .required_field("app.version".to_string(), "string".to_string())
        .required_field("server.host".to_string(), "string".to_string())
        .required_field("server.port".to_string(), "number".to_string())
        .required_field("database.url".to_string(), "string".to_string())
        .required_field("database.pool_size".to_string(), "number".to_string())
        .custom_validator("server.port".to_string(), |value| {
            if let Some(port) = value.as_u64() {
                if port < 1024 {
                    return Err(ConfigValidationError::ValidationFailed {
                        field: "server.port".to_string(),
                        message: "端口号必须大于等于1024".to_string(),
                    });
                }
                if port > 65535 {
                    return Err(ConfigValidationError::ValidationFailed {
                        field: "server.port".to_string(),
                        message: "端口号必须小于等于65535".to_string(),
                    });
                }
            }
            Ok(())
        });

    // 4. 创建自定义验证器
    println!("\n4. 创建自定义验证器:");
    
    let database_validator = Box::new(DatabaseValidator::new());
    let server_validator = Box::new(ServerValidator::new());
    let app_validator = Box::new(AppValidator::new());

    println!("   ✓ 数据库验证器已创建");
    println!("   ✓ 服务器验证器已创建");
    println!("   ✓ 应用验证器已创建");

    // 5. 创建验证器注册表
    println!("\n5. 创建验证器注册表:");
    
    let validator_registry = ValidatorRegistry::new()
        .add_validator(Box::new(basic_validator))
        .add_validator(database_validator)
        .add_validator(server_validator)
        .add_validator(app_validator);

    println!("   ✓ 验证器注册表已创建");
    println!("   包含的验证器: {:?}", validator_registry.validator_names());

    // 6. 测试有效配置
    println!("\n6. 测试有效配置:");
    
    match validator_registry.validate(&valid_config) {
        Ok(_) => {
            println!("   ✓ 有效配置验证通过");
            
            // 显示配置内容
            if let Some(app) = valid_config.get("app") {
                println!("   应用名称: {}", app.get("name").unwrap());
                println!("   版本: {}", app.get("version").unwrap());
            }
            
            if let Some(server) = valid_config.get("server") {
                println!("   服务器: {}:{}", 
                    server.get("host").unwrap(), 
                    server.get("port").unwrap()
                );
            }
        }
        Err(e) => {
            println!("   ✗ 有效配置验证失败: {}", e);
        }
    }

    // 7. 测试无效配置
    println!("\n7. 测试无效配置:");
    
    match validator_registry.validate(&invalid_config) {
        Ok(_) => {
            println!("   ✗ 无效配置意外通过了验证");
        }
        Err(e) => {
            println!("   ✓ 无效配置被正确拒绝");
            println!("   错误信息: {}", e);
        }
    }

    // 8. 使用配置管理器集成验证
    println!("\n8. 使用配置管理器集成验证:");
    
    let manager = ConfigBuilder::new()
        .add_source(ConfigSource::Memory {
            data: valid_config.clone(),
        })
        .build();

    // 加载配置
    match manager.load().await {
        Ok(_) => {
            println!("   ✓ 配置加载成功");
            
            // 手动验证配置
            match validator_registry.validate(&valid_config) {
                Ok(_) => {
                    println!("   ✓ 配置验证通过");
                }
                Err(e) => {
                    println!("   ✗ 配置验证失败: {}", e);
                }
            }
        }
        Err(e) => {
            println!("   ✗ 配置加载失败: {}", e);
        }
    }

    // 9. 测试各种验证错误
    println!("\n9. 测试各种验证错误:");
    
    let test_cases = vec![
        // 缺失必需字段
        serde_json::json!({
            "app": {
                "name": "Test"
                // 缺少 version
            }
        }),
        // 类型不匹配
        serde_json::json!({
            "app": {
                "name": "Test",
                "version": 123  // 应该是字符串
            },
            "server": {
                "host": "localhost",
                "port": "8080"  // 应该是数字
            }
        }),
        // 无效值
        serde_json::json!({
            "app": {
                "name": "Test App",
                "version": "1.0.0"
            },
            "server": {
                "host": "localhost",
                "port": 80  // 系统保留端口
            },
            "database": {
                "url": "invalid-url",
                "pool_size": 0
            }
        }),
    ];

    for (i, test_config) in test_cases.iter().enumerate() {
        println!("   测试用例 {}: {:?}", i + 1, test_config);
        
        match validator_registry.validate(test_config) {
            Ok(_) => {
                println!("     意外通过了验证");
            }
            Err(e) => {
                println!("     正确拒绝: {}", e);
            }
        }
        println!();
    }

    // 10. 创建Schema验证器
    println!("10. 创建Schema验证器:");
    
    let schema = serde_json::json!({
        "type": "object",
        "required": ["app", "server"],
        "properties": {
            "app": {
                "type": "object",
                "required": ["name", "version"],
                "properties": {
                    "name": {"type": "string"},
                    "version": {"type": "string"}
                }
            },
            "server": {
                "type": "object",
                "required": ["host", "port"],
                "properties": {
                    "host": {"type": "string"},
                    "port": {"type": "number"}
                }
            }
        }
    });

    let schema_validator = SchemaValidator::new("SchemaValidator".to_string(), schema);

    // 测试Schema验证
    println!("   测试Schema验证:");
    match schema_validator.validate(&valid_config) {
        Ok(_) => println!("   ✓ Schema验证通过"),
        Err(e) => println!("   ✗ Schema验证失败: {}", e),
    }

    match schema_validator.validate(&invalid_config) {
        Ok(_) => println!("   ✗ 无效配置意外通过了Schema验证"),
        Err(e) => println!("   ✓ 无效配置被Schema验证拒绝: {}", e),
    }

    // 11. 创建部分有效的配置用于演示
    println!("\n11. 部分有效配置演示:");
    
    let partial_valid_config = serde_json::json!({
        "app": {
            "name": "Partial App",
            "version": "2.0.0"
        },
        "server": {
            "host": "localhost",
            "port": 9000
        }
        // 缺少数据库配置
    });

    // 只使用基本验证器（不检查数据库）
    let basic_only_registry = ValidatorRegistry::new()
        .add_validator(Box::new(BasicConfigValidator::new("BasicOnly".to_string())
            .required_field("app.name".to_string(), "string".to_string())
            .required_field("app.version".to_string(), "string".to_string())
            .required_field("server.host".to_string(), "string".to_string())
            .required_field("server.port".to_string(), "number".to_string())
        ));

    match basic_only_registry.validate(&partial_valid_config) {
        Ok(_) => println!("   ✓ 部分配置通过基本验证"),
        Err(e) => println!("   ✗ 部分配置验证失败: {}", e),
    }

    // 使用完整验证器
    match validator_registry.validate(&partial_valid_config) {
        Ok(_) => println!("   ✗ 部分配置意外通过了完整验证"),
        Err(e) => println!("   ✓ 部分配置被完整验证器正确拒绝: {}", e),
    }

    println!("\n=== 配置验证示例完成 ===");
    Ok(())
}