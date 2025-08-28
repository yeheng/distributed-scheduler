// 验证模块
// 提供配置验证功能

pub mod basic;

pub use basic::{
    BasicConfigValidator, ConfigValidator, ValidationError, ValidationResult, ValidationSeverity,
    ValidationUtils, ValidationWarning,
};
