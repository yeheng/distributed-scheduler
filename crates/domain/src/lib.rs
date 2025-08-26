pub mod entities;
pub mod events;
pub mod ports;
pub mod repositories;
pub mod services;
pub mod task_dependency_service;
pub mod task_query_builder;
pub mod value_objects;

// SQLx 实现（仅在启用 sqlx-support feature 时编译）
#[cfg(feature = "sqlx-support")]
pub mod sqlx_impls;

pub use entities::*;
pub use events::*;
pub use ports::*;
pub use repositories::*;
pub use scheduler_errors::{SchedulerError, SchedulerResult};
pub use services::*;
pub use value_objects::*;
