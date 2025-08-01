//! Configuration reloading strategies
//!
//! This module provides different strategies for reloading configuration.

use serde::{Deserialize, Serialize};

/// Configuration reload strategy
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ReloadStrategy {
    /// Manual reload
    ///
    /// Only reloads when explicitly calling the reload() method.
    /// This is the default strategy, suitable for production environments.
    #[default]
    Manual,

    /// Auto reload
    ///
    /// Monitors configuration files for changes and automatically reloads.
    ///
    /// # Parameters
    ///
    /// * `debounce_ms` - Debounce time in milliseconds to avoid frequent reloads
    Auto { debounce_ms: u64 },

    /// Periodic reload
    ///
    /// Reloads configuration at specified intervals.
    ///
    /// # Parameters
    ///
    /// * `interval_seconds` - Reload interval in seconds
    Periodic { interval_seconds: u64 },
}

/// Configuration reload manager
pub struct ReloadManager {
    strategy: ReloadStrategy,
}

impl ReloadManager {
    /// Create a new reload manager
    pub fn new(strategy: ReloadStrategy) -> Self {
        Self { strategy }
    }

    /// Get the current reload strategy
    pub fn strategy(&self) -> &ReloadStrategy {
        &self.strategy
    }

    /// Check if auto-reload is enabled
    pub fn is_auto_reload_enabled(&self) -> bool {
        matches!(self.strategy, ReloadStrategy::Auto { .. })
    }

    /// Check if periodic reload is enabled
    pub fn is_periodic_reload_enabled(&self) -> bool {
        matches!(self.strategy, ReloadStrategy::Periodic { .. })
    }

    /// Get the debounce interval for auto reload
    pub fn debounce_interval_ms(&self) -> Option<u64> {
        match self.strategy {
            ReloadStrategy::Auto { debounce_ms } => Some(debounce_ms),
            _ => None,
        }
    }

    /// Get the periodic reload interval
    pub fn periodic_interval_seconds(&self) -> Option<u64> {
        match self.strategy {
            ReloadStrategy::Periodic { interval_seconds } => Some(interval_seconds),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reload_strategy_default() {
        let strategy = ReloadStrategy::default();
        assert!(matches!(strategy, ReloadStrategy::Manual));
    }

    #[test]
    fn test_reload_manager() {
        let strategy = ReloadStrategy::Auto { debounce_ms: 1000 };
        let manager = ReloadManager::new(strategy);

        assert!(manager.is_auto_reload_enabled());
        assert!(!manager.is_periodic_reload_enabled());
        assert_eq!(manager.debounce_interval_ms(), Some(1000));
        assert_eq!(manager.periodic_interval_seconds(), None);
    }

    #[test]
    fn test_periodic_reload_manager() {
        let strategy = ReloadStrategy::Periodic {
            interval_seconds: 60,
        };
        let manager = ReloadManager::new(strategy);

        assert!(!manager.is_auto_reload_enabled());
        assert!(manager.is_periodic_reload_enabled());
        assert_eq!(manager.debounce_interval_ms(), None);
        assert_eq!(manager.periodic_interval_seconds(), Some(60));
    }
}
