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

// Core types and utilities
pub use models::{Message, Task, TaskResult, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
pub use errors::SchedulerError;
pub use container::ServiceLocator;
pub type SchedulerResult<T> = std::result::Result<T, SchedulerError>;

// Core modules with focused re-exports
pub mod prelude {
    pub use crate::circuit_breaker::*;
    pub use crate::config::*;
    pub use crate::container::*;
    pub use crate::error_handling::*;
    pub use crate::executor_registry::*;
    pub use crate::logging::*;
}

// Service interfaces organized by domain  
pub mod api {
    pub use crate::services::{
        TaskControlService, WorkerHealthService, AlertManagementService,
        ConfigurationService, HealthCheckService, PerformanceMonitoringService,
    };
}

// Core traits for extensibility
pub mod ext {
    pub use crate::traits::{
        ExecutorRegistry, ExecutorStatus, MessageQueue, ResourceLimits, 
        TaskExecutionContextTrait, TaskExecutor, WorkerServiceTrait,
    };
}

// Backward compatibility re-exports
pub use crate::ext::*;
