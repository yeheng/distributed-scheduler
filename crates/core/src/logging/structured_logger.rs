use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::logging::log_level::LogLevel;
use crate::logging::log_config::OutputFormat;

/// Log entry structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    /// Timestamp
    pub timestamp: u64,
    /// Log level
    pub level: LogLevel,
    /// Component/module name
    pub component: String,
    /// Message
    pub message: String,
    /// Additional context data
    pub context: HashMap<String, serde_json::Value>,
    /// Request ID (for tracing)
    pub request_id: Option<String>,
    /// User ID (if applicable)
    pub user_id: Option<String>,
    /// Error details (if any)
    pub error: Option<String>,
}

impl LogEntry {
    /// Create new log entry
    pub fn new(level: LogLevel, component: String, message: String) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            level,
            component,
            message,
            context: HashMap::new(),
            request_id: None,
            user_id: None,
            error: None,
        }
    }

    /// Add context field
    pub fn with_context(mut self, key: String, value: serde_json::Value) -> Self {
        self.context.insert(key, value);
        self
    }

    /// Add request ID
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Add user ID
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Add error details
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Convert to pretty JSON string
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Structured logger implementation
pub struct StructuredLogger {
    log_level: LogLevel,
    output_format: crate::logging::log_config::OutputFormat,
}

impl StructuredLogger {
    /// Create new structured logger with default settings
    pub fn new() -> Self {
        Self {
            log_level: LogLevel::from_env(),
            output_format: OutputFormat::Json,
        }
    }

    /// Create logger with specific configuration
    pub fn with_config(config: &crate::logging::log_config::LogConfig) -> Self {
        Self {
            log_level: config.level,
            output_format: config.format,
        }
    }

    /// Log a message at the specified level
    pub fn log(&self, level: LogLevel, component: &str, message: &str) {
        if level < self.log_level {
            return;
        }

        let entry = LogEntry::new(level, component.to_string(), message.to_string());
        self.write_entry(&entry);
    }

    /// Log with context
    pub fn log_with_context(
        &self,
        level: LogLevel,
        component: &str,
        message: &str,
        context: HashMap<String, serde_json::Value>,
    ) {
        if level < self.log_level {
            return;
        }

        let entry = LogEntry::new(level, component.to_string(), message.to_string())
            .with_context("context".to_string(), serde_json::json!(context));
        self.write_entry(&entry);
    }

    /// Log error
    pub fn log_error(&self, component: &str, message: &str, error: &str) {
        let entry = LogEntry::new(LogLevel::Error, component.to_string(), message.to_string())
            .with_error(error.to_string());
        self.write_entry(&entry);
    }

    /// Write log entry to output
    fn write_entry(&self, entry: &LogEntry) {
        let output = match self.output_format {
            OutputFormat::Json => entry.to_json().unwrap_or_else(|_| "Invalid log entry".to_string()),
            OutputFormat::Pretty => entry.to_json_pretty().unwrap_or_else(|_| "Invalid log entry".to_string()),
            OutputFormat::Text => self.format_as_text(entry),
        };

        println!("{output}");
    }

    /// Format log entry as plain text
    fn format_as_text(&self, entry: &LogEntry) -> String {
        let timestamp = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Invalid timestamp".to_string());

        format!(
            "[{}] [{}] {}: {}",
            timestamp,
            entry.level.as_str(),
            entry.component,
            entry.message
        )
    }

    /// Set log level
    pub fn set_level(&mut self, level: LogLevel) {
        self.log_level = level;
    }

    /// Get current log level
    pub fn level(&self) -> LogLevel {
        self.log_level
    }
}

impl Default for StructuredLogger {
    fn default() -> Self {
        Self::new()
    }
}