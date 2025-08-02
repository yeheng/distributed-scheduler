use serde::{Deserialize, Serialize};

/// Redis Stream配置
///
/// 包含连接Redis服务器和配置消息队列行为所需的所有参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisStreamConfig {
    pub host: String,
    pub port: u16,
    pub database: i64,
    pub password: Option<String>,
    pub connection_timeout_seconds: u64,
    pub max_retry_attempts: u32,
    pub retry_delay_seconds: u64,
    pub consumer_group_prefix: String,
    pub consumer_id: String,
    // 连接池配置
    pub pool_min_idle: u32,
    pub pool_max_open: u32,
    pub pool_timeout_seconds: u64,
}

impl Default for RedisStreamConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            database: 0,
            password: None,
            connection_timeout_seconds: 30,
            max_retry_attempts: 3,
            retry_delay_seconds: 1,
            consumer_group_prefix: "scheduler".to_string(),
            consumer_id: "default_consumer".to_string(),
            pool_min_idle: 1,
            pool_max_open: 10,
            pool_timeout_seconds: 30,
        }
    }
}

impl RedisStreamConfig {
    /// 构建Redis连接URL
    pub fn build_connection_url(&self) -> String {
        if let Some(password) = &self.password {
            format!(
                "redis://:{}@{}:{}/{}",
                password, self.host, self.port, self.database
            )
        } else {
            format!("redis://{}:{}/{}", self.host, self.port, self.database)
        }
    }
}
