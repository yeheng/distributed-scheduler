// Core configuration abstractions
pub mod core;
// Configuration validation
pub mod validation;
// File-based configuration loading
pub mod loader;
// Environment-specific configuration
pub mod environment;
// Hot-reload functionality
pub mod hot_reload;
// Configuration builders and utilities
// Organized configuration models
pub mod models;
// Service implementations
pub mod services;
// Organized test modules
#[cfg(test)]
pub mod tests;

// Legacy modules for backward compatibility
pub mod config_loader;
pub mod builder;

// Re-export key types and traits for easier imports
pub use core::{ConfigurationService, TypedConfig, ConfigValue, ConfigSource};
pub use validation::{ConfigValidator, ConfigValidationError, BasicConfigValidator};
pub use loader::{ConfigLoader as FileConfigLoaderTrait, FileConfigLoader, ConfigSourceType, MultiSourceLoader, MergeStrategy};
pub use environment::{Environment, ConfigProfile};
pub use hot_reload::{HotReloadManager, ConfigChangeEvent};
pub use models::{AppConfig, DatabaseConfig, MessageQueueConfig, MessageQueueType, RedisConfig, DispatcherConfig, WorkerConfig, ApiConfig, ObservabilityConfig};
pub use services::{ConfigurationService as ServiceTrait, ConfigCache};
pub use builder::{InMemoryConfigService, ConfigBuilder, ConfigManager};

// Legacy exports
pub use config_loader::ConfigLoader as LegacyConfigLoader;

#[cfg(test)]
mod config_test;