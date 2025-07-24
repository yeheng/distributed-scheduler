use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, Environment, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::path::Path;

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

impl Config {
    /// 从配置文件和环境变量加载配置
    ///
    /// 加载顺序：
    /// 1. 默认配置
    /// 2. 配置文件 (TOML格式)
    /// 3. 环境变量覆盖 (前缀: SCHEDULER_)
    ///
    /// # Arguments
    ///
    /// * `config_path` - 配置文件路径，如果为None则使用默认路径
    ///
    /// # Returns
    ///
    /// 返回加载并验证后的配置
    pub fn load(config_path: Option<&str>) -> Result<Self> {
        let mut builder = ConfigBuilder::builder();

        // 1. 设置默认配置
        let default_config = Config::default();
        builder = builder.add_source(config::Config::try_from(&default_config)?);

        // 2. 加载配置文件
        if let Some(path) = config_path {
            if Path::new(path).exists() {
                builder = builder.add_source(File::new(path, FileFormat::Toml));
            } else {
                return Err(anyhow::anyhow!("配置文件不存在: {}", path));
            }
        } else {
            // 尝试加载默认配置文件
            let default_paths = [
                "config/scheduler.toml",
                "scheduler.toml",
                "/etc/scheduler/config.toml",
            ];

            for path in &default_paths {
                if Path::new(path).exists() {
                    builder = builder.add_source(File::new(*path, FileFormat::Toml));
                    break;
                }
            }
        }

        // 3. 环境变量覆盖 (前缀: SCHEDULER_)
        builder = builder.add_source(
            Environment::with_prefix("SCHEDULER")
                .separator("_")
                .try_parsing(true),
        );

        // 构建配置
        let config: Config = builder
            .build()
            .context("构建配置失败")?
            .try_deserialize()
            .context("反序列化配置失败")?;

        // 验证配置
        config.validate()?;

        Ok(config)
    }

    /// 从TOML字符串加载配置
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        let config: Config = toml::from_str(toml_str).context("解析TOML配置失败")?;

        config.validate()?;
        Ok(config)
    }

    /// 将配置序列化为TOML字符串
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("序列化配置为TOML失败")
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<()> {
        // 验证数据库配置
        self.database.validate().context("数据库配置验证失败")?;

        // 验证消息队列配置
        self.message_queue
            .validate()
            .context("消息队列配置验证失败")?;

        // 验证Dispatcher配置
        self.dispatcher
            .validate()
            .context("Dispatcher配置验证失败")?;

        // 验证Worker配置
        self.worker.validate().context("Worker配置验证失败")?;

        // 验证API配置
        self.api.validate().context("API配置验证失败")?;

        // 验证可观测性配置
        self.observability
            .validate()
            .context("可观测性配置验证失败")?;

        Ok(())
    }
}

impl DatabaseConfig {
    /// 验证数据库配置
    pub fn validate(&self) -> Result<()> {
        if self.url.is_empty() {
            return Err(anyhow::anyhow!("数据库URL不能为空"));
        }

        if !self.url.starts_with("postgresql://") && !self.url.starts_with("postgres://") {
            return Err(anyhow::anyhow!("数据库URL必须是PostgreSQL格式"));
        }

        if self.max_connections == 0 {
            return Err(anyhow::anyhow!("最大连接数必须大于0"));
        }

        if self.min_connections > self.max_connections {
            return Err(anyhow::anyhow!("最小连接数不能大于最大连接数"));
        }

        if self.connection_timeout_seconds == 0 {
            return Err(anyhow::anyhow!("连接超时时间必须大于0"));
        }

        Ok(())
    }
}

impl MessageQueueConfig {
    /// 验证消息队列配置
    pub fn validate(&self) -> Result<()> {
        if self.url.is_empty() {
            return Err(anyhow::anyhow!("消息队列URL不能为空"));
        }

        if !self.url.starts_with("amqp://") && !self.url.starts_with("amqps://") {
            return Err(anyhow::anyhow!("消息队列URL必须是AMQP格式"));
        }

        if self.task_queue.is_empty() {
            return Err(anyhow::anyhow!("任务队列名称不能为空"));
        }

        if self.status_queue.is_empty() {
            return Err(anyhow::anyhow!("状态队列名称不能为空"));
        }

        if self.heartbeat_queue.is_empty() {
            return Err(anyhow::anyhow!("心跳队列名称不能为空"));
        }

        if self.control_queue.is_empty() {
            return Err(anyhow::anyhow!("控制队列名称不能为空"));
        }

        if self.max_retries < 0 {
            return Err(anyhow::anyhow!("最大重试次数不能为负数"));
        }

        Ok(())
    }
}

impl DispatcherConfig {
    /// 验证Dispatcher配置
    pub fn validate(&self) -> Result<()> {
        if self.schedule_interval_seconds == 0 {
            return Err(anyhow::anyhow!("调度间隔必须大于0"));
        }

        if self.max_concurrent_dispatches == 0 {
            return Err(anyhow::anyhow!("最大并发调度数必须大于0"));
        }

        if self.worker_timeout_seconds <= 0 {
            return Err(anyhow::anyhow!("Worker超时时间必须大于0"));
        }

        let valid_strategies = ["round_robin", "load_based", "task_type_affinity"];
        if !valid_strategies.contains(&self.dispatch_strategy.as_str()) {
            return Err(anyhow::anyhow!(
                "无效的调度策略: {}，支持的策略: {:?}",
                self.dispatch_strategy,
                valid_strategies
            ));
        }

        Ok(())
    }
}

impl WorkerConfig {
    /// 验证Worker配置
    pub fn validate(&self) -> Result<()> {
        if self.worker_id.is_empty() {
            return Err(anyhow::anyhow!("Worker ID不能为空"));
        }

        if self.hostname.is_empty() {
            return Err(anyhow::anyhow!("主机名不能为空"));
        }

        if self.ip_address.is_empty() {
            return Err(anyhow::anyhow!("IP地址不能为空"));
        }

        // 简单的IP地址格式验证
        if !self
            .ip_address
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
        {
            return Err(anyhow::anyhow!("IP地址格式无效: {}", self.ip_address));
        }

        if self.max_concurrent_tasks <= 0 {
            return Err(anyhow::anyhow!("最大并发任务数必须大于0"));
        }

        if self.supported_task_types.is_empty() {
            return Err(anyhow::anyhow!("支持的任务类型不能为空"));
        }

        if self.heartbeat_interval_seconds == 0 {
            return Err(anyhow::anyhow!("心跳间隔必须大于0"));
        }

        if self.task_poll_interval_seconds == 0 {
            return Err(anyhow::anyhow!("任务轮询间隔必须大于0"));
        }

        Ok(())
    }
}

impl ApiConfig {
    /// 验证API配置
    pub fn validate(&self) -> Result<()> {
        if self.bind_address.is_empty() {
            return Err(anyhow::anyhow!("绑定地址不能为空"));
        }

        // 简单的地址格式验证
        if !self.bind_address.contains(':') {
            return Err(anyhow::anyhow!("绑定地址格式无效，应为 host:port"));
        }

        if self.request_timeout_seconds == 0 {
            return Err(anyhow::anyhow!("请求超时时间必须大于0"));
        }

        if self.max_request_size_mb == 0 {
            return Err(anyhow::anyhow!("最大请求大小必须大于0"));
        }

        Ok(())
    }
}

impl ObservabilityConfig {
    /// 验证可观测性配置
    pub fn validate(&self) -> Result<()> {
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.log_level.to_lowercase().as_str()) {
            return Err(anyhow::anyhow!(
                "无效的日志级别: {}，支持的级别: {:?}",
                self.log_level,
                valid_log_levels
            ));
        }

        if self.metrics_endpoint.is_empty() {
            return Err(anyhow::anyhow!("指标端点不能为空"));
        }

        if !self.metrics_endpoint.starts_with('/') {
            return Err(anyhow::anyhow!("指标端点必须以'/'开头"));
        }

        Ok(())
    }
}
