//! Worker Composition Root
//! 
//! This crate serves as the composition root for the worker service,
//! coordinating the assembly and initialization of all worker components.
//! The actual business logic has been moved to the application layer.

pub mod components;
pub mod executor_factory;
pub mod executors;
pub mod service;

#[cfg(test)]
mod service_test;

#[cfg(test)]
mod executors_test;

// Re-export the main service types for composition root usage
pub use executor_factory::ExecutorFactory;
pub use executors::{HttpExecutor, HttpTaskParams, ShellExecutor, ShellTaskParams};
pub use service::{WorkerConfig, WorkerServiceBuilder, WorkerServiceImpl};

// Re-export components that are needed for composition
pub use components::{HeartbeatManager, TaskExecutionManager, WorkerLifecycle};
