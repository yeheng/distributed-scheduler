//! 类型安全的配置管理
//!
//! 此模块提供了类型安全的配置包装器和实用工具，遵循 KISS 和 SOLID 设计原则。
//! 它确保配置值的类型安全性，并提供编译时的类型检查。
//!
//! # 核心特性
//!
//! - **类型安全**: 编译时确保配置值的类型正确性
//! - **零成本抽象**: 运行时没有额外的性能开销
//! - **异步支持**: 所有操作都支持异步执行
//! - **错误处理**: 提供详细的错误信息和上下文
//! - **线程安全**: 支持多线程环境下的并发访问
//! - **构建器模式**: 提供流式配置构建接口
//! - **环境配置**: 支持不同环境的特定配置
//!
//! # 设计原则
//!
//! 遵循 KISS (Keep It Simple, Stupid) 和 SOLID 设计原则：
//! - **单一职责**: 每个组件都有明确的职责
//! - **开闭原则**: 通过泛型和特征支持扩展
//! - **里氏替换**: 所有类型安全的配置都可以互换使用
//! - **接口隔离**: 提供最小化的必要接口
//! - **依赖倒置**: 依赖抽象而非具体实现
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use scheduler_core::config::typesafe::{ConfigBuilder, TypedConfig};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Serialize, Deserialize)]
//! struct DatabaseConfig {
//!     url: String,
//!     pool_size: u32,
//!     timeout_seconds: u64,
//! }
//!
//! #[derive(Debug, Serialize, Deserialize)]
//! struct ServerConfig {
//!     host: String,
//!     port: u16,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 使用构建器创建配置管理器
//!     let manager = ConfigBuilder::new()
//!         .typed::<DatabaseConfig>("database")
//!         .typed::<ServerConfig>("server")
//!         .build();
//!     
//!     // 创建类型安全的配置包装器
//!     let db_config = TypedConfig::<DatabaseConfig>::new(
//!         manager.clone(), 
//!         "database".to_string()
//!     );
//!     
//!     // 获取配置值
//!     let config = db_config.get().await?;
//!     if let Some(db) = config {
//!         println!("Database URL: {}", db.url);
//!         println!("Pool size: {}", db.pool_size);
//!     }
//!     
//!     // 设置配置值
//!     let new_config = DatabaseConfig {
//!         url: "postgresql://localhost/newdb".to_string(),
//!         pool_size: 20,
//!         timeout_seconds: 30,
//!     };
//!     db_config.set(&new_config).await?;
//!     
//!     Ok(())
//! }
//! ```

use std::marker::PhantomData;
use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::{errors::SchedulerError, SchedulerResult};

use super::manager::{UnifiedConfigManager, ConfigSource, ConfigValidator, ReloadStrategy};

/// 类型安全的配置包装器
///
/// 提供编译时类型安全的配置访问接口，确保配置值的类型正确性。
/// 使用泛型和 PhantomData 来确保类型安全，避免运行时类型错误。
///
/// # 类型参数
///
/// - `T`: 配置值的类型，必须支持序列化和反序列化
///
/// # 特性
///
/// - **类型安全**: 编译时确保配置值的类型正确性
/// - **零成本抽象**: 运行时没有额外的性能开销
/// - **异步支持**: 所有操作都支持异步执行
/// - **错误处理**: 提供详细的错误信息和上下文
/// - **线程安全**: 支持多线程环境下的并发访问
///
/// # 使用场景
///
/// - 数据库配置的类型安全管理
/// - 服务器配置的类型安全访问
/// - 应用配置的类型安全封装
/// - 第三方服务配置的类型安全处理
///
/// # 示例
///
/// ```rust,no_run
/// use scheduler_core::config::typesafe::TypedConfig;
/// use serde::{Deserialize, Serialize};
/// use std::sync::Arc;
/// 
/// #[derive(Debug, Serialize, Deserialize)]
/// struct DatabaseConfig {
///     url: String,
///     pool_size: u32,
/// }
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let manager = Arc::new(create_config_manager());
///     
///     // 创建类型安全的配置包装器
///     let db_config = TypedConfig::<DatabaseConfig>::new(
///         manager.clone(), 
///         "database".to_string()
///     );
///     
///     // 获取配置值
///     let config = db_config.get().await?;
///     if let Some(db) = config {
///         println!("Database URL: {}", db.url);
///         println!("Pool size: {}", db.pool_size);
///     }
///     
///     // 设置配置值
///     let new_config = DatabaseConfig {
///         url: "postgresql://localhost/newdb".to_string(),
///         pool_size: 20,
///     };
///     db_config.set(&new_config).await?;
///     
///     // 检查配置是否存在
///     if db_config.exists().await? {
///         println!("Database configuration exists");
///     }
///     
///     Ok(())
/// }
/// ```
pub struct TypedConfig<T> {
    /// 配置管理器实例
    manager: Arc<UnifiedConfigManager>,
    /// 配置键名
    key: String,
    /// 类型标记 PhantomData
    _phantom: PhantomData<T>,
}

impl<T> TypedConfig<T>
where
    T: Serialize + DeserializeOwned + Send + Sync,
{
    /// Create new typed configuration wrapper
    pub fn new(manager: Arc<UnifiedConfigManager>, key: String) -> Self {
        Self {
            manager,
            key,
            _phantom: PhantomData,
        }
    }

    /// Get configuration value with type safety
    pub async fn get(&self) -> SchedulerResult<Option<T>> {
        match self.manager.get(&self.key).await {
            Ok(value) => Ok(Some(value)),
            Err(SchedulerError::Configuration(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get configuration value with default
    pub async fn get_or_default(&self, default: T) -> SchedulerResult<T> {
        match self.get().await {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Ok(default),
            Err(e) => Err(e),
        }
    }

    /// Set configuration value with type safety
    pub async fn set(&self, value: &T) -> SchedulerResult<()> {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!(
                "Failed to serialize config '{}': {}",
                self.key, e
            ))
        })?;
        self.manager.set(&self.key, json_value).await
    }

    /// Get configuration key
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Check if configuration value exists
    pub async fn exists(&self) -> SchedulerResult<bool> {
        match self.manager.get::<Value>(&self.key).await {
            Ok(_) => Ok(true),
            Err(SchedulerError::Configuration(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Delete configuration value
    pub async fn delete(&self) -> SchedulerResult<bool> {
        // Set to null to delete
        self.manager.set(&self.key, Value::Null).await?;
        Ok(true)
    }
}

/// Configuration service trait for abstraction
#[async_trait::async_trait]
pub trait ConfigurationService: Send + Sync {
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<Value>>;
    async fn set_config_value(&self, key: &str, value: &Value) -> SchedulerResult<()>;
    async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;
    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;
    async fn reload_config(&self) -> SchedulerResult<()>;
}

/// Implementation of ConfigurationService for UnifiedConfigManager
#[async_trait::async_trait]
impl ConfigurationService for UnifiedConfigManager {
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<Value>> {
        match self.get::<Value>(key).await {
            Ok(value) => Ok(Some(value)),
            Err(SchedulerError::Configuration(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn set_config_value(&self, key: &str, value: &Value) -> SchedulerResult<()> {
        self.set(key, value.clone()).await
    }

    async fn delete_config(&self, key: &str) -> SchedulerResult<bool> {
        match self.set(key, Value::Null).await {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>> {
        // Get all keys from the configuration
        let config_json = self.to_json().await?;
        let config: Value = serde_json::from_str(&config_json).map_err(|e| {
            SchedulerError::Configuration(format!("Failed to parse config JSON: {}", e))
        })?;
        if let Value::Object(map) = config {
            Ok(map.keys().cloned().collect())
        } else {
            Ok(Vec::new())
        }
    }

    async fn reload_config(&self) -> SchedulerResult<()> {
        self.reload().await
    }
}

/// Configuration builder - Fluent interface for configuration management
pub struct ConfigBuilder {
    manager: UnifiedConfigManager,
}

impl ConfigBuilder {
    /// Create new configuration builder
    pub fn new() -> Self {
        Self {
            manager: UnifiedConfigManager::new(),
        }
    }

    /// Add configuration source
    pub fn add_source(mut self, source: ConfigSource) -> Self {
        self.manager = self.manager.add_source(source);
        self
    }

    /// Add configuration validator
    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.manager = self.manager.add_validator(validator);
        self
    }

    /// Set reload strategy
    pub fn with_reload_strategy(mut self, strategy: ReloadStrategy) -> Self {
        self.manager = self.manager.with_reload_strategy(strategy);
        self
    }

    /// Add configuration change callback
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&Value) + Send + Sync + 'static,
    {
        self.manager = self.manager.add_callback(callback);
        self
    }

    /// Build the configuration manager
    pub fn build(self) -> Arc<UnifiedConfigManager> {
        Arc::new(self.manager)
    }

    /// Create typed configuration for a specific key
    pub fn typed<T>(self, key: &str) -> TypedConfig<T>
    where
        T: Serialize + DeserializeOwned + Send + Sync,
    {
        let manager = Arc::new(self.manager);
        TypedConfig::new(manager, key.to_string())
    }

    /// Get configuration value
    pub async fn get<T>(&self, key: &str) -> SchedulerResult<Option<T>>
    where
        T: DeserializeOwned + Send + Sync,
    {
        self.manager.get(key).await
    }

    /// Get configuration value with default
    pub async fn get_or_default<T>(&self, key: &str, default: T) -> SchedulerResult<T>
    where
        T: DeserializeOwned + Send + Sync + Clone,
    {
        Ok(self.manager.get_or_default(key, default).await)
    }

    /// Set configuration value
    pub async fn set<T>(&self, key: &str, value: &T) -> SchedulerResult<()>
    where
        T: Serialize + Send + Sync,
    {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!("Failed to serialize config '{key}': {e}"))
        })?;
        self.manager.set(key, json_value).await
    }

    /// Delete configuration
    pub async fn delete(&self, key: &str) -> SchedulerResult<bool> {
        self.manager.set(key, Value::Null).await.map(|_| true)
    }

    /// List all configuration keys
    pub async fn list_keys(&self) -> SchedulerResult<Vec<String>> {
        let config_json = self.manager.to_json().await?;
        let config: Value = serde_json::from_str(&config_json).map_err(|e| {
            SchedulerError::Configuration(format!("Failed to parse config JSON: {}", e))
        })?;
        if let Value::Object(map) = config {
            Ok(map.keys().cloned().collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Reload configuration
    pub async fn reload(&self) -> SchedulerResult<()> {
        self.manager.reload().await
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Environment-specific configuration
#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

impl Environment {
    /// Parse environment from string
    pub fn from_str(env: &str) -> SchedulerResult<Self> {
        match env.to_lowercase().as_str() {
            "development" | "dev" => Ok(Environment::Development),
            "testing" | "test" => Ok(Environment::Testing),
            "staging" | "stage" => Ok(Environment::Staging),
            "production" | "prod" => Ok(Environment::Production),
            _ => Err(SchedulerError::Configuration(format!(
                "Invalid environment: {env}"
            ))),
        }
    }

    /// Get current environment from environment variable
    pub fn current() -> SchedulerResult<Self> {
        std::env::var("APP_ENV")
            .map(|s| Self::from_str(&s))
            .unwrap_or(Ok(Environment::Development))
    }

    /// Get environment as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Testing => "testing",
            Environment::Staging => "staging",
            Environment::Production => "production",
        }
    }

    /// Check if environment is production
    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }

    /// Check if environment is development
    pub fn is_development(&self) -> bool {
        matches!(self, Environment::Development)
    }
}

/// Configuration profile for different environments
#[derive(Debug, Clone)]
pub struct ConfigProfile {
    /// Environment for this profile
    pub environment: Environment,
    /// Configuration overrides for this environment
    pub overrides: Value,
    /// Feature flags for this environment
    pub features: std::collections::HashMap<String, bool>,
}

impl ConfigProfile {
    /// Create new configuration profile
    pub fn new(environment: Environment) -> Self {
        Self {
            environment,
            overrides: Value::Object(serde_json::Map::new()),
            features: std::collections::HashMap::new(),
        }
    }

    /// Add configuration override
    pub fn with_override(mut self, key: &str, value: Value) -> Self {
        if let Value::Object(ref mut map) = self.overrides {
            map.insert(key.to_string(), value);
        }
        self
    }

    /// Add feature flag
    pub fn with_feature(mut self, name: &str, enabled: bool) -> Self {
        self.features.insert(name.to_string(), enabled);
        self
    }

    /// Check if feature is enabled
    pub fn is_feature_enabled(&self, name: &str) -> bool {
        self.features.get(name).copied().unwrap_or(false)
    }
}

impl Default for ConfigProfile {
    fn default() -> Self {
        Self::new(Environment::Development)
    }
}

/// Configuration utilities
pub mod utils {
    use super::*;

    /// Create a typed configuration from a manager and key
    pub fn typed<T>(manager: Arc<UnifiedConfigManager>, key: &str) -> TypedConfig<T>
    where
        T: Serialize + DeserializeOwned + Send + Sync,
    {
        TypedConfig::new(manager, key.to_string())
    }

    /// Get environment variable with fallback to configuration
    pub async fn env_or_config<T>(
        manager: &UnifiedConfigManager,
        env_var: &str,
        config_key: &str,
        default: T,
    ) -> SchedulerResult<T>
    where
        T: DeserializeOwned + Clone + Send + Sync,
    {
        if let Ok(env_value) = std::env::var(env_var) {
            // Try to parse environment variable as JSON, fall back to string
            let parsed_value: Value = serde_json::from_str(&env_value)
                .unwrap_or_else(|_| Value::String(env_value));
            
            T::deserialize(&parsed_value).map_err(|e| {
                SchedulerError::Configuration(format!(
                    "Failed to deserialize environment variable '{env_var}': {e}"
                ))
            })
        } else {
            Ok(manager.get_or_default(config_key, default).await)
        }
    }

    /// Merge multiple configuration sources with priority
    pub async fn merge_configs(
        configs: &[serde_json::Value],
    ) -> SchedulerResult<serde_json::Value> {
        let mut result = Value::Object(serde_json::Map::new());
        
        for config in configs {
            result = merge_two_configs(result, config.clone());
        }
        
        Ok(result)
    }

    /// Helper function to merge two configurations
    fn merge_two_configs(base: Value, override_config: Value) -> Value {
        match (base, override_config) {
            (Value::Object(mut base_map), Value::Object(override_map)) => {
                for (key, value) in override_map {
                    if let (Some(base_value), Value::Object(override_obj)) =
                        (base_map.get(&key), &value)
                    {
                        if let Value::Object(base_obj) = base_value {
                            let merged = merge_two_configs(
                                Value::Object(base_obj.clone()),
                                Value::Object(override_obj.clone()),
                            );
                            base_map.insert(key, merged);
                            continue;
                        }
                    }
                    base_map.insert(key, value);
                }
                Value::Object(base_map)
            }
            (_, override_config) => override_config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_typed_config() {
        let manager = Arc::new(UnifiedConfigManager::new().add_source(ConfigSource::Memory {
            data: serde_json::json!({"test_key": "test_value"}),
        }));

        manager.load().await.unwrap();

        let typed_config = TypedConfig::<String>::new(manager, "test_key".to_string());

        // Test get
        let result = typed_config.get().await.unwrap();
        assert_eq!(result, Some("test_value".to_string()));

        // Test exists
        assert!(typed_config.exists().await.unwrap());

        // Test set
        typed_config.set(&"new_value".to_string()).await.unwrap();
        let result = typed_config.get().await.unwrap();
        assert_eq!(result, Some("new_value".to_string()));
    }

    #[tokio::test]
    async fn test_config_builder_fluent() {
        let builder = ConfigBuilder::new()
            .add_source(ConfigSource::Memory {
                data: serde_json::json!({"database": {"url": "postgresql://localhost/test"}}),
            });

        let manager = builder.build();
        manager.load().await.unwrap();

        // Test typed configuration
        let typed_db = TypedConfig::<serde_json::Value>::new(manager.clone(), "database.url".to_string());
        let result = typed_db.get().await.unwrap();
        
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_environment_handling() {
        // Test environment parsing
        let env = Environment::from_str("production").unwrap();
        assert_eq!(env, Environment::Production);
        assert!(env.is_production());

        // Test current environment
        std::env::set_var("APP_ENV", "development");
        let current = Environment::current().unwrap();
        assert_eq!(current, Environment::Development);
        assert!(current.is_development());

        // Cleanup
        std::env::remove_var("APP_ENV");
    }

    #[tokio::test]
    async fn test_config_profile() {
        let profile = ConfigProfile::new(Environment::Production)
            .with_override("server.port", serde_json::json!(8080))
            .with_feature("logging", true);

        assert_eq!(profile.environment, Environment::Production);
        assert!(profile.is_feature_enabled("logging"));
        assert!(!profile.is_feature_enabled("nonexistent"));
    }

    #[tokio::test]
    async fn test_env_or_config() {
        let manager = UnifiedConfigManager::new()
            .add_source(ConfigSource::Memory {
                data: serde_json::json!({"fallback": "config_value"}),
            });

        manager.load().await.unwrap();

        // Test with environment variable
        std::env::set_var("TEST_VAR", "env_value");
        let result: String = utils::env_or_config(&manager, "TEST_VAR", "fallback", "default".to_string()).await.unwrap();
        assert_eq!(result, "env_value");

        // Test without environment variable (fallback to config)
        std::env::remove_var("TEST_VAR");
        let result: String = utils::env_or_config(&manager, "TEST_VAR", "fallback", "default".to_string()).await.unwrap();
        assert_eq!(result, "config_value");
    }
}