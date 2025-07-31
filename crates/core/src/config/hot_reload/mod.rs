pub mod events;
pub mod watchers;
pub mod manager;

// Re-export main types for easier imports
pub use events::{ConfigChangeEvent, ConfigChangeSource};
pub use watchers::{ConfigWatcher, FileConfigWatcher};
pub use manager::HotReloadManager;