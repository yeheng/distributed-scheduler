use crate::logging::log_level::LogLevel;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogConfig {
    pub level: LogLevel,
    pub format: OutputFormat,
    pub enable_file_logging: bool,
    pub log_file_path: Option<String>,
    pub max_file_size: Option<u64>,
    pub max_files: Option<u32>,
    pub include_timestamps: bool,
    pub include_caller: bool,
    pub default_context: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum OutputFormat {
    Json,
    Text,
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
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(level_str) = std::env::var("LOG_LEVEL") {
            if let Ok(level) = level_str.parse::<LogLevel>() {
                config.level = level;
            }
        }
        if let Ok(format_str) = std::env::var("LOG_FORMAT") {
            config.format = match format_str.to_lowercase().as_str() {
                "json" => OutputFormat::Json,
                "text" => OutputFormat::Text,
                "pretty" => OutputFormat::Pretty,
                _ => OutputFormat::Json,
            };
        }
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
        if let Ok(include_timestamps) = std::env::var("LOG_TIMESTAMPS") {
            config.include_timestamps = include_timestamps.to_lowercase() == "true";
        }

        if let Ok(include_caller) = std::env::var("LOG_CALLER") {
            config.include_caller = include_caller.to_lowercase() == "true";
        }

        config
    }
    pub fn with_level(level: LogLevel) -> Self {
        Self {
            level,
            ..Self::default()
        }
    }
    pub fn with_file_logging(mut self, path: String) -> Self {
        self.enable_file_logging = true;
        self.log_file_path = Some(path);
        self
    }
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }
    pub fn with_default_context(mut self, key: String, value: serde_json::Value) -> Self {
        self.default_context.insert(key, value);
        self
    }
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
