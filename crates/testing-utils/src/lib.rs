//! # Scheduler Testing Utils
//! 
//! Shared testing utilities for the distributed task scheduler system.
//! This crate provides mock implementations, test containers, and testing helpers
//! that can be used across all other crates in the workspace.
//!
//! ## Features
//! 
//! - **Mock Repositories**: In-memory implementations of all repository traits
//! - **Mock Services**: Test doubles for all service interfaces  
//! - **Database Test Containers**: PostgreSQL test container with migrations
//! - **Message Queue Mocks**: In-memory message queue for testing
//! - **Test Data Builders**: Utilities for creating test data
//! - **Integration Test Helpers**: Common setup and teardown utilities
//!
//! ## Usage
//!
//! Add this crate as a dev-dependency:
//!
//! ```toml
//! [dev-dependencies]
//! scheduler-testing-utils = { path = "../testing-utils" }
//! ```
//!
//! Then use the mocks in your tests:
//!
//! ```rust
//! use scheduler_testing_utils::mocks::*;
//! use scheduler_testing_utils::containers::DatabaseTestContainer;
//! ```

pub mod mocks;
pub mod containers;
pub mod builders;
pub mod helpers;

// Re-export commonly used items
pub use mocks::*;
pub use containers::*;
pub use builders::*;
pub use helpers::*;