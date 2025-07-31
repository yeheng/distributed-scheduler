use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::SchedulerResult;

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

impl LogLevel {
    /// Parse log level from string
    pub fn from_str(level: &str) -> SchedulerResult<Self> {
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

    /// Get current log level from environment variable
    pub fn from_env() -> Self {
        std::env::var("LOG_LEVEL")
            .map(|s| Self::from_str(&s).unwrap_or(Self::Info))
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

/// Log entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Add error
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> SchedulerResult<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::errors::SchedulerError::Internal(format!("Failed to serialize log entry: {e}"))
        })
    }

    /// Convert to pretty JSON string
    pub fn to_pretty_json(&self) -> SchedulerResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            crate::errors::SchedulerError::Internal(format!("Failed to serialize log entry: {e}"))
        })
    }
}

/// Log appender trait
#[async_trait]
pub trait LogAppender: Send + Sync {
    /// Append log entry
    async fn append(&self, entry: &LogEntry) -> SchedulerResult<()>;

    /// Flush pending logs
    async fn flush(&self) -> SchedulerResult<()>;

    /// Shutdown appender
    async fn shutdown(&self) -> SchedulerResult<()>;
}

/// Console appender - Output logs to console
pub struct ConsoleAppender {
    /// Minimum log level
    min_level: LogLevel,
    /// Enable colored output
    colored: bool,
    /// Enable JSON output
    json_output: bool,
}

impl ConsoleAppender {
    /// Create new console appender
    pub fn new(min_level: LogLevel) -> Self {
        Self {
            min_level,
            colored: true,
            json_output: false,
        }
    }

    /// Enable/disable colored output
    pub fn with_colored(mut self, colored: bool) -> Self {
        self.colored = colored;
        self
    }

    /// Enable/disable JSON output
    pub fn with_json_output(mut self, json_output: bool) -> Self {
        self.json_output = json_output;
        self
    }
}

#[async_trait]
impl LogAppender for ConsoleAppender {
    async fn append(&self, entry: &LogEntry) -> SchedulerResult<()> {
        if entry.level < self.min_level {
            return Ok(());
        }

        if self.json_output {
            let json = entry.to_json()?;
            println!("{json}");
        } else {
            let timestamp = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
                .unwrap_or_else(|| "INVALID_TIMESTAMP".to_string());

            let level_str = if self.colored {
                match entry.level {
                    LogLevel::Trace => "\x1b[90mTRACE\x1b[0m",
                    LogLevel::Debug => "\x1b[36mDEBUG\x1b[0m",
                    LogLevel::Info => "\x1b[32mINFO\x1b[0m",
                    LogLevel::Warn => "\x1b[33mWARN\x1b[0m",
                    LogLevel::Error => "\x1b[31mERROR\x1b[0m",
                }
            } else {
                entry.level.as_str()
            };

            println!(
                "{} [{}] {} - {}",
                timestamp, level_str, entry.component, entry.message
            );

            // Print additional context if present
            if !entry.context.is_empty() {
                for (key, value) in &entry.context {
                    println!("  {key}: {value}");
                }
            }

            // Print error if present
            if let Some(error) = &entry.error {
                println!("  Error: {error}");
            }

            // Print request ID if present
            if let Some(request_id) = &entry.request_id {
                println!("  Request ID: {request_id}");
            }

            // Print user ID if present
            if let Some(user_id) = &entry.user_id {
                println!("  User ID: {user_id}");
            }
        }

        Ok(())
    }

    async fn flush(&self) -> SchedulerResult<()> {
        // Console output is immediately flushed
        Ok(())
    }

    async fn shutdown(&self) -> SchedulerResult<()> {
        Ok(())
    }
}

/// Memory appender - Store logs in memory for testing
pub struct MemoryAppender {
    /// Stored log entries
    logs: Arc<RwLock<Vec<LogEntry>>>,
    /// Maximum number of logs to store
    max_logs: usize,
}

impl MemoryAppender {
    /// Create new memory appender
    pub fn new(max_logs: usize) -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
            max_logs,
        }
    }

    /// Get all logs
    pub async fn get_logs(&self) -> Vec<LogEntry> {
        self.logs.read().await.clone()
    }

    /// Clear all logs
    pub async fn clear(&self) {
        self.logs.write().await.clear();
    }

    /// Get logs by level
    pub async fn get_logs_by_level(&self, level: LogLevel) -> Vec<LogEntry> {
        self.logs
            .read()
            .await
            .iter()
            .filter(|log| log.level == level)
            .cloned()
            .collect()
    }

    /// Get logs by component
    pub async fn get_logs_by_component(&self, component: &str) -> Vec<LogEntry> {
        self.logs
            .read()
            .await
            .iter()
            .filter(|log| log.component == component)
            .cloned()
            .collect()
    }
}

#[async_trait]
impl LogAppender for MemoryAppender {
    async fn append(&self, entry: &LogEntry) -> SchedulerResult<()> {
        let mut logs = self.logs.write().await;
        logs.push(entry.clone());

        // Keep only the most recent logs
        if logs.len() > self.max_logs {
            logs.remove(0);
        }

        Ok(())
    }

    async fn flush(&self) -> SchedulerResult<()> {
        Ok(())
    }

    async fn shutdown(&self) -> SchedulerResult<()> {
        Ok(())
    }
}

/// Logger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerConfig {
    /// Minimum log level
    pub min_level: LogLevel,
    /// Enable JSON output
    pub json_output: bool,
    /// Enable colored output
    pub colored_output: bool,
    /// Log appenders
    pub appenders: Vec<String>,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Info,
            json_output: false,
            colored_output: true,
            appenders: vec!["console".to_string()],
        }
    }
}

/// Structured logger
pub struct Logger {
    /// Logger configuration
    config: LoggerConfig,
    /// Log appenders
    appenders: Vec<Arc<dyn LogAppender>>,
    /// Component name
    component: String,
    /// Request ID (for tracing)
    request_id: Option<String>,
    /// User ID (if applicable)
    user_id: Option<String>,
}

impl Logger {
    /// Create new logger
    pub fn new(component: String) -> Self {
        let config = LoggerConfig::default();
        let appenders =
            vec![Arc::new(ConsoleAppender::new(config.min_level)) as Arc<dyn LogAppender>];

        Self {
            config,
            appenders,
            component,
            request_id: None,
            user_id: None,
        }
    }

    /// Create logger with custom configuration
    pub fn with_config(component: String, config: LoggerConfig) -> SchedulerResult<Self> {
        let mut appenders: Vec<Arc<dyn LogAppender>> = Vec::new();

        for appender_name in &config.appenders {
            match appender_name.as_str() {
                "console" => {
                    appenders.push(Arc::new(
                        ConsoleAppender::new(config.min_level)
                            .with_colored(config.colored_output)
                            .with_json_output(config.json_output),
                    ));
                }
                "memory" => {
                    appenders.push(Arc::new(MemoryAppender::new(1000)));
                }
                _ => {
                    return Err(crate::errors::SchedulerError::Configuration(format!(
                        "Unknown log appender: {appender_name}"
                    )));
                }
            }
        }

        Ok(Self {
            config,
            appenders,
            component,
            request_id: None,
            user_id: None,
        })
    }

    /// Set request ID
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Set user ID
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Create child logger with sub-component name
    pub fn child(&self, sub_component: &str) -> Self {
        let component = format!("{}.{}", self.component, sub_component);
        // Create new Arc references to the same appenders
        let appenders: Vec<Arc<dyn LogAppender>> = self.appenders.iter().map(Arc::clone).collect();

        Self {
            config: self.config.clone(),
            appenders,
            component,
            request_id: self.request_id.clone(),
            user_id: self.user_id.clone(),
        }
    }

    /// Log message at specified level
    async fn log(
        &self,
        level: LogLevel,
        message: String,
        context: Option<HashMap<String, serde_json::Value>>,
        error: Option<String>,
    ) {
        if level < self.config.min_level {
            return;
        }

        let mut entry = LogEntry::new(level, self.component.clone(), message);

        if let Some(ctx) = context {
            for (key, value) in ctx {
                entry = entry.with_context(key, value);
            }
        }

        if let Some(req_id) = &self.request_id {
            entry = entry.with_request_id(req_id.clone());
        }

        if let Some(user_id) = &self.user_id {
            entry = entry.with_user_id(user_id.clone());
        }

        if let Some(err) = error {
            entry = entry.with_error(err);
        }

        // Append to all appenders
        for appender in &self.appenders {
            if let Err(e) = appender.append(&entry).await {
                // Log appender error to stderr
                eprintln!("Failed to append log entry: {e}");
            }
        }
    }

    /// Log trace message
    pub async fn trace(&self, message: String) {
        self.log(LogLevel::Trace, message, None, None).await;
    }

    /// Log trace message with context
    pub async fn trace_with_context(
        &self,
        message: String,
        context: HashMap<String, serde_json::Value>,
    ) {
        self.log(LogLevel::Trace, message, Some(context), None)
            .await;
    }

    /// Log debug message
    pub async fn debug(&self, message: String) {
        self.log(LogLevel::Debug, message, None, None).await;
    }

    /// Log debug message with context
    pub async fn debug_with_context(
        &self,
        message: String,
        context: HashMap<String, serde_json::Value>,
    ) {
        self.log(LogLevel::Debug, message, Some(context), None)
            .await;
    }

    /// Log info message
    pub async fn info(&self, message: String) {
        self.log(LogLevel::Info, message, None, None).await;
    }

    /// Log info message with context
    pub async fn info_with_context(
        &self,
        message: String,
        context: HashMap<String, serde_json::Value>,
    ) {
        self.log(LogLevel::Info, message, Some(context), None).await;
    }

    /// Log warning message
    pub async fn warn(&self, message: String) {
        self.log(LogLevel::Warn, message, None, None).await;
    }

    /// Log warning message with context
    pub async fn warn_with_context(
        &self,
        message: String,
        context: HashMap<String, serde_json::Value>,
    ) {
        self.log(LogLevel::Warn, message, Some(context), None).await;
    }

    /// Log error message
    pub async fn error(&self, message: String) {
        self.log(LogLevel::Error, message, None, None).await;
    }

    /// Log error message with context
    pub async fn error_with_context(
        &self,
        message: String,
        context: HashMap<String, serde_json::Value>,
    ) {
        self.log(LogLevel::Error, message, Some(context), None)
            .await;
    }

    /// Log error message with error details
    pub async fn error_with_error(&self, message: String, error: String) {
        self.log(LogLevel::Error, message, None, Some(error)).await;
    }

    /// Log error message with context and error details
    pub async fn error_with_context_and_error(
        &self,
        message: String,
        context: HashMap<String, serde_json::Value>,
        error: String,
    ) {
        self.log(LogLevel::Error, message, Some(context), Some(error))
            .await;
    }

    /// Flush all appenders
    pub async fn flush(&self) -> SchedulerResult<()> {
        for appender in &self.appenders {
            appender.flush().await?;
        }
        Ok(())
    }

    /// Shutdown logger
    pub async fn shutdown(&self) -> SchedulerResult<()> {
        for appender in &self.appenders {
            appender.shutdown().await?;
        }
        Ok(())
    }
}

/// Global logger instance
static GLOBAL_LOGGER: std::sync::OnceLock<Arc<Logger>> = std::sync::OnceLock::new();

/// Initialize global logger
pub fn init_global_logger(component: String) -> SchedulerResult<Arc<Logger>> {
    let logger = Arc::new(Logger::new(component));
    GLOBAL_LOGGER.set(logger.clone()).map_err(|_| {
        crate::errors::SchedulerError::Internal("Global logger already initialized".to_string())
    })?;
    Ok(logger)
}

/// Initialize global logger with custom configuration
pub fn init_global_logger_with_config(
    component: String,
    config: LoggerConfig,
) -> SchedulerResult<Arc<Logger>> {
    let logger = Arc::new(Logger::with_config(component, config)?);
    GLOBAL_LOGGER.set(logger.clone()).map_err(|_| {
        crate::errors::SchedulerError::Internal("Global logger already initialized".to_string())
    })?;
    Ok(logger)
}

/// Get global logger instance
pub fn global_logger() -> Option<Arc<Logger>> {
    GLOBAL_LOGGER.get().cloned()
}

/// Global logging macros
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logging::global_logger() {
            logger.trace(format!($($arg)*)).await;
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logging::global_logger() {
            logger.debug(format!($($arg)*)).await;
        }
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logging::global_logger() {
            logger.info(format!($($arg)*)).await;
        }
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logging::global_logger() {
            logger.warn(format!($($arg)*)).await;
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        if let Some(logger) = $crate::logging::global_logger() {
            logger.error(format!($($arg)*)).await;
        }
    };
}

#[macro_export]
macro_rules! log_error_with_error {
    ($msg:expr, $error:expr) => {
        if let Some(logger) = $crate::logging::global_logger() {
            logger
                .error_with_error(format!($msg), $error.to_string())
                .await;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_log_levels() {
        let logger = Logger::new("test_component".to_string());

        // Test all log levels
        logger.trace("Trace message".to_string()).await;
        logger.debug("Debug message".to_string()).await;
        logger.info("Info message".to_string()).await;
        logger.warn("Warning message".to_string()).await;
        logger.error("Error message".to_string()).await;

        // Test with context
        let mut context = HashMap::new();
        context.insert("key1".to_string(), json!("value1"));
        context.insert("key2".to_string(), json!(42));

        logger
            .info_with_context("Info with context".to_string(), context)
            .await;

        // Test with error
        logger
            .error_with_error(
                "Error occurred".to_string(),
                "Something went wrong".to_string(),
            )
            .await;

        // Test with context and error
        let mut context = HashMap::new();
        context.insert("operation".to_string(), json!("test_operation"));

        logger
            .error_with_context_and_error(
                "Error with context".to_string(),
                context,
                "Detailed error".to_string(),
            )
            .await;
    }

    #[tokio::test]
    async fn test_logger_with_request_id() {
        let logger =
            Logger::new("test_component".to_string()).with_request_id("req-123".to_string());

        logger.info("Test message".to_string()).await;
    }

    #[tokio::test]
    async fn test_logger_with_user_id() {
        let logger = Logger::new("test_component".to_string()).with_user_id("user-456".to_string());

        logger.info("Test message".to_string()).await;
    }

    #[tokio::test]
    async fn test_child_logger() {
        let parent = Logger::new("parent".to_string());
        let child = parent.child("child");

        parent.info("Parent message".to_string()).await;
        child.info("Child message".to_string()).await;
    }

    #[tokio::test]
    async fn test_memory_appender() {
        let appender = MemoryAppender::new(10);
        let entry = LogEntry::new(
            LogLevel::Info,
            "test".to_string(),
            "Test message".to_string(),
        );

        appender.append(&entry).await.unwrap();

        let logs = appender.get_logs().await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "Test message");

        let info_logs = appender.get_logs_by_level(LogLevel::Info).await;
        assert_eq!(info_logs.len(), 1);

        let test_logs = appender.get_logs_by_component("test").await;
        assert_eq!(test_logs.len(), 1);
    }

    #[tokio::test]
    async fn test_log_entry_serialization() {
        let mut context = HashMap::new();
        context.insert("key".to_string(), json!("value"));

        let entry = LogEntry::new(
            LogLevel::Info,
            "test".to_string(),
            "Test message".to_string(),
        )
        .with_context("context_key".to_string(), json!("context_value"))
        .with_request_id("req-123".to_string())
        .with_user_id("user-456".to_string())
        .with_error("Test error".to_string());

        let json = entry.to_json().unwrap();
        let deserialized: LogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.component, "test");
        assert_eq!(deserialized.message, "Test message");
        assert_eq!(deserialized.level, LogLevel::Info);
        assert_eq!(deserialized.request_id, Some("req-123".to_string()));
        assert_eq!(deserialized.user_id, Some("user-456".to_string()));
        assert_eq!(deserialized.error, Some("Test error".to_string()));
        assert_eq!(
            deserialized.context.get("context_key"),
            Some(&json!("context_value"))
        );
    }

    #[tokio::test]
    async fn test_global_logger() {
        let logger = init_global_logger("global_test".to_string()).unwrap();

        log_info!("Global logger test message");

        // Test macros
        log_trace!("Trace from macro");
        log_debug!("Debug from macro");
        log_info!("Info from macro");
        log_warn!("Warning from macro");
        log_error!("Error from macro");

        logger.flush().await.unwrap();
    }

    #[tokio::test]
    async fn test_logger_config() {
        let config = LoggerConfig {
            min_level: LogLevel::Debug,
            json_output: true,
            colored_output: false,
            appenders: vec!["console".to_string(), "memory".to_string()],
        };

        let logger = Logger::with_config("config_test".to_string(), config).unwrap();

        logger.info("Config test message".to_string()).await;
    }
}
