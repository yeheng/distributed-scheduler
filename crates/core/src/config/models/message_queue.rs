use serde::{Deserialize, Serialize};

/// Message queue type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum MessageQueueType {
    Rabbitmq,
    #[default]
    RedisStream,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub database: i64,
    pub password: Option<String>,
    pub connection_timeout_seconds: u64,
    pub max_retry_attempts: u32,
    pub retry_delay_seconds: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            database: 0,
            password: None,
            connection_timeout_seconds: 30,
            max_retry_attempts: 3,
            retry_delay_seconds: 1,
        }
    }
}

impl RedisConfig {
    /// Validate Redis configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.host.is_empty() {
            return Err(anyhow::anyhow!("Redis主机地址不能为空"));
        }

        if self.port == 0 {
            return Err(anyhow::anyhow!("Redis端口必须大于0"));
        }

        if self.database < 0 {
            return Err(anyhow::anyhow!("Redis数据库索引不能为负数"));
        }

        if self.connection_timeout_seconds == 0 {
            return Err(anyhow::anyhow!("Redis连接超时时间必须大于0"));
        }

        if self.max_retry_attempts == 0 {
            return Err(anyhow::anyhow!("Redis最大重试次数必须大于0"));
        }

        if self.retry_delay_seconds == 0 {
            return Err(anyhow::anyhow!("Redis重试延迟时间必须大于0"));
        }

        Ok(())
    }

    /// Build Redis connection URL
    pub fn build_url(&self) -> String {
        let auth = if let Some(password) = &self.password {
            format!(":{password}@")
        } else {
            String::new()
        };
        format!(
            "redis://{}{}:{}/{}",
            auth, self.host, self.port, self.database
        )
    }

    /// Check if password is configured
    pub fn has_password(&self) -> bool {
        self.password.is_some()
    }
}

/// Message queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueConfig {
    #[serde(rename = "type", default)]
    pub r#type: MessageQueueType,
    pub url: String,
    pub redis: Option<RedisConfig>,
    pub task_queue: String,
    pub status_queue: String,
    pub heartbeat_queue: String,
    pub control_queue: String,
    pub max_retries: i32,
    pub retry_delay_seconds: u64,
    pub connection_timeout_seconds: u64,
}

impl MessageQueueConfig {
    /// Validate message queue configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate queue names
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

        // Validate type-specific configuration
        match self.r#type {
            MessageQueueType::Rabbitmq => {
                self.validate_rabbitmq_config()?;
            }
            MessageQueueType::RedisStream => {
                self.validate_redis_stream_config()?;
            }
        }

        Ok(())
    }

    /// Validate RabbitMQ configuration
    fn validate_rabbitmq_config(&self) -> anyhow::Result<()> {
        if self.url.is_empty() {
            return Err(anyhow::anyhow!("RabbitMQ URL不能为空"));
        }

        if !self.url.starts_with("amqp://") && !self.url.starts_with("amqps://") {
            return Err(anyhow::anyhow!("RabbitMQ URL必须是AMQP格式"));
        }

        Ok(())
    }

    /// Validate Redis Stream configuration
    fn validate_redis_stream_config(&self) -> anyhow::Result<()> {
        // For Redis Stream, URL can be empty (use redis config) or redis:// format
        if !self.url.is_empty()
            && !self.url.starts_with("redis://")
            && !self.url.starts_with("rediss://")
        {
            return Err(anyhow::anyhow!("Redis URL必须是redis://或rediss://格式"));
        }

        // Validate Redis-specific configuration
        if let Some(redis_config) = &self.redis {
            redis_config.validate()?;
        } else if self.url.is_empty() {
            return Err(anyhow::anyhow!(
                "使用Redis Stream时，必须提供URL或redis配置"
            ));
        }

        Ok(())
    }

    /// Check if using RabbitMQ
    pub fn is_rabbitmq(&self) -> bool {
        self.r#type == MessageQueueType::Rabbitmq
    }

    /// Check if using Redis Stream
    pub fn is_redis_stream(&self) -> bool {
        self.r#type == MessageQueueType::RedisStream
    }

    /// Get Redis connection URL (if configured)
    pub fn get_redis_url(&self) -> Option<String> {
        if self.is_redis_stream() {
            if !self.url.is_empty() {
                Some(self.url.clone())
            } else {
                self.redis
                    .as_ref()
                    .map(|redis_config| redis_config.build_url())
            }
        } else {
            None
        }
    }
}

impl Default for MessageQueueConfig {
    fn default() -> Self {
        Self {
            r#type: MessageQueueType::default(),
            url: "redis://localhost:6379".to_string(),
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status_updates".to_string(),
            heartbeat_queue: "heartbeats".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 5,
            connection_timeout_seconds: 30,
        }
    }
}
