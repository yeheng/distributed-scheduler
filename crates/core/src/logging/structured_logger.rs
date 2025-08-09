use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::logging::log_level::LogLevel;
use crate::logging::log_config::OutputFormat;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub component: String,
    pub message: String,
    pub context: HashMap<String, serde_json::Value>,
    pub request_id: Option<String>,
    pub user_id: Option<String>,
    pub error: Option<String>,
}

impl LogEntry {
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
    pub fn with_context(mut self, key: String, value: serde_json::Value) -> Self {
        self.context.insert(key, value);
        self
    }
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub struct StructuredLogger {
    log_level: LogLevel,
    output_format: crate::logging::log_config::OutputFormat,
}

impl StructuredLogger {
    pub fn new() -> Self {
        Self {
            log_level: LogLevel::from_env(),
            output_format: OutputFormat::Json,
        }
    }
    pub fn with_config(config: &crate::logging::log_config::LogConfig) -> Self {
        Self {
            log_level: config.level,
            output_format: config.format,
        }
    }
    pub fn log(&self, level: LogLevel, component: &str, message: &str) {
        if level < self.log_level {
            return;
        }

        let entry = LogEntry::new(level, component.to_string(), message.to_string());
        self.write_entry(&entry);
    }
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
    pub fn log_error(&self, component: &str, message: &str, error: &str) {
        let entry = LogEntry::new(LogLevel::Error, component.to_string(), message.to_string())
            .with_error(error.to_string());
        self.write_entry(&entry);
    }
    fn write_entry(&self, entry: &LogEntry) {
        let output = match self.output_format {
            OutputFormat::Json => entry.to_json().unwrap_or_else(|_| "Invalid log entry".to_string()),
            OutputFormat::Pretty => entry.to_json_pretty().unwrap_or_else(|_| "Invalid log entry".to_string()),
            OutputFormat::Text => self.format_as_text(entry),
        };

        println!("{output}");
    }
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
    pub fn set_level(&mut self, level: LogLevel) {
        self.log_level = level;
    }
    pub fn level(&self) -> LogLevel {
        self.log_level
    }
}

impl Default for StructuredLogger {
    fn default() -> Self {
        Self::new()
    }
}