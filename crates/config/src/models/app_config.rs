use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, Environment, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{
    api_observability::{ApiConfig, ObservabilityConfig},
    database::DatabaseConfig,
    dispatcher_worker::{DispatcherConfig, WorkerConfig},
    executor::ExecutorConfig,
    message_queue::MessageQueueConfig,
    resilience::ResilienceConfig,
};
use crate::validation_new::ConfigValidator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub message_queue: MessageQueueConfig,
    pub dispatcher: DispatcherConfig,
    pub worker: WorkerConfig,
    pub api: ApiConfig,
    pub observability: ObservabilityConfig,
    pub executor: ExecutorConfig,
    pub resilience: ResilienceConfig,
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
                auth: super::api_observability::AuthConfig::default(),
                rate_limiting: super::api_observability::RateLimitingConfig::default(),
            },
            observability: ObservabilityConfig {
                tracing_enabled: true,
                metrics_enabled: true,
                metrics_endpoint: "/metrics".to_string(),
                log_level: "info".to_string(),
                jaeger_endpoint: None,
            },
            executor: ExecutorConfig::default(),
            resilience: ResilienceConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load(config_path: Option<&str>) -> Result<Self> {
        let mut builder = ConfigBuilder::builder();

        if let Some(path) = config_path {
            if Path::new(path).exists() {
                builder = builder.add_source(File::new(path, FileFormat::Toml));
            } else {
                return Err(anyhow::anyhow!("配置文件不存在: {}", path));
            }
        } else {
            let default_paths = [
                "config/scheduler.toml",
                "scheduler.toml",
                "/etc/scheduler/config.toml",
            ];

            let mut config_file_found = false;
            for path in &default_paths {
                if Path::new(path).exists() {
                    builder = builder.add_source(File::new(path, FileFormat::Toml));
                    config_file_found = true;
                    break;
                }
            }

            if !config_file_found {
                builder = builder
                    .set_default("database.url", "postgresql://localhost/scheduler")?
                    .set_default("database.max_connections", 10)?
                    .set_default("database.min_connections", 1)?
                    .set_default("database.connection_timeout_seconds", 30)?
                    .set_default("database.idle_timeout_seconds", 600)?
                    .set_default("message_queue.url", "redis://localhost:6379")?
                    .set_default("message_queue.task_queue", "tasks")?
                    .set_default("message_queue.status_queue", "status_updates")?
                    .set_default("message_queue.heartbeat_queue", "heartbeats")?
                    .set_default("message_queue.control_queue", "control")?
                    .set_default("message_queue.max_retries", 3)?
                    .set_default("message_queue.retry_delay_seconds", 5)?
                    .set_default("message_queue.connection_timeout_seconds", 30)?
                    .set_default("dispatcher.enabled", true)?
                    .set_default("dispatcher.schedule_interval_seconds", 10)?
                    .set_default("dispatcher.max_concurrent_dispatches", 100)?
                    .set_default("dispatcher.worker_timeout_seconds", 90)?
                    .set_default("dispatcher.dispatch_strategy", "round_robin")?
                    .set_default("worker.enabled", false)?
                    .set_default("worker.worker_id", "worker-001")?
                    .set_default("worker.hostname", "localhost")?
                    .set_default("worker.ip_address", "127.0.0.1")?
                    .set_default("worker.max_concurrent_tasks", 5)?
                    .set_default("worker.heartbeat_interval_seconds", 30)?
                    .set_default("worker.task_poll_interval_seconds", 5)?
                    .set_default("worker.supported_task_types", vec!["shell", "http"])?
                    .set_default("api.enabled", true)?
                    .set_default("api.bind_address", "0.0.0.0:8080")?
                    .set_default("api.cors_enabled", true)?
                    .set_default("api.cors_origins", vec!["*"])?
                    .set_default("api.request_timeout_seconds", 30)?
                    .set_default("api.max_request_size_mb", 10)?
                    .set_default("api.auth.enabled", false)?
                    .set_default(
                        "api.auth.jwt_secret",
                        "your-secret-key-change-this-in-production",
                    )?
                    .set_default("api.auth.jwt_expiration_hours", 24)?
                    .set_default("observability.tracing_enabled", true)?
                    .set_default("observability.metrics_enabled", true)?
                    .set_default("observability.metrics_endpoint", "/metrics")?
                    .set_default("observability.log_level", "info")?
                    .set_default("executor.enabled", true)?
                    .set_default("executor.default_executor", "shell")?;
            }
        }

        builder = builder.add_source(
            Environment::with_prefix("SCHEDULER")
                .separator("_")
                .try_parsing(true),
        );

        let config: AppConfig = builder
            .build()
            .context("构建配置失败")?
            .try_deserialize()
            .context("反序列化配置失败")?;

        config.validate()?;

        Ok(config)
    }

    pub fn from_toml(toml_str: &str) -> Result<Self> {
        let config: AppConfig = toml::from_str(toml_str).context("解析TOML配置失败")?;
        config.validate()?;
        Ok(config)
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("序列化配置为TOML失败")
    }
}

impl ConfigValidator for AppConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        self.database.validate()?;
        self.message_queue.validate()?;
        self.dispatcher.validate()?;
        self.worker.validate()?;
        self.api.validate()?;
        self.observability.validate()?;
        self.executor.validate()?;
        self.resilience.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.dispatcher.schedule_interval_seconds, 10);
        assert_eq!(config.worker.worker_id, "worker-001");
        assert_eq!(config.api.bind_address, "0.0.0.0:8080");
    }

    #[test]
    fn test_app_config_validation() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_app_config_serialization() {
        let config = AppConfig::default();
        let serialized = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: AppConfig =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(
            config.database.max_connections,
            deserialized.database.max_connections
        );
        assert_eq!(
            config.dispatcher.schedule_interval_seconds,
            deserialized.dispatcher.schedule_interval_seconds
        );
    }

    #[test]
    fn test_app_config_from_toml() {
        let toml_str = r#"
[database]
url = "postgresql://localhost/test"
max_connections = 20
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[dispatcher]
enabled = true
schedule_interval_seconds = 5
max_concurrent_dispatches = 100
worker_timeout_seconds = 90
dispatch_strategy = "round_robin"

[worker]
enabled = true
worker_id = "test-worker"
hostname = "localhost"
ip_address = "127.0.0.1"
max_concurrent_tasks = 5
supported_task_types = ["shell", "http"]
heartbeat_interval_seconds = 30
task_poll_interval_seconds = 5

[api]
enabled = true
bind_address = "0.0.0.0:9000"
cors_enabled = true
cors_origins = ["*"]
request_timeout_seconds = 30
max_request_size_mb = 10

[api.auth]
enabled = false
jwt_secret = "test-secret"
jwt_expiration_hours = 24
api_keys = {}

[api.rate_limiting]
enabled = false
max_requests_per_minute = 60
refill_rate_per_second = 1.0
burst_size = 10
endpoint_limits = {}

[observability]
tracing_enabled = true
metrics_enabled = true
metrics_endpoint = "/metrics"
log_level = "info"

[message_queue]
type = "RedisStream"
url = "redis://localhost:6379"
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 5
connection_timeout_seconds = 30

[resilience]
database_circuit_breaker = { failure_threshold = 5, recovery_timeout = 60, success_threshold = 3, call_timeout = 30, backoff_multiplier = 2.0, max_recovery_timeout = 300 }
message_queue_circuit_breaker = { failure_threshold = 3, recovery_timeout = 30, success_threshold = 2, call_timeout = 10, backoff_multiplier = 1.5, max_recovery_timeout = 180 }
external_service_circuit_breaker = { failure_threshold = 3, recovery_timeout = 45, success_threshold = 3, call_timeout = 15, backoff_multiplier = 2.0, max_recovery_timeout = 240 }
[executor]
enabled = true
default_executor = "shell"
[executor.executors.shell]
executor_type = "shell"
supported_task_types = ["shell"]
priority = 100
max_concurrent_tasks = 10
timeout_seconds = 300

[executor.executors.http]
executor_type = "http"
supported_task_types = ["http"]
priority = 90
max_concurrent_tasks = 20
timeout_seconds = 60

[executor.executor_factory]
auto_discovery = false
discovery_path = "./executors"
dynamic_loading = false
plugin_directories = ["./plugins"]
validation_enabled = true
"#;

        let config = AppConfig::from_toml(toml_str).expect("Failed to parse TOML");
        assert_eq!(config.database.max_connections, 20);
        assert_eq!(config.dispatcher.schedule_interval_seconds, 5);
        assert_eq!(config.worker.worker_id, "test-worker");
        assert_eq!(config.api.bind_address, "0.0.0.0:9000");
    }
}
