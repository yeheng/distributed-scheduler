//! Configuration metadata management
//!
//! This module provides metadata tracking for configuration values.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuration metadata
///
/// Contains version information, modification time, checksum, and validation status.
/// This information is used for configuration monitoring, auditing, and debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    /// Configuration version
    pub version: String,

    /// Last modification timestamp (Unix timestamp)
    pub last_modified: u64,

    /// Configuration checksum
    pub checksum: String,

    /// Configuration source information
    pub source: String,

    /// Whether the configuration is valid
    pub is_valid: bool,

    /// Validation error list
    pub validation_errors: Vec<String>,
}

impl Default for ConfigMetadata {
    /// Create default configuration metadata
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            last_modified: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            checksum: String::new(),
            source: "unknown".to_string(),
            is_valid: true,
            validation_errors: Vec::new(),
        }
    }
}

impl ConfigMetadata {
    /// Create new configuration metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the configuration version
    pub fn with_version(mut self, version: String) -> Self {
        self.version = version;
        self
    }

    /// Set the configuration source
    pub fn with_source(mut self, source: String) -> Self {
        self.source = source;
        self
    }

    /// Set the checksum
    pub fn with_checksum(mut self, checksum: String) -> Self {
        self.checksum = checksum;
        self
    }

    /// Mark configuration as valid
    pub fn mark_valid(mut self) -> Self {
        self.is_valid = true;
        self.validation_errors.clear();
        self
    }

    /// Mark configuration as invalid with errors
    pub fn mark_invalid(mut self, errors: Vec<String>) -> Self {
        self.is_valid = false;
        self.validation_errors = errors;
        self
    }

    /// Update the modification timestamp
    pub fn update_timestamp(&mut self) {
        self.last_modified = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Check if the configuration is valid
    pub fn is_valid(&self) -> bool {
        self.is_valid && self.validation_errors.is_empty()
    }

    /// Get the validation errors
    pub fn validation_errors(&self) -> &[String] {
        &self.validation_errors
    }

    /// Get the configuration version
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get the last modification time
    pub fn last_modified(&self) -> u64 {
        self.last_modified
    }

    /// Get the checksum
    pub fn checksum(&self) -> &str {
        &self.checksum
    }

    /// Get the source information
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Clone the metadata with updated timestamp
    pub fn clone_with_timestamp(&self) -> Self {
        let mut cloned = self.clone();
        cloned.update_timestamp();
        cloned
    }
}

/// Configuration metadata builder
pub struct ConfigMetadataBuilder {
    metadata: ConfigMetadata,
}

impl ConfigMetadataBuilder {
    /// Create a new metadata builder
    pub fn new() -> Self {
        Self {
            metadata: ConfigMetadata::default(),
        }
    }

    /// Set the version
    pub fn version(mut self, version: String) -> Self {
        self.metadata.version = version;
        self
    }

    /// Set the source
    pub fn source(mut self, source: String) -> Self {
        self.metadata.source = source;
        self
    }

    /// Set the checksum
    pub fn checksum(mut self, checksum: String) -> Self {
        self.metadata.checksum = checksum;
        self
    }

    /// Mark as valid
    pub fn valid(mut self) -> Self {
        self.metadata.is_valid = true;
        self.metadata.validation_errors.clear();
        self
    }

    /// Add validation error
    pub fn add_error(mut self, error: String) -> Self {
        self.metadata.is_valid = false;
        self.metadata.validation_errors.push(error);
        self
    }

    /// Build the metadata
    pub fn build(self) -> ConfigMetadata {
        self.metadata
    }
}

impl Default for ConfigMetadataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_metadata() {
        let metadata = ConfigMetadata::default();

        assert_eq!(metadata.version, "1.0.0");
        assert!(metadata.is_valid);
        assert!(metadata.validation_errors.is_empty());
        assert_eq!(metadata.source, "unknown");
        assert!(metadata.last_modified > 0);
    }

    #[test]
    fn test_metadata_builder() {
        let metadata = ConfigMetadataBuilder::new()
            .version("2.0.0".to_string())
            .source("test".to_string())
            .checksum("abc123".to_string())
            .valid()
            .build();

        assert_eq!(metadata.version, "2.0.0");
        assert_eq!(metadata.source, "test");
        assert_eq!(metadata.checksum, "abc123");
        assert!(metadata.is_valid);
    }

    #[test]
    fn test_metadata_with_errors() {
        let metadata = ConfigMetadataBuilder::new()
            .add_error("Invalid port".to_string())
            .add_error("Missing host".to_string())
            .build();

        assert!(!metadata.is_valid);
        assert_eq!(metadata.validation_errors.len(), 2);
        assert!(metadata
            .validation_errors
            .contains(&"Invalid port".to_string()));
        assert!(metadata
            .validation_errors
            .contains(&"Missing host".to_string()));
    }

    #[test]
    fn test_metadata_clone_with_timestamp() {
        let original = ConfigMetadata::default();
        let timestamp1 = original.last_modified;

        // Wait a tiny bit to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        let cloned = original.clone_with_timestamp();
        let timestamp2 = cloned.last_modified;

        assert!(timestamp2 > timestamp1);
        assert_eq!(cloned.version, original.version);
        assert_eq!(cloned.source, original.source);
    }
}
