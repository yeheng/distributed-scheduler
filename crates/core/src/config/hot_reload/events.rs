use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use crate::config::core::ConfigValue;

/// Configuration change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    /// Changed key
    pub key: String,
    /// Old value
    pub old_value: Option<ConfigValue>,
    /// New value
    pub new_value: Option<ConfigValue>,
    /// Change timestamp
    pub timestamp: SystemTime,
    /// Change source
    pub source: ConfigChangeSource,
}

/// Source of configuration change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigChangeSource {
    File(String),
    Environment,
    Runtime,
    HotReload,
}