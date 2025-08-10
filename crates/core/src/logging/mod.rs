pub mod event_tracker;
pub mod log_config;
pub mod log_level;
pub mod structured_logger;
pub use event_tracker::EventTracker;
pub use log_config::LogConfig;
pub use log_level::LogLevel;
pub use structured_logger::StructuredLogger;

pub struct LoggingService {
    logger: StructuredLogger,
    config: LogConfig,
    event_tracker: EventTracker,
}

impl LoggingService {
    pub fn new() -> Self {
        Self {
            logger: StructuredLogger::new(),
            config: LogConfig::default(),
            event_tracker: EventTracker::new(),
        }
    }
    pub fn with_config(config: LogConfig) -> Self {
        Self {
            logger: StructuredLogger::with_config(&config),
            config,
            event_tracker: EventTracker::new(),
        }
    }
    pub fn log_level(&self) -> LogLevel {
        self.config.level
    }
    pub fn set_log_level(&mut self, level: LogLevel) {
        self.config.level = level;
    }
    pub fn logger(&self) -> &StructuredLogger {
        &self.logger
    }
    pub fn event_tracker(&self) -> &EventTracker {
        &self.event_tracker
    }
    pub fn event_tracker_mut(&mut self) -> &mut EventTracker {
        &mut self.event_tracker
    }
}

impl Default for LoggingService {
    fn default() -> Self {
        Self::new()
    }
}
