use std::str::FromStr;

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum LogLevel {
    /// Trace level - Very detailed logging
    Trace = 0,
    /// Debug level - Debug information
    Debug = 1,
    /// Info level - General information
    Info = 2,
    /// Warn level - Warning messages
    Warn = 3,
    /// Error level - Error messages
    Error = 4,
}

impl FromStr for LogLevel {
    type Err = crate::errors::SchedulerError;

    fn from_str(level: &str) -> Result<Self, Self::Err> {
        match level.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(crate::errors::SchedulerError::Configuration(format!(
                "Invalid log level: {level}"
            ))),
        }
    }
}

impl LogLevel {
    /// Get current log level from environment variable
    pub fn from_env() -> Self {
        std::env::var("LOG_LEVEL")
            .map(|s| s.parse().unwrap_or(Self::Info))
            .unwrap_or(Self::Info)
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}