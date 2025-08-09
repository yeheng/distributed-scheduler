
pub mod circuit_breaker;
pub mod config;
pub mod container;
pub mod error_handling;
pub mod errors;
pub mod executor_registry;
pub mod logging;
pub mod models;
pub mod service_interfaces;
pub mod services;
pub mod traits;
pub use models::{Message, Task, TaskResult, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
pub use errors::SchedulerError;
pub use container::ServiceLocator;
pub type SchedulerResult<T> = std::result::Result<T, SchedulerError>;
pub mod prelude {
    pub use crate::circuit_breaker::*;
    pub use crate::config::*;
    pub use crate::container::*;
    pub use crate::error_handling::*;
    pub use crate::executor_registry::*;
    pub use crate::logging::*;
}
pub mod api {
    pub use crate::services::{
        TaskControlService, WorkerHealthService, AlertManagementService,
        ConfigurationService, HealthCheckService, PerformanceMonitoringService,
    };
}
pub mod ext {
    pub use crate::traits::{
        ExecutorRegistry, ExecutorStatus, MessageQueue, ResourceLimits, 
        TaskExecutionContextTrait, TaskExecutor, WorkerServiceTrait,
    };
}
pub use crate::ext::*;
