//! Configuration module for the distributed scheduler system
//!
//! This module provides a comprehensive configuration management system
//! that supports multiple configuration sources, validation, and hot-reloading.
//!
//! # Architecture
//!
//! The configuration system is organized into several focused modules:
//!
//! - `sources`: Configuration source implementations
//! - `validation`: Configuration validation framework
//! - `reloading`: Configuration reloading strategies
//! - `metadata`: Configuration metadata management
//! - `builder`: Configuration builder pattern
//! - `typed`: Type-safe configuration wrapper
//! - `manager`: Main configuration manager
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use scheduler_core::config::manager::{ConfigBuilder, ConfigSource};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let manager = ConfigBuilder::new()
//!         .add_source(ConfigSource::Memory {
//!             data: json!({
//!                 "server": {
//!                     "host": "localhost",
//!                     "port": 8080
//!                 }
//!             }),
//!         })
//!         .build();
//!     
//!     manager.load().await?;
//!     let host: String = manager.get("server.host").await?;
//!     
//!     println!("Server running on {}", host);
//!     Ok(())
//! }
//! ```

pub mod builder;
pub mod config_callback_manager;
pub mod config_hot_reload_manager;
pub mod config_loader;
pub mod config_metadata_manager;
pub mod manager;
pub mod metadata;
pub mod reloading;
pub mod sources;
pub mod typed;
pub mod validation;

// Re-export commonly used types
pub use builder::ConfigBuilder;
pub use manager::ConfigurationService;
pub use manager::UnifiedConfigManager;
pub use metadata::ConfigMetadata;
pub use reloading::ReloadStrategy;
pub use sources::ConfigSource;
pub use typed::TypedConfig;
pub use validation::{BasicConfigValidator, ConfigValidationError, ConfigValidator};
