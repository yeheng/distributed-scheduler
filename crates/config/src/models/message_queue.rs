use crate::validation_new::{ConfigValidator, ValidationUtils};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageQueueType {
    Rabbitmq,
    RedisStream,
    InMemory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub database: i32,
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
            retry_delay_seconds: 5,
        }
    }
}

impl ConfigValidator for RedisConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        ValidationUtils::validate_not_empty(&self.host, "redis.host")?;
        ValidationUtils::validate_port(self.port)?;
        ValidationUtils::validate_timeout_seconds(self.connection_timeout_seconds)?;
        ValidationUtils::validate_timeout_seconds(self.retry_delay_seconds)?;

        if self.max_retry_attempts == 0 {
            return Err(crate::ConfigError::Validation(
                "redis.max_retry_attempts must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueConfig {
    pub r#type: MessageQueueType,
    pub url: String,
    pub redis: Option<RedisConfig>,
    pub task_queue: String,
    pub status_queue: String,
    pub heartbeat_queue: String,
    pub control_queue: String,
    pub max_retries: u32,
    pub retry_delay_seconds: u64,
    pub connection_timeout_seconds: u64,
}

impl Default for MessageQueueConfig {
    fn default() -> Self {
        Self {
            r#type: MessageQueueType::RedisStream,
            url: "redis://localhost:6379".to_string(),
            redis: Some(RedisConfig::default()),
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

impl MessageQueueConfig {
    pub fn in_memory_default() -> Self {
        Self {
            r#type: MessageQueueType::InMemory,
            url: "".to_string(), // 内存队列不需要URL
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status_updates".to_string(),
            heartbeat_queue: "heartbeats".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 1, // 内存队列重试更快
            connection_timeout_seconds: 1, // 内存队列不需要连接超时
        }
    }
}

impl ConfigValidator for MessageQueueConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        match self.r#type {
            MessageQueueType::Rabbitmq => {
                ValidationUtils::validate_not_empty(&self.url, "message_queue.url")?;

                if !self.url.starts_with("amqp://") && !self.url.starts_with("amqps://") {
                    return Err(crate::ConfigError::Validation(
                        "RabbitMQ URL must start with amqp:// or amqps://".to_string(),
                    ));
                }
            }
            MessageQueueType::RedisStream => {
                if self.redis.is_none()
                    && (self.url.is_empty()
                        || (!self.url.starts_with("redis://")
                            && !self.url.starts_with("rediss://")))
                {
                    return Err(crate::ConfigError::Validation(
                        "Redis Stream config requires either redis configuration or valid Redis URL".to_string(),
                    ));
                }

                if let Some(redis_config) = &self.redis {
                    redis_config.validate()?;
                }
            }
            MessageQueueType::InMemory => {
                // 内存队列不需要URL或其他外部配置验证
            }
        }

        ValidationUtils::validate_not_empty(&self.task_queue, "message_queue.task_queue")?;
        ValidationUtils::validate_not_empty(&self.status_queue, "message_queue.status_queue")?;
        ValidationUtils::validate_not_empty(
            &self.heartbeat_queue,
            "message_queue.heartbeat_queue",
        )?;
        ValidationUtils::validate_not_empty(&self.control_queue, "message_queue.control_queue")?;

        if self.max_retries == 0 {
            return Err(crate::ConfigError::Validation(
                "message_queue.max_retries must be greater than 0".to_string(),
            ));
        }

        ValidationUtils::validate_timeout_seconds(self.retry_delay_seconds)?;
        ValidationUtils::validate_timeout_seconds(self.connection_timeout_seconds)?;

        Ok(())
    }
}

impl MessageQueueConfig {
    pub fn parse_type_string(type_str: &str) -> crate::ConfigResult<MessageQueueType> {
        match type_str.to_lowercase().as_str() {
            "rabbitmq" => Ok(MessageQueueType::Rabbitmq),
            "redis_stream" => Ok(MessageQueueType::RedisStream),
            "in_memory" => Ok(MessageQueueType::InMemory),
            _ => Err(crate::ConfigError::Validation(format!(
                "Unsupported message queue type: {type_str}, supported types: rabbitmq, redis_stream, in_memory"
            ))),
        }
    }

    pub fn get_type_string(&self) -> &'static str {
        match self.r#type {
            MessageQueueType::Rabbitmq => "rabbitmq",
            MessageQueueType::RedisStream => "redis_stream",
            MessageQueueType::InMemory => "in_memory",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_queue_config_default() {
        let config = MessageQueueConfig::default();
        assert_eq!(config.r#type, MessageQueueType::RedisStream);
        assert_eq!(config.task_queue, "tasks");
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_message_queue_config_validation() {
        let config = MessageQueueConfig::default();
        assert!(config.validate().is_ok());

        // Test RabbitMQ config
        let rabbitmq_config = MessageQueueConfig {
            r#type: MessageQueueType::Rabbitmq,
            url: "amqp://localhost:5672".to_string(),
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 5,
            connection_timeout_seconds: 30,
        };
        assert!(rabbitmq_config.validate().is_ok());

        // Test invalid RabbitMQ URL
        let mut invalid_config = rabbitmq_config.clone();
        invalid_config.url = "invalid://localhost:5672".to_string();
        assert!(invalid_config.validate().is_err());

        // Test invalid Redis Stream config
        let mut invalid_config = config.clone();
        invalid_config.redis = None;
        invalid_config.url = "".to_string();
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_redis_config_validation() {
        let config = RedisConfig::default();
        assert!(config.validate().is_ok());

        // Test invalid host
        let mut invalid_config = config.clone();
        invalid_config.host = "".to_string();
        assert!(invalid_config.validate().is_err());

        // Test invalid port
        let mut invalid_config = config.clone();
        invalid_config.port = 0;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_message_queue_type_parsing() {
        assert_eq!(
            MessageQueueConfig::parse_type_string("rabbitmq").unwrap(),
            MessageQueueType::Rabbitmq
        );
        assert_eq!(
            MessageQueueConfig::parse_type_string("redis_stream").unwrap(),
            MessageQueueType::RedisStream
        );
        assert_eq!(
            MessageQueueConfig::parse_type_string("in_memory").unwrap(),
            MessageQueueType::InMemory
        );
        assert!(MessageQueueConfig::parse_type_string("invalid").is_err());
    }

    #[test]
    fn test_in_memory_default_config() {
        let config = MessageQueueConfig::in_memory_default();
        assert_eq!(config.r#type, MessageQueueType::InMemory);
        assert_eq!(config.url, "");
        assert!(config.redis.is_none());
        assert_eq!(config.task_queue, "tasks");
        assert_eq!(config.status_queue, "status_updates");
        assert_eq!(config.heartbeat_queue, "heartbeats");
        assert_eq!(config.control_queue, "control");
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_seconds, 1);
        assert_eq!(config.connection_timeout_seconds, 1);
        
        // Validate that the in-memory config is valid
        assert!(config.validate().is_ok());
    }
}
