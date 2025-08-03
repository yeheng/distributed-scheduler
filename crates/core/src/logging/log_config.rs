use crate::logging::log_level::LogLevel;

/// Logging configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogConfig {
    /// Minimum log level to display
    pub level: LogLevel,
    /// Output format for logs
    pub format: OutputFormat,
    /// Whether to enable file logging
    pub enable_file_logging: bool,
    /// File path for log output (if enabled)
    pub log_file_path: Option<String>,
    /// Maximum file size in bytes before rotation
    pub max_file_size: Option<u64>,
    /// Maximum number of rotated files to keep
    pub max_files: Option<u32>,
    /// Whether to include timestamps in logs
    pub include_timestamps: bool,
    /// Whether to include caller information
    pub include_caller: bool,
    /// Additional context fields to include
    pub default_context: std::collections::HashMap<String, serde_json::Value>,
}

/// Output format for log entries
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum OutputFormat {
    /// JSON format
    Json,
    /// Plain text format
    Text,
    /// Pretty-printed JSON
    Pretty,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: OutputFormat::Json,
            enable_file_logging: false,
            log_file_path: None,
            max_file_size: Some(10 * 1024 * 1024), // 10MB
            max_files: Some(5),
            include_timestamps: true,
            include_caller: false,
            default_context: std::collections::HashMap::new(),
        }
    }
}

impl LogConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Log level from environment
        if let Ok(level_str) = std::env::var("LOG_LEVEL") {
            if let Ok(level) = level_str.parse::<LogLevel>() {
                config.level = level;
            }
        }

        // Output format from environment
        if let Ok(format_str) = std::env::var("LOG_FORMAT") {
            config.format = match format_str.to_lowercase().as_str() {
                "json" => OutputFormat::Json,
                "text" => OutputFormat::Text,
                "pretty" => OutputFormat::Pretty,
                _ => OutputFormat::Json,
            };
        }

        // File logging configuration
        if let Ok(enable_file) = std::env::var("LOG_TO_FILE") {
            config.enable_file_logging = enable_file.to_lowercase() == "true";
        }

        if config.enable_file_logging {
            config.log_file_path = std::env::var("LOG_FILE_PATH").ok();
            
            if let Ok(max_size_str) = std::env::var("LOG_MAX_SIZE") {
                config.max_file_size = max_size_str.parse().ok();
            }
            
            if let Ok(max_files_str) = std::env::var("LOG_MAX_FILES") {
                config.max_files = max_files_str.parse().ok();
            }
        }

        // Other configuration options
        if let Ok(include_timestamps) = std::env::var("LOG_TIMESTAMPS") {
            config.include_timestamps = include_timestamps.to_lowercase() == "true";
        }

        if let Ok(include_caller) = std::env::var("LOG_CALLER") {
            config.include_caller = include_caller.to_lowercase() == "true";
        }

        config
    }

    /// Create configuration with custom settings
    pub fn with_level(level: LogLevel) -> Self {
        Self {
            level,
            ..Self::default()
        }
    }

    /// Enable file logging with specified path
    pub fn with_file_logging(mut self, path: String) -> Self {
        self.enable_file_logging = true;
        self.log_file_path = Some(path);
        self
    }

    /// Set output format
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    /// Add default context field
    pub fn with_default_context(mut self, key: String, value: serde_json::Value) -> Self {
        self.default_context.insert(key, value);
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.enable_file_logging && self.log_file_path.is_none() {
            return Err("Log file path is required when file logging is enabled".to_string());
        }

        if let Some(max_size) = self.max_file_size {
            if max_size == 0 {
                return Err("Max file size must be greater than 0".to_string());
            }
        }

        if let Some(max_files) = self.max_files {
            if max_files == 0 {
                return Err("Max files must be greater than 0".to_string());
            }
        }

        Ok(())
    }
}