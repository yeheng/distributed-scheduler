use crate::validation_new::ConfigValidator;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(format!(
                "Invalid log level: {s}. Valid levels: trace, debug, info, warn, error"
            )),
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum OutputFormat {
    #[default]
    Json,
    Text,
    Pretty,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            "text" => Ok(OutputFormat::Text),
            "pretty" => Ok(OutputFormat::Pretty),
            _ => Err(format!(
                "Invalid output format: {s}. Valid formats: json, text, pretty"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl ConfigValidator for LogConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        if self.enable_file_logging && self.log_file_path.is_none() {
            return Err(crate::ConfigError::Validation(
                "Log file path is required when file logging is enabled".to_string(),
            ));
        }

        if let Some(max_size) = self.max_file_size {
            if max_size == 0 {
                return Err(crate::ConfigError::Validation(
                    "Max file size must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(max_files) = self.max_files {
            if max_files == 0 {
                return Err(crate::ConfigError::Validation(
                    "Max files must be greater than 0".to_string(),
                ));
            }
        }

        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.level, LogLevel::Info);
        assert_eq!(config.format, OutputFormat::Json);
        assert!(!config.enable_file_logging);
    }

    #[test]
    fn test_log_config_validation() {
        let config = LogConfig::default();
        assert!(config.validate().is_ok());

        // Test file logging without path
        let mut invalid_config = config.clone();
        invalid_config.enable_file_logging = true;
        invalid_config.log_file_path = None;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_log_level_from_str() {
        assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("DEBUG".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert!("invalid".parse::<LogLevel>().is_err());
    }

    #[test]
    fn test_log_config_from_env() {
        std::env::set_var("LOG_LEVEL", "debug");
        std::env::set_var("LOG_FORMAT", "text");
        std::env::set_var("LOG_TO_FILE", "true");
        std::env::set_var("LOG_FILE_PATH", "/tmp/test.log");

        let config = LogConfig::from_env();
        assert_eq!(config.level, LogLevel::Debug);
        assert_eq!(config.format, OutputFormat::Text);
        assert!(config.enable_file_logging);
        assert_eq!(config.log_file_path, Some("/tmp/test.log".to_string()));

        // Clean up
        std::env::remove_var("LOG_LEVEL");
        std::env::remove_var("LOG_FORMAT");
        std::env::remove_var("LOG_TO_FILE");
        std::env::remove_var("LOG_FILE_PATH");
    }
}
