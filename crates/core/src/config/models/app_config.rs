use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, Environment, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{
    api_observability::{ApiConfig, ObservabilityConfig},
    database::DatabaseConfig,
    dispatcher_worker::{DispatcherConfig, WorkerConfig},
    message_queue::MessageQueueConfig,
};

/// System configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub message_queue: MessageQueueConfig,
    pub dispatcher: DispatcherConfig,
    pub worker: WorkerConfig,
    pub api: ApiConfig,
    pub observability: ObservabilityConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                url: "postgresql://localhost/scheduler".to_string(),
                max_connections: 10,
                min_connections: 1,
                connection_timeout_seconds: 30,
                idle_timeout_seconds: 600,
            },
            message_queue: MessageQueueConfig::default(),
            dispatcher: DispatcherConfig {
                enabled: true,
                schedule_interval_seconds: 10,
                max_concurrent_dispatches: 100,
                worker_timeout_seconds: 90,
                dispatch_strategy: "round_robin".to_string(),
            },
            worker: WorkerConfig {
                enabled: false,
                worker_id: "worker-001".to_string(),
                hostname: "localhost".to_string(),
                ip_address: "127.0.0.1".to_string(),
                max_concurrent_tasks: 5,
                supported_task_types: vec!["shell".to_string(), "http".to_string()],
                heartbeat_interval_seconds: 30,
                task_poll_interval_seconds: 5,
            },
            api: ApiConfig {
                enabled: true,
                bind_address: "0.0.0.0:8080".to_string(),
                cors_enabled: true,
                cors_origins: vec!["*".to_string()],
                request_timeout_seconds: 30,
                max_request_size_mb: 10,
            },
            observability: ObservabilityConfig {
                tracing_enabled: true,
                metrics_enabled: true,
                metrics_endpoint: "/metrics".to_string(),
                log_level: "info".to_string(),
                jaeger_endpoint: None,
            },
        }
    }
}

impl AppConfig {
    /// Load configuration from config file and environment variables
    ///
    /// Load order:
    /// 1. Default configuration
    /// 2. Config file (TOML format)
    /// 3. Environment variable overrides (prefix: SCHEDULER_)
    ///
    /// # Arguments
    ///
    /// * `config_path` - Config file path, if None use default paths
    ///
    /// # Returns
    ///
    /// Returns loaded and validated configuration
    pub fn load(config_path: Option<&str>) -> Result<Self> {
        let mut builder = ConfigBuilder::builder();

        // 1. Set default configuration
        let default_config = AppConfig::default();
        builder = builder.add_source(config::Config::try_from(&default_config)?);

        // 2. Load config file
        if let Some(path) = config_path {
            if Path::new(path).exists() {
                builder = builder.add_source(File::new(path, FileFormat::Toml));
            } else {
                return Err(anyhow::anyhow!("配置文件不存在: {}", path));
            }
        } else {
            // Try to load default config files
            let default_paths = [
                "config/scheduler.toml",
                "scheduler.toml",
                "/etc/scheduler/config.toml",
            ];

            for path in &default_paths {
                if Path::new(path).exists() {
                    builder = builder.add_source(File::new(path, FileFormat::Toml));
                    break;
                }
            }
        }

        // 3. Environment variable overrides (prefix: SCHEDULER_)
        builder = builder.add_source(
            Environment::with_prefix("SCHEDULER")
                .separator("_")
                .try_parsing(true),
        );

        // Build configuration
        let config: AppConfig = builder
            .build()
            .context("构建配置失败")?
            .try_deserialize()
            .context("反序列化配置失败")?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Load configuration from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        let config: AppConfig = toml::from_str(toml_str).context("解析TOML配置失败")?;

        config.validate()?;
        Ok(config)
    }

    /// Serialize configuration to TOML string
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("序列化配置为TOML失败")
    }

    /// Validate configuration effectiveness
    pub fn validate(&self) -> Result<()> {
        // Validate database configuration
        self.database.validate().context("数据库配置验证失败")?;

        // Validate message queue configuration
        self.message_queue
            .validate()
            .context("消息队列配置验证失败")?;

        // Validate dispatcher configuration
        self.dispatcher
            .validate()
            .context("Dispatcher配置验证失败")?;

        // Validate worker configuration
        self.worker.validate().context("Worker配置验证失败")?;

        // Validate API configuration
        self.api.validate().context("API配置验证失败")?;

        // Validate observability configuration
        self.observability
            .validate()
            .context("可观测性配置验证失败")?;

        Ok(())
    }
}
