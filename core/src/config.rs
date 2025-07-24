use serde::{Deserialize, Serialize};

/// 系统配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub message_queue: MessageQueueConfig,
    pub dispatcher: DispatcherConfig,
    pub worker: WorkerConfig,
    pub api: ApiConfig,
    pub observability: ObservabilityConfig,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

/// 消息队列配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueConfig {
    pub url: String,
    pub task_queue: String,
    pub status_queue: String,
    pub heartbeat_queue: String,
    pub control_queue: String,
    pub max_retries: i32,
    pub retry_delay_seconds: u64,
    pub connection_timeout_seconds: u64,
}

/// Dispatcher配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatcherConfig {
    pub enabled: bool,
    pub schedule_interval_seconds: u64,
    pub max_concurrent_dispatches: usize,
    pub worker_timeout_seconds: i64,
    pub dispatch_strategy: String, // "round_robin", "load_based", "task_type_affinity"
}

/// Worker配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub enabled: bool,
    pub worker_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub max_concurrent_tasks: i32,
    pub supported_task_types: Vec<String>,
    pub heartbeat_interval_seconds: u64,
    pub task_poll_interval_seconds: u64,
}

/// API配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub cors_enabled: bool,
    pub cors_origins: Vec<String>,
    pub request_timeout_seconds: u64,
    pub max_request_size_mb: usize,
}

/// 可观测性配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub tracing_enabled: bool,
    pub metrics_enabled: bool,
    pub metrics_endpoint: String,
    pub log_level: String,
    pub jaeger_endpoint: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                url: "postgresql://localhost/scheduler".to_string(),
                max_connections: 10,
                min_connections: 1,
                connection_timeout_seconds: 30,
                idle_timeout_seconds: 600,
            },
            message_queue: MessageQueueConfig {
                url: "amqp://localhost:5672".to_string(),
                task_queue: "tasks".to_string(),
                status_queue: "status_updates".to_string(),
                heartbeat_queue: "heartbeats".to_string(),
                control_queue: "control".to_string(),
                max_retries: 3,
                retry_delay_seconds: 60,
                connection_timeout_seconds: 30,
            },
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