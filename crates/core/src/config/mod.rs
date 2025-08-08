//! # 配置管理系统
//!
//! 提供统一、类型安全的配置管理功能，支持多环境配置和动态更新。
//!
//! ## 概述
//!
//! 此模块实现了完整的配置管理系统，采用分层设计支持不同环境（开发、测试、生产）
//! 的配置需求，提供配置验证、默认值、环境变量覆盖等功能。
//!
//! ## 架构设计
//!
//! ```
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    配置管理系统                              │
//! ├─────────────────────────────────────────────────────────────┤
//! │  AppConfig  │  环境配置  │  验证器  │  加载器  │  工厂方法  │
//! │  (主配置)    │  (Profile)  │  (Validator) │ (Loader)  │  (Builder)  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## 核心功能
//!
//! ### 配置加载
//! - **文件加载**: 支持TOML格式配置文件
//! - **环境变量**: 支持环境变量覆盖配置
//! - **默认值**: 提供合理的默认配置值
//! - **环境配置**: 支持开发、测试、生产等不同环境
//!
//! ### 配置验证
//! - **类型验证**: 强类型配置结构
//! - **业务验证**: 自定义验证规则
//! - **依赖验证**: 配置项之间的依赖关系验证
//! - **范围验证**: 数值范围和枚举值验证
//!
//! ### 配置管理
//! - **热更新**: 支持配置的动态更新
//! - **配置合并**: 多配置源合并
//! - **配置导出**: 配置导出和备份
//! - **配置监控**: 配置变更监控和通知
//!
//! ## 配置结构
//!
//! ### 主配置 (AppConfig)
//! ```rust
//! pub struct AppConfig {
//!     pub database: DatabaseConfig,
//!     pub message_queue: MessageQueueConfig,
//!     pub api: ApiConfig,
//!     pub dispatcher: DispatcherConfig,
//!     pub worker: WorkerConfig,
//!     pub observability: ObservabilityConfig,
//!     pub profile: ConfigProfile,
//! }
//! ```
//!
//! ### 数据库配置 (DatabaseConfig)
//! ```rust
//! pub struct DatabaseConfig {
//!     pub url: String,
//!     pub pool_size: u32,
//!     pub connection_timeout_seconds: u64,
//!     pub max_lifetime_seconds: u64,
//!     pub idle_timeout_seconds: u64,
//! }
//! ```
//!
//! ### 消息队列配置 (MessageQueueConfig)
//! ```rust
//! pub enum MessageQueueConfig {
//!     RabbitMQ(RabbitMQConfig),
//!     RedisStream(RedisStreamConfig),
//! }
//! ```
//!
//! ## 使用示例
//!
//! ### 基本配置加载
//!
//! ```rust
//! use scheduler_core::config::{AppConfig, ConfigProfile};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 从配置文件加载
//!     let config = AppConfig::from_file("config/production.toml").await?;
//!
//!     // 设置环境配置
//!     let config = AppConfig::builder()
//!         .profile(ConfigProfile::Production)
//!         .database_url("postgresql://user:pass@localhost/scheduler")
//!         .api_port(8080)
//!         .build()?;
//!
//!     // 使用配置
//!     println!("Database URL: {}", config.database.url);
//!     println!("API Port: {}", config.api.port);
//!     println!("Message Queue: {:?}", config.message_queue);
//!
//!     Ok(())
//! }
//! ```
//!
//! ### 环境变量配置
//!
//! ```rust
//! use scheduler_core::config::AppConfig;
//!
//! // 设置环境变量
//! std::env::set_var("SCHEDULER_DATABASE_URL", "postgresql://localhost/scheduler");
//! std::env::set_var("SCHEDULER_API_PORT", "8080");
//!
//! // 加载配置（环境变量会覆盖配置文件）
//! let config = AppConfig::load_with_env("config/default.toml").await?;
//! ```
//!
//! ### 配置验证
//!
//! ```rust
//! use scheduler_core::config::{AppConfig, ConfigValidator};
//!
//! // 自定义验证器
//! struct CustomValidator;
//!
//! impl ConfigValidator for CustomValidator {
//!     fn validate(&self, config: &AppConfig) -> Result<(), ConfigValidationError> {
//!         // 验证数据库连接池大小
//!         if config.database.pool_size > 100 {
//!             return Err(ConfigValidationError::new(
//!                 "database.pool_size",
//!                 "连接池大小不能超过100"
//!             ));
//!         }
//!
//!         // 验证API端口范围
//!         if config.api.port > 65535 {
//!             return Err(ConfigValidationError::new(
//!                 "api.port",
//!                 "端口号必须在1-65535之间"
//!             ));
//!         }
//!
//!         Ok(())
//!     }
//! }
//!
//! // 使用验证器
//! let config = AppConfig::from_file("config/production.toml").await?;
//! CustomValidator.validate(&config)?;
//! ```
//!
//! ### 多环境配置
//!
//! ```rust
//! use scheduler_core::config::{AppConfig, ConfigProfile, Environment};
//!
//! // 根据环境变量自动选择配置
//! let profile = ConfigProfile::from_env()?;
//! let config = AppConfig::load_for_profile(&profile).await?;
//!
//! // 手动指定环境
//! let config = AppConfig::load_for_profile(&ConfigProfile::Development).await?;
//!
//! // 环境特定配置文件
//! let config = AppConfig::from_file(format!("config/{}.toml", profile.as_str())).await?;
//! ```
//!
//! ## 配置文件示例
//!
//! ### config/development.toml
//! ```toml
//! [database]
//! url = "postgresql://localhost:5432/scheduler_dev"
//! pool_size = 10
//! connection_timeout_seconds = 30
//!
//! [message_queue]
//! type = "rabbitmq"
//! host = "localhost"
//! port = 5672
//! username = "guest"
//! password = "guest"
//!
//! [api]
//! host = "127.0.0.1"
//! port = 8080
//! cors_origins = ["http://localhost:3000"]
//!
//! [dispatcher]
//! enabled = true
//! max_workers = 50
//! task_timeout_seconds = 3600
//!
//! [worker]
//! heartbeat_interval_seconds = 30
//! task_timeout_seconds = 1800
//! max_retries = 3
//!
//! [observability]
//! log_level = "debug"
//! metrics_enabled = true
//! tracing_enabled = true
//! ```
//!
//! ### config/production.toml
//! ```toml
//! [database]
//! url = "postgresql://prod-db:5432/scheduler"
//! pool_size = 50
//! connection_timeout_seconds = 60
//!
//! [message_queue]
//! type = "redis_stream"
//! redis_url = "redis://redis-cluster:6379"
//! stream_name = "scheduler_tasks"
//!
//! [api]
//! host = "0.0.0.0"
//! port = 8080
//! cors_origins = ["https://app.example.com"]
//!
//! [dispatcher]
//! enabled = true
//! max_workers = 200
//! task_timeout_seconds = 7200
//!
//! [worker]
//! heartbeat_interval_seconds = 15
//! task_timeout_seconds = 3600
//! max_retries = 5
//!
//! [observability]
//! log_level = "info"
//! metrics_enabled = true
//! tracing_enabled = true
//! ```
//!
//! ## 配置验证规则
//!
//! ### 数据库配置验证
//! - URL格式正确性
//! - 连接池大小范围 (1-100)
//! - 超时时间合理性
//!
//! ### API配置验证
//! - 端口号范围 (1-65535)
//! - 主机地址格式
//! - CORS源列表有效性
//!
//! ### 消息队列配置验证
//! - 连接参数完整性
//! - 队列类型有效性
//! - 认证信息安全性
//!
//! ## 配置热更新
//!
//! ```rust
//! use scheduler_core::config::AppConfig;
//! use tokio::signal;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut config = AppConfig::from_file("config/production.toml").await?;
//!
//!     // 监听配置文件变化
//!     let mut watcher = config.watch_file_changes("config/production.toml").await?;
//!
//!     tokio::spawn(async move {
//!         while let Some(()) = watcher.changed().await {
//!             println!("配置文件已更改，重新加载配置...");
//!             match AppConfig::from_file("config/production.toml").await {
//!                 Ok(new_config) => {
//!                     config = new_config;
//!                     println!("配置重新加载成功");
//!                 }
//!                 Err(e) => {
//!                     eprintln!("配置重新加载失败: {}", e);
//!                 }
//!             }
//!         }
//!     });
//!
//!     // 主程序逻辑
//!     signal::ctrl_c().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## 配置安全
//!
//! ### 敏感信息处理
//! ```rust
//! use scheduler_core::config::AppConfig;
//!
//! // 脱敏显示配置
//! fn display_config_safely(config: &AppConfig) {
//!     println!("Database: {}", config.database.url_masked());
//!     println!("Message Queue: {:?}", config.message_queue.type());
//!     println!("API Port: {}", config.api.port);
//!     // 不显示密码和敏感信息
//! }
//! ```
//!
//! ### 环境变量安全
//! - 使用环境变量传递敏感信息
//! - 避免在配置文件中存储密码
//! - 支持配置文件加密
//!
//! ## 性能优化
//!
//! ### 配置缓存
//! - 配置加载后缓存内存
//! - 避免重复文件读取
//! - 支持配置变更通知
//!
//! ### 延迟加载
//! - 大配置项按需加载
//! - 支持配置分片
//! - 减少启动时间
//!
//! ## 错误处理
//!
//! ```rust
//! use scheduler_core::config::{ConfigError, AppConfig};
//!
//! match AppConfig::from_file("config/production.toml").await {
//!     Ok(config) => {
//!         println!("配置加载成功");
//!         // 使用配置
//!     }
//!     Err(ConfigError::FileNotFound(path)) => {
//!         eprintln!("配置文件不存在: {}", path);
//!     }
//!     Err(ConfigError::ParseError(msg)) => {
//!         eprintln!("配置解析错误: {}", msg);
//!     }
//!     Err(ConfigError::ValidationError(errors)) => {
//!         for error in errors {
//!             eprintln!("配置验证错误: {}", error);
//!         }
//!     }
//!     Err(e) => {
//!         eprintln!("未知错误: {}", e);
//!     }
//! }
//! ```
//!
//! ## 测试支持
//!
//! ```rust
//! use scheduler_core::config::AppConfig;
//!
//! #[tokio::test]
//! async fn test_config_loading() {
//!     // 创建测试配置
//!     let config = AppConfig::builder()
//!         .database_url("postgresql://localhost/test")
//!         .api_port(8080)
//!         .build()
//!         .unwrap();
//!
//!     assert_eq!(config.api.port, 8080);
//!     assert!(config.database.url.contains("test"));
//! }
//!
//! #[tokio::test]
//! async fn test_config_validation() {
//!     let config = AppConfig::builder()
//!         .database_url("invalid-url")
//!         .build();
//!
//!     assert!(config.is_err());
//! }
//! ```
//!
//! ## 相关模块
//!
//! - [`crate::models`] - 数据模型定义
//! - [`crate::services`] - 服务实现
//! - [`crate::logging`] - 日志配置
//!
//! ## 扩展性
//!
//! ### 自定义配置源
//! ```rust
//! impl AppConfig {
//!     pub async fn from_consul(consul_url: &str, key: &str) -> Result<Self, ConfigError> {
//!         // 从Consul加载配置
//!         // ...
//!     }
//!
//!     pub async fn from_etcd(etcd_url: &str, key: &str) -> Result<Self, ConfigError> {
//!         // 从etcd加载配置
//!         // ...
//!     }
//! }
//! ```
//!
//! ### 自定义验证器
//! ```rust
//! pub struct SecurityValidator;
//!
//! impl ConfigValidator for SecurityValidator {
//!     fn validate(&self, config: &AppConfig) -> Result<(), ConfigValidationError> {
//!         // 安全相关的配置验证
//!         // ...
//!     }
//! }
//! ```

// 配置验证
pub mod validation;
// 环境特定配置
pub mod environment;
// 组织化的配置模型
pub mod models;
// 组织化的测试模块
#[cfg(test)]
pub mod tests;

// 重新导出关键类型
pub use environment::{ConfigProfile, Environment};
pub use models::{
    ApiConfig, AppConfig, DatabaseConfig, DispatcherConfig, MessageQueueConfig, MessageQueueType,
    ObservabilityConfig, RedisConfig, WorkerConfig,
};
pub use validation::{BasicConfigValidator, ConfigValidationError, ConfigValidator};

// 通用验证器（为了方便）
pub mod validators {
    pub use super::validation::*;
}