//! Logging module
//!
//! This module provides structured logging capabilities for the scheduler system.
//! It is organized into focused sub-modules for better maintainability.

pub mod log_level;
pub mod structured_logger;
pub mod log_config;
pub mod event_tracker;

// Re-export commonly used types
pub use log_level::LogLevel;
pub use structured_logger::StructuredLogger;
pub use log_config::LogConfig;
pub use event_tracker::EventTracker;

/// Main logging interface
pub struct LoggingService {
    logger: StructuredLogger,
    config: LogConfig,
    event_tracker: EventTracker,
}

impl LoggingService {
    /// Create a new logging service with default configuration
    pub fn new() -> Self {
        Self {
            logger: StructuredLogger::new(),
            config: LogConfig::default(),
            event_tracker: EventTracker::new(),
        }
    }
    
    /// Create a logging service with custom configuration
    pub fn with_config(config: LogConfig) -> Self {
        Self {
            logger: StructuredLogger::with_config(&config),
            config,
            event_tracker: EventTracker::new(),
        }
    }
    
    /// Get the current log level
    pub fn log_level(&self) -> LogLevel {
        self.config.level
    }
    
    /// Set the log level
    pub fn set_log_level(&mut self, level: LogLevel) {
        self.config.level = level;
    }
    
    /// Get the structured logger
    pub fn logger(&self) -> &StructuredLogger {
        &self.logger
    }
    
    /// Get the event tracker
    pub fn event_tracker(&self) -> &EventTracker {
        &self.event_tracker
    }
    
    /// Get mutable reference to event tracker
    pub fn event_tracker_mut(&mut self) -> &mut EventTracker {
        &mut self.event_tracker
    }
}

impl Default for LoggingService {
    fn default() -> Self {
        Self::new()
    }
}