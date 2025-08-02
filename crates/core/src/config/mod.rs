//! 统一配置管理系统
//!
//! 提供简单、实用的配置管理功能，遵循KISS原则。
//!
//! # 核心特性
//!
//! - **简单配置加载**: 支持TOML文件和环境变量
//! - **类型安全**: 使用强类型配置结构
//! - **配置验证**: 内置配置验证
//! - **默认值支持**: 合理的默认配置
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use scheduler_core::config::AppConfig;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 加载配置
//!     let config = AppConfig::load(Some("config/scheduler.toml"))?;
//!     
//!     println!("Database URL: {}", config.database.url);
//!     println!("API bind address: {}", config.api.bind_address);
//!     
//!     Ok(())
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

#[cfg(test)]
mod config_test;
