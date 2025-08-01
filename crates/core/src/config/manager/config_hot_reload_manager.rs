//! Configuration hot reload manager - Handles configuration hot reloading
//!
//! This component is responsible only for managing configuration hot reloading
//! and file change detection.

use std::fs;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use tokio::sync::RwLock;

use super::{metadata::ConfigMetadata, sources::ConfigSource};
use crate::SchedulerResult;

/// Configuration hot reload manager - Handles configuration hot reloading
/// Follows SRP: Only responsible for hot reloading and file change detection
pub struct ConfigHotReloadManager {
    /// Configuration sources
    sources: Vec<ConfigSource>,

    /// Configuration metadata
    metadata: Arc<RwLock<ConfigMetadata>>,

    /// Reload interval in seconds
    reload_interval_seconds: u64,

    /// Whether hot reload is enabled
    enabled: bool,
}

impl ConfigHotReloadManager {
    /// Create a new hot reload manager
    pub fn new(sources: Vec<ConfigSource>, reload_interval_seconds: u64) -> Self {
        Self {
            sources,
            metadata: Arc::new(RwLock::new(ConfigMetadata::default())),
            reload_interval_seconds,
            enabled: reload_interval_seconds > 0,
        }
    }

    /// Start hot reload monitoring
    pub async fn start_monitoring(&self) -> SchedulerResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let sources = self.sources.clone();
        let metadata = Arc::clone(&self.metadata);
        let reload_interval = Duration::from_secs(self.reload_interval_seconds);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(reload_interval);

            loop {
                interval.tick().await;

                // Check all sources for changes
                for source in &sources {
                    if let ConfigSource::File { path } = source {
                        if let Ok(file_metadata) = fs::metadata(path) {
                            if let Ok(modified) = file_metadata.modified() {
                                let modified_secs = modified
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();

                                let current_metadata = metadata.read().await;
                                if modified_secs > current_metadata.last_modified {
                                    // Update timestamp
                                    let mut metadata_guard = metadata.write().await;
                                    metadata_guard.update_timestamp();
                                    drop(metadata_guard);

                                    // Return true to indicate reload needed
                                    // In a real implementation, this would trigger a reload
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Check if reload is needed
    pub async fn needs_reload(&self) -> bool {
        if !self.enabled {
            return false;
        }

        for source in &self.sources {
            if let ConfigSource::File { path } = source {
                if let Ok(file_metadata) = fs::metadata(path) {
                    if let Ok(modified) = file_metadata.modified() {
                        let modified_secs = modified
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        let metadata = self.metadata.read().await;
                        if modified_secs > metadata.last_modified {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Update last modified timestamp
    pub async fn update_timestamp(&self) {
        let mut metadata = self.metadata.write().await;
        metadata.update_timestamp();
    }

    /// Get reload interval
    pub fn reload_interval(&self) -> Duration {
        Duration::from_secs(self.reload_interval_seconds)
    }

    /// Check if hot reload is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable hot reload
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for ConfigHotReloadManager {
    fn default() -> Self {
        Self::new(Vec::new(), 0)
    }
}
