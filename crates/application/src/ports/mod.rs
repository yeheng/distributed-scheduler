//! # Application Ports
//!
//! This module contains the port interfaces for the application layer.
//! These interfaces define the contracts that external adapters must implement
//! to interact with the application's business logic.
//!
//! ## Port Types
//!
//! - **Primary Ports** (Driving Ports): Interfaces that allow external actors to drive the application
//! - **Secondary Ports** (Driven Ports): Interfaces that the application uses to interact with external systems
//!
//! ## Structure
//!
//! - `scheduler.rs` - Task scheduling and control interfaces (infrastructure)
//! - `executor.rs` - Task execution interfaces
//! - `strategy.rs` - Task dispatch and scheduling strategy interfaces
//! - `service_interfaces.rs` - General service interfaces

pub mod executor;
pub mod scheduler;
pub mod service_interfaces;
pub mod strategy;

// Re-export all ports for convenience
pub use executor::*;
pub use scheduler::*;
pub use service_interfaces::*;
pub use strategy::*;
