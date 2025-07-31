use serde::{Deserialize, Serialize};

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

impl DatabaseConfig {
    /// Validate database configuration
    pub fn validate(&self) -> anyhow::Result<()> {
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