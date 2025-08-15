//! # Scheduler Foundation
//!
//! The scheduler-foundation crate provides the foundational components and core abstractions
//! for the distributed task scheduler system. This crate focuses on providing essential
//! building blocks that are used across the entire scheduler ecosystem.
//!
//! ## Core Responsibilities
//!
//! - **Service Container & Dependency Injection**: Provides `ApplicationContext` and `ServiceLocator`
//!   for managing service dependencies and lifecycle
//! - **Core Service Interfaces**: Defines essential traits for task control, scheduling, dispatching,
//!   and worker management (backward compatibility)
//! - **Task Execution Framework**: Contains `TaskExecutor` trait and `ExecutorRegistry` for
//!   managing and executing different types of tasks
//! - **Message Queue Abstraction**: Provides `MessageQueue` trait for asynchronous communication
//! - **Core Data Models**: Contains essential data structures like `TaskStatusUpdate` and
//!   various statistics structures
//! - **Mock Implementations**: Provides mock implementations for testing purposes
//!
//! ## Architecture
//!
//! This crate serves as the foundation layer, providing:
//! - Essential interfaces that higher-level crates depend on
//! - Common data structures used throughout the system
//! - Dependency injection infrastructure
//! - Testing utilities and mock implementations
//!
//! ## Usage
//!
//! ```rust
//! use scheduler_foundation::prelude::*;
//! use scheduler_foundation::{ServiceLocator, SchedulerResult, TaskControlService};
//!
//! // Create service locator and access core services
//! let service_locator = Arc::new(ServiceLocator::new(app_context));
//! let task_controller = service_locator.task_controller().await?;
//! ```

pub mod container;
pub mod executor_registry;
pub mod models;
pub mod traits;

// Re-export essential types for convenience
pub use container::ServiceLocator;
pub use models::TaskStatusUpdate;
pub use scheduler_domain::entities::{
    Message, Task, TaskFilter, TaskResult, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus,
};
pub use scheduler_errors::{SchedulerError, SchedulerResult};
pub use traits::*;

// Prelude module for common imports
pub mod prelude {
    pub use crate::container::*;
    pub use crate::executor_registry::*;
}
