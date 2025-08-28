//! # scheduler-core
//!
//! 分布式任务调度系统的核心模块
//!
//! 本模块提供：
//! - 依赖注入框架
//! - 系统常量和类型定义
//! - 核心trait和接口
//! - 通用工具函数

pub mod di;

// Re-export the DI types for external crates
pub use di::{ApplicationContext, ServiceContainer, ServiceLocator};

// Re-export common functionality
pub use scheduler_common::*;

// Re-export domain types
pub use scheduler_domain::{SchedulerError, SchedulerResult};

/// 核心模块版本信息
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 获取核心模块的版本信息
pub fn get_core_version() -> scheduler_common::types::VersionInfo {
    scheduler_common::types::VersionInfo {
        version: CORE_VERSION.to_string(),
        build_time: chrono::Utc::now().to_rfc3339(),
        git_hash: option_env!("GIT_HASH").map(|s| s.to_string()),
        build_env: scheduler_common::env::detect_environment(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_version() {
        let version = get_core_version();
        assert!(!version.version.is_empty());
        assert!(!version.build_time.is_empty());
    }

    #[test]
    fn test_re_exports() {
        // 测试是否能正确导入通用功能
        let _now = time::now_utc();
        let _uuid = string::generate_uuid();
        assert!(SYSTEM_NAME.len() > 0);
    }
}
