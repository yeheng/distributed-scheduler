//! # scheduler-common
//! 
//! 分布式任务调度系统的共享工具和常量模块
//! 
//! 本模块提供：
//! - 系统常量定义
//! - 通用工具函数
//! - 共享类型定义
//! - 通用trait定义

pub mod constants;
pub mod utils;
pub mod types;
pub mod traits;

// Re-export commonly used items
pub use constants::*;
pub use types::*;
pub use traits::*;
pub use utils::*;

// Re-export error types
pub use scheduler_errors::{SchedulerError, SchedulerResult};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Test that all modules are properly exported
        assert!(SYSTEM_NAME.len() > 0);
        assert!(DEFAULT_TIMEOUT_SECONDS > 0);
    }
}