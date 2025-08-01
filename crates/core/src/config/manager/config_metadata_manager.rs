//! Configuration metadata manager - Handles configuration metadata and validation state
//!
//! This component is responsible only for managing configuration metadata,
//! validation state, and timestamps.

use std::sync::Arc;
use tokio::sync::RwLock;

use super::metadata::ConfigMetadata;

/// Configuration metadata manager - Handles configuration metadata and validation state
/// Follows SRP: Only responsible for metadata management
pub struct ConfigMetadataManager {
    /// Configuration metadata
    metadata: Arc<RwLock<ConfigMetadata>>,
}

impl ConfigMetadataManager {
    /// Create a new metadata manager
    pub fn new() -> Self {
        Self {
            metadata: Arc::new(RwLock::new(ConfigMetadata::default())),
        }
    }

    /// Update metadata after successful validation
    pub async fn mark_valid(&self) {
        let mut metadata = self.metadata.write().await;
        metadata.is_valid = true;
        metadata.validation_errors.clear();
        metadata.update_timestamp();
    }

    /// Update metadata after validation failure
    pub async fn mark_invalid(&self, errors: Vec<String>) {
        let mut metadata = self.metadata.write().await;
        metadata.is_valid = false;
        metadata.validation_errors = errors;
        metadata.update_timestamp();
    }

    /// Update timestamp
    pub async fn update_timestamp(&self) {
        let mut metadata = self.metadata.write().await;
        metadata.update_timestamp();
    }

    /// Check if configuration is valid
    pub async fn is_valid(&self) -> bool {
        self.metadata.read().await.is_valid()
    }

    /// Get validation errors
    pub async fn get_validation_errors(&self) -> Vec<String> {
        self.metadata.read().await.validation_errors().to_vec()
    }

    /// Get metadata
    pub async fn get_metadata(&self) -> ConfigMetadata {
        self.metadata.read().await.clone()
    }

    /// Get last modified timestamp
    pub async fn get_last_modified(&self) -> u64 {
        self.metadata.read().await.last_modified
    }

    /// Check if reload is needed based on file modification time
    pub async fn needs_reload(&self, file_modified: u64) -> bool {
        let metadata = self.metadata.read().await;
        file_modified > metadata.last_modified
    }
}

impl Default for ConfigMetadataManager {
    fn default() -> Self {
        Self::new()
    }
}
