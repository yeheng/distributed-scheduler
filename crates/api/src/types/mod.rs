//! Type definitions for the API
//!
//! This module contains common types used throughout the API implementation,
//! including update value types for precise PATCH operations.

pub mod update_value;

// Re-export commonly used types
pub use update_value::{NumericUpdateValue, UpdateValue};
