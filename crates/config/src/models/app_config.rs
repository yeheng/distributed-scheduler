use anyhow::{Context, Result};
use figment::{
    providers::{Env, Format, Json, Toml},
    Figment
};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::defaults::environment_overrides;

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
    /// 加载配置文件，支持多层配置合并
    /// 
    /// 配置合并顺序（后面的覆盖前面的）：
    /// 1. 默认配置
    /// 2. 环境特定配置（通过环境变量SCHEDULER_ENV指定）
    /// 3. 基础配置文件（base.toml）
    /// 4. 指定的配置文件
    /// 5. 环境变量（前缀 SCHEDULER_）
    pub fn load(config_path: Option<&str>) -> Result<Self> {
        use figment::providers::Serialized;
        
        let mut figment = Figment::new();
        
        // 1. 加载默认配置
        figment = figment.merge(Serialized::defaults(AppConfig::default()));
        
        // 2. 根据环境变量加载环境特定配置
        if let Ok(env) = std::env::var("SCHEDULER_ENV") {
            if let Some(env_overrides) = environment_overrides(&env) {
                figment = figment.merge(("", env_overrides));
            }
        }
        
        // 3. 尝试加载基础配置文件
        let base_config_paths = [
            "config/base.toml",
            "base.toml",
            "/etc/scheduler/base.toml",
        ];
        
        for path in &base_config_paths {
            if Path::new(path).exists() {
                figment = figment.merge(Toml::file(path));
                break;
            }
        }
        
        // 4. 加载指定的配置文件或默认配置文件
        if let Some(path) = config_path {
            if Path::new(path).exists() {
                // 根据文件扩展名选择合适的提供器
                if path.ends_with(".json") {
                    figment = figment.merge(Json::file(path));
                } else {
                    // 默认为TOML
                    figment = figment.merge(Toml::file(path));
                }
            } else {
                return Err(anyhow::anyhow!("配置文件不存在: {}", path));
            }
        } else {
            // 尝试默认配置文件路径
            let default_paths = [
                "config/scheduler.toml",
                "scheduler.toml",
                "/etc/scheduler/config.toml",
            ];

            for path in &default_paths {
                if Path::new(path).exists() {
                    figment = figment.merge(Toml::file(path));
                    break;
                }
            }
        }

        // 5. 最后合并环境变量（优先级最高）
        figment = figment.merge(
            Env::prefixed("SCHEDULER_")
                .split("__")
                .ignore(&["SCHEDULER_ENV"]) // 忽略环境标识变量
        );

        // 提取并验证配置
        let config: AppConfig = figment
            .extract()
            .context("配置提取失败")?;

        config.validate()?;

        Ok(config)
    }

    /// 从环境加载配置（优先级：指定环境 > development）
    pub fn from_env(env: Option<&str>) -> Result<Self> {
        let env_name = env.unwrap_or("development");
        std::env::set_var("SCHEDULER_ENV", env_name);
        
        Self::load(None)
    }
    
    /// 从指定文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::load(Some(path.as_ref().to_str().unwrap()))
    }
    
    /// 加载默认配置（不加载任何文件，仅使用默认值和环境变量）
    pub fn default_with_env() -> Result<Self> {
        use figment::providers::Serialized;
        
        let figment = Figment::new()
            .merge(Serialized::defaults(AppConfig::default()))
            .merge(
                Env::prefixed("SCHEDULER_")
                    .split("__")
                    .ignore(&["SCHEDULER_ENV"])
            );

        let config: AppConfig = figment
            .extract()
            .context("配置提取失败")?;

        config.validate()?;
        Ok(config)
    }
    
    /// 获取当前配置的环境名称
    pub fn environment(&self) -> String {
        std::env::var("SCHEDULER_ENV").unwrap_or_else(|_| "development".to_string())
    }
    
    /// 检查是否为生产环境
    pub fn is_production(&self) -> bool {
        self.environment() == "production"
    }
    
    /// 检查是否为开发环境
    pub fn is_development(&self) -> bool {
        matches!(self.environment().as_str(), "development" | "dev")
    }
    
    /// 检查是否为测试环境
    pub fn is_test(&self) -> bool {
        matches!(self.environment().as_str(), "test" | "testing")
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
