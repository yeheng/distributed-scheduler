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
//! - `app_scheduler.rs` - Application scheduler interfaces
//! - `messaging.rs` - Message queue interfaces  
//! - `executor.rs` - Task execution interfaces
//! - `strategy.rs` - Task dispatch and scheduling strategy interfaces
//! - `task_executor.rs` - Task executor interfaces
//! - `service_interfaces.rs` - General service interfaces

pub mod app_scheduler;
pub mod executor;
pub mod messaging;
pub mod scheduler;
pub mod service_interfaces;
pub mod strategy;
pub mod task_executor;

// Re-export all ports for convenience
pub use app_scheduler::*;
pub use executor::*;
pub use messaging::*;
pub use scheduler::*;
pub use service_interfaces::*;
pub use strategy::*;
pub use task_executor::*;
