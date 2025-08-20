pub mod di;

// Re-export the DI types for external crates
pub use di::{ApplicationContext, ServiceContainer, ServiceLocator};