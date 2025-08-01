use serde::{Deserialize, Serialize};

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub cors_enabled: bool,
    pub cors_origins: Vec<String>,
    pub request_timeout_seconds: u64,
    pub max_request_size_mb: usize,
}

impl ApiConfig {
    /// Validate API configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.bind_address.is_empty() {
            return Err(anyhow::anyhow!("绑定地址不能为空"));
        }

        // Simple address format validation
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

/// Observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub tracing_enabled: bool,
    pub metrics_enabled: bool,
    pub metrics_endpoint: String,
    pub log_level: String,
    pub jaeger_endpoint: Option<String>,
}

impl ObservabilityConfig {
    /// Validate observability configuration
    pub fn validate(&self) -> anyhow::Result<()> {
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
