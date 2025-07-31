//! 统一配置管理系统
//!
//! 这是调度器核心库的配置管理模块，提供了全面、类型安全、高性能的配置管理解决方案。
//! 该模块整合了多个实现的最佳特性，支持多种配置源、验证、热重载和类型安全的配置访问。
//!
//! # 模块架构
//!
//! ## 核心模块
//!
//! - **core**: 核心配置抽象和基础组件
//! - **manager**: 统一配置管理器实现
//! - **typesafe**: 类型安全的配置包装器
//! - **validation**: 配置验证框架
//!
//! ## 功能模块
//!
//! - **loader**: 文件配置加载器
//! - **environment**: 环境特定配置
//! - **hot_reload**: 热重载功能
//! - **models**: 配置数据模型
//! - **services**: 配置服务实现
//!
//! ## 兼容性模块
//!
//! - **config_loader**: 传统配置加载器（向后兼容）
//! - **builder**: 构建器模式实现（向后兼容）
//!
//! # 核心特性
//!
//! - **多配置源支持**: 文件、环境变量、命令行参数、远程配置、内存配置
//! - **类型安全**: 编译时类型检查，避免运行时类型错误
//! - **配置验证**: 内置验证框架，确保配置正确性
//! - **热重载**: 支持配置的热重载，无需重启应用
//! - **高性能**: 内置缓存和并发访问支持
//! - **环境配置**: 支持不同环境的特定配置
//! - **构建器模式**: 流式配置构建接口
//! - **回调机制**: 配置变更通知机制
//!
//! # 设计原则
//!
//! 遵循 KISS (Keep It Simple, Stupid) 和 SOLID 设计原则：
//! - **单一职责**: 每个模块都有明确的职责
//! - **开闭原则**: 通过特征和抽象支持扩展
//! - **里氏替换**: 所有兼容实现都可以互换使用
//! - **接口隔离**: 提供最小化的必要接口
//! - **依赖倒置**: 依赖抽象而非具体实现
//!
//! # 使用示例
//!
//! ## 基本使用
//!
//! ```rust,no_run
//! use scheduler_core::config::manager::{ConfigBuilder, ConfigSource};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 创建配置管理器
//!     let manager = ConfigBuilder::new()
//!         .add_source(ConfigSource::Memory {
//!             data: json!({
//!                 "server": {
//!                     "host": "localhost",
//!                     "port": 8080
//!                 },
//!                 "database": {
//!                     "url": "postgresql://localhost/mydb"
//!                 }
//!             }),
//!         })
//!         .build();
//!     
//!     // 加载配置
//!     manager.load().await?;
//!     
//!     // 获取配置值
//!     let host: String = manager.get("server.host").await?;
//!     let port: u16 = manager.get("server.port").await?;
//!     
//!     println!("Server running on {}:{}", host, port);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## 类型安全配置
//!
//! ```rust,no_run
//! use scheduler_core::config::typesafe::{TypedConfig, ConfigBuilder};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Serialize, Deserialize)]
//! struct DatabaseConfig {
//!     url: String,
//!     pool_size: u32,
//!     timeout_seconds: u64,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let manager = ConfigBuilder::new()
//!         .add_source(ConfigSource::Memory {
//!             data: json!({
//!                 "database": {
//!                     "url": "postgresql://localhost/mydb",
//!                     "pool_size": 10,
//!                     "timeout_seconds": 30
//!                 }
//!             }),
//!         })
//!         .build();
//!     
//!     manager.load().await?;
//!     
//!     // 创建类型安全的配置包装器
//!     let db_config = TypedConfig::<DatabaseConfig>::new(
//!         std::sync::Arc::new(manager),
//!         "database".to_string(),
//!     );
//!     
//!     // 获取类型安全的配置
//!     let config = db_config.get().await?;
//!     if let Some(db) = config {
//!         println!("Database URL: {}", db.url);
//!         println!("Pool size: {}", db.pool_size);
//!         println!("Timeout: {}s", db.timeout_seconds);
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## 多配置源合并
//!
//! ```rust,no_run
//! use scheduler_core::config::manager::{ConfigBuilder, ConfigSource};
//! use std::path::PathBuf;
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let manager = ConfigBuilder::new()
//!         // 默认配置（最低优先级）
//!         .add_source(ConfigSource::Memory {
//!             data: json!({
//!                 "server": {
//!                     "host": "localhost",
//!                     "port": 8080,
//!                     "debug": false
//!                 }
//!             }),
//!         })
//!         // 文件配置（中等优先级）
//!         .add_source(ConfigSource::File {
//!             path: PathBuf::from("config.json"),
//!         })
//!         // 环境变量配置（最高优先级）
//!         .add_source(ConfigSource::Environment {
//!             prefix: "APP_".to_string(),
//!         })
//!         .build();
//!     
//!     manager.load().await?;
//!     
//!     // 配置会按照优先级合并
//!     let host: String = manager.get("server.host").await?;
//!     let port: u16 = manager.get("server.port").await?;
//!     let debug: bool = manager.get("server.debug").await?;
//!     
//!     println!("Final config - {}:{}", host, port);
//!     println!("Debug mode: {}", debug);
//!     
//!     Ok(())
//! }
//! ```

// 核心配置抽象
pub mod core;
// 配置验证
pub mod validation;
// 基于文件的配置加载
pub mod loader;
// 环境特定配置
pub mod environment;
// 热重载功能
pub mod hot_reload;
// 统一配置管理器
pub mod manager;
// 类型安全的配置包装器
pub mod typesafe;
// 配置构建器和实用工具
// 组织化的配置模型
pub mod models;
// 服务实现
pub mod services;
// 组织化的测试模块
#[cfg(test)]
pub mod tests;

// 向后兼容的传统模块
pub mod config_loader;
pub mod builder;

// 重新导出关键类型和特征以方便导入
pub use core::{ConfigurationService, TypedConfig, ConfigValue, ConfigSource};
pub use validation::{ConfigValidator, ConfigValidationError, BasicConfigValidator};
pub use loader::{ConfigLoader as FileConfigLoaderTrait, FileConfigLoader, ConfigSourceType, MultiSourceLoader, MergeStrategy};
pub use environment::{Environment, ConfigProfile};
pub use hot_reload::{HotReloadManager, ConfigChangeEvent};
pub use manager::{UnifiedConfigManager, ConfigBuilder as UnifiedConfigBuilder, ReloadStrategy, ConfigMetadata};
pub use typesafe::{TypedConfig as TypeSafeConfig, ConfigBuilder, ConfigProfile as TypeSafeProfile};
pub use models::{AppConfig, DatabaseConfig, MessageQueueConfig, MessageQueueType, RedisConfig, DispatcherConfig, WorkerConfig, ApiConfig, ObservabilityConfig};
pub use services::{ConfigurationService as ServiceTrait, ConfigCache};
pub use builder::{InMemoryConfigService, ConfigBuilder as LegacyConfigBuilder, ConfigManager};

// 传统导出
pub use config_loader::ConfigLoader as LegacyConfigLoader;

// 通用验证器（为了方便）
pub mod validators {
    pub use super::validation::*;
}

#[cfg(test)]
mod config_test;