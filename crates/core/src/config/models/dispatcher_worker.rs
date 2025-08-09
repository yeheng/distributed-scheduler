use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatcherConfig {
    pub enabled: bool,
    pub schedule_interval_seconds: u64,
    pub max_concurrent_dispatches: usize,
    pub worker_timeout_seconds: i64,
    pub dispatch_strategy: String, // "round_robin", "load_based", "task_type_affinity"
}

impl DispatcherConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
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

impl WorkerConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.worker_id.is_empty() {
            return Err(anyhow::anyhow!("Worker ID不能为空"));
        }

        if self.hostname.is_empty() {
            return Err(anyhow::anyhow!("主机名不能为空"));
        }

        if self.ip_address.is_empty() {
            return Err(anyhow::anyhow!("IP地址不能为空"));
        }
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
