//! Dispatcher Composition Root
//! 
//! This crate serves as the composition root for the dispatcher service,
//! coordinating the assembly and initialization of all dispatcher components.
//! The core business logic has been moved to the application layer.

pub mod controller;
pub mod recovery_service;
pub mod retry_service;
pub mod scheduler;
pub mod state_listener;
pub mod strategies;
pub mod worker_failure_detector;

#[cfg(test)]
pub mod test_utils;

// Re-export key components that may be used by the composition root
pub use controller::*;
pub use strategies::*;
