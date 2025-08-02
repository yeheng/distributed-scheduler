//! 服务层抽象模块
//!
//! 按照接口隔离原则（ISP）将原本的大型服务接口拆分为多个小型、专门的接口
//! 每个模块专注于特定的业务领域

pub mod config_services;
pub mod monitoring_services;
pub mod task_services;
pub mod worker_services;

// 重新导出所有接口
pub use config_services::*;
pub use monitoring_services::*;
pub use task_services::*;
pub use worker_services::*;

// 通用时间范围类型
#[derive(Debug, Clone)]
pub struct TimeRange {
    /// 开始时间
    pub start: chrono::DateTime<chrono::Utc>,
    /// 结束时间
    pub end: chrono::DateTime<chrono::Utc>,
}
