
pub mod config_services;
pub mod monitoring_services;
pub mod task_services;
pub mod worker_services;
pub use config_services::*;
pub use monitoring_services::*;
pub use task_services::*;
pub use worker_services::*;
#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: chrono::DateTime<chrono::Utc>,
}
