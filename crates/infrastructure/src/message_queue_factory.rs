use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info};

use scheduler_config::models::{MessageQueueConfig, MessageQueueType};
use scheduler_errors::SchedulerError;
use scheduler_foundation::{traits::MessageQueue, SchedulerResult};

use crate::{redis_stream::RedisStreamConfig, RabbitMQMessageQueue, RedisStreamMessageQueue};

pub struct MessageQueueFactory;

impl MessageQueueFactory {
    pub async fn create(
        config: &MessageQueueConfig,
    ) -> SchedulerResult<Arc<dyn MessageQueue + Send + Sync>> {
        debug!("Creating message queue with type: {:?}", config.r#type);

        match config.r#type {
            MessageQueueType::Rabbitmq => {
                info!("Initializing RabbitMQ message queue");
                let rabbitmq = RabbitMQMessageQueue::new(config.clone()).await?;
                Ok(Arc::new(rabbitmq))
            }
            MessageQueueType::RedisStream => {
                info!("Initializing Redis Stream message queue");
                let redis_config = Self::build_redis_config(config)?;
                let redis_stream = RedisStreamMessageQueue::new(redis_config).await?;
                Ok(Arc::new(redis_stream))
            }
        }
    }
    fn build_redis_config(config: &MessageQueueConfig) -> SchedulerResult<RedisStreamConfig> {
        if let Some(redis_config) = &config.redis {
            return Ok(RedisStreamConfig {
                host: redis_config.host.clone(),
                port: redis_config.port,
                database: redis_config.database as i64,
                password: redis_config.password.clone(),
                connection_timeout_seconds: redis_config.connection_timeout_seconds,
                max_retry_attempts: redis_config.max_retry_attempts,
                retry_delay_seconds: redis_config.retry_delay_seconds,
                consumer_group_prefix: "scheduler".to_string(),
                consumer_id: format!("consumer_{}", &uuid::Uuid::new_v4().to_string()[..8]),
                pool_min_idle: 1,
                pool_max_open: 10,
                pool_timeout_seconds: 30,
            });
        }
        if !config.url.is_empty()
            && (config.url.starts_with("redis://") || config.url.starts_with("rediss://"))
        {
            Self::parse_redis_url(&config.url, config)
        } else {
            Err(SchedulerError::Configuration(
                "Redis Stream配置缺失：需要提供redis配置段或有效的Redis URL".to_string(),
            ))
        }
    }
    pub fn parse_redis_url(
        url: &str,
        config: &MessageQueueConfig,
    ) -> SchedulerResult<RedisStreamConfig> {
        let url = url::Url::parse(url)
            .map_err(|e| SchedulerError::Configuration(format!("无效的Redis URL: {e}")))?;

        let host = url.host_str().unwrap_or("127.0.0.1").to_string();
        let port = url.port().unwrap_or(6379);
        let database = if url.path().len() > 1 {
            url.path()[1..].parse().unwrap_or(0)
        } else {
            0
        };
        let password = if !url.password().unwrap_or("").is_empty() {
            Some(url.password().unwrap().to_string())
        } else {
            None
        };

        Ok(RedisStreamConfig {
            host,
            port,
            database,
            password,
            connection_timeout_seconds: config.connection_timeout_seconds,
            max_retry_attempts: config.max_retries,
            retry_delay_seconds: config.retry_delay_seconds,
            consumer_group_prefix: "scheduler".to_string(),
            consumer_id: format!("consumer_{}", &uuid::Uuid::new_v4().to_string()[..8]),
            pool_min_idle: 1,
            pool_max_open: 10,
            pool_timeout_seconds: 30,
        })
    }
    pub fn validate_config(config: &MessageQueueConfig) -> SchedulerResult<()> {
        match config.r#type {
            MessageQueueType::Rabbitmq => {
                if config.url.is_empty() {
                    return Err(SchedulerError::Configuration(
                        "RabbitMQ配置缺失：需要提供有效的AMQP URL".to_string(),
                    ));
                }
                if !config.url.starts_with("amqp://") && !config.url.starts_with("amqps://") {
                    return Err(SchedulerError::Configuration(
                        "RabbitMQ URL必须以amqp://或amqps://开头".to_string(),
                    ));
                }
            }
            MessageQueueType::RedisStream => {
                if config.redis.is_none()
                    && (config.url.is_empty()
                        || (!config.url.starts_with("redis://")
                            && !config.url.starts_with("rediss://")))
                {
                    return Err(SchedulerError::Configuration(
                        "Redis Stream配置缺失：需要提供redis配置段或有效的Redis URL".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
    pub fn get_type_string(queue_type: &MessageQueueType) -> &'static str {
        match queue_type {
            MessageQueueType::Rabbitmq => "rabbitmq",
            MessageQueueType::RedisStream => "redis_stream",
        }
    }
    pub fn parse_type_string(type_str: &str) -> SchedulerResult<MessageQueueType> {
        match type_str.to_lowercase().as_str() {
            "rabbitmq" => Ok(MessageQueueType::Rabbitmq),
            "redis_stream" => Ok(MessageQueueType::RedisStream),
            _ => Err(SchedulerError::Configuration(format!(
                "不支持的消息队列类型: {type_str}，支持的类型: rabbitmq, redis_stream"
            ))),
        }
    }
}

pub struct MessageQueueManager {
    current_queue: Arc<dyn MessageQueue + Send + Sync>,
    current_config: MessageQueueConfig,
}

impl MessageQueueManager {
    pub async fn new(config: MessageQueueConfig) -> SchedulerResult<Self> {
        MessageQueueFactory::validate_config(&config)?;
        let queue = MessageQueueFactory::create(&config).await?;

        Ok(Self {
            current_queue: queue,
            current_config: config,
        })
    }
    pub fn get_queue(&self) -> Arc<dyn MessageQueue + Send + Sync> {
        self.current_queue.clone()
    }
    pub fn get_config(&self) -> &MessageQueueConfig {
        &self.current_config
    }
    pub async fn switch_to(&mut self, new_config: MessageQueueConfig) -> SchedulerResult<()> {
        info!(
            "Switching message queue from {:?} to {:?}",
            self.current_config.r#type, new_config.r#type
        );
        MessageQueueFactory::validate_config(&new_config)?;
        let new_queue = MessageQueueFactory::create(&new_config).await?;
        self.current_queue = new_queue;
        self.current_config = new_config;

        info!("Successfully switched to new message queue configuration");
        Ok(())
    }
    pub fn is_rabbitmq(&self) -> bool {
        matches!(self.current_config.r#type, MessageQueueType::Rabbitmq)
    }
    pub fn is_redis_stream(&self) -> bool {
        matches!(self.current_config.r#type, MessageQueueType::RedisStream)
    }
    pub fn get_current_type_string(&self) -> &'static str {
        MessageQueueFactory::get_type_string(&self.current_config.r#type)
    }
}

#[async_trait]
impl MessageQueue for MessageQueueManager {
    async fn publish_message(
        &self,
        queue: &str,
        message: &scheduler_domain::entities::Message,
    ) -> SchedulerResult<()> {
        self.current_queue.publish_message(queue, message).await
    }

    async fn consume_messages(
        &self,
        queue: &str,
    ) -> SchedulerResult<Vec<scheduler_domain::entities::Message>> {
        self.current_queue.consume_messages(queue).await
    }

    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()> {
        self.current_queue.ack_message(message_id).await
    }

    async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()> {
        self.current_queue.nack_message(message_id, requeue).await
    }

    async fn create_queue(&self, queue: &str, durable: bool) -> SchedulerResult<()> {
        self.current_queue.create_queue(queue, durable).await
    }

    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()> {
        self.current_queue.delete_queue(queue).await
    }

    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32> {
        self.current_queue.get_queue_size(queue).await
    }

    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()> {
        self.current_queue.purge_queue(queue).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_config::models::{MessageQueueConfig, MessageQueueType, RedisConfig};

    #[test]
    fn test_validate_rabbitmq_config() {
        let config = MessageQueueConfig {
            r#type: MessageQueueType::Rabbitmq,
            url: "amqp://localhost:5672".to_string(),
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 60,
            connection_timeout_seconds: 30,
        };

        assert!(MessageQueueFactory::validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_redis_stream_config_with_redis_section() {
        let config = MessageQueueConfig {
            r#type: MessageQueueType::RedisStream,
            url: "".to_string(),
            redis: Some(RedisConfig::default()),
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 60,
            connection_timeout_seconds: 30,
        };

        assert!(MessageQueueFactory::validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_redis_stream_config_with_url() {
        let config = MessageQueueConfig {
            r#type: MessageQueueType::RedisStream,
            url: "redis://localhost:6379".to_string(),
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 60,
            connection_timeout_seconds: 30,
        };

        assert!(MessageQueueFactory::validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_rabbitmq_config() {
        let config = MessageQueueConfig {
            r#type: MessageQueueType::Rabbitmq,
            url: "invalid://localhost:5672".to_string(),
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 60,
            connection_timeout_seconds: 30,
        };

        assert!(MessageQueueFactory::validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_redis_stream_config() {
        let config = MessageQueueConfig {
            r#type: MessageQueueType::RedisStream,
            url: "".to_string(),
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 60,
            connection_timeout_seconds: 30,
        };

        assert!(MessageQueueFactory::validate_config(&config).is_err());
    }

    #[test]
    fn test_parse_type_string() {
        assert_eq!(
            MessageQueueFactory::parse_type_string("rabbitmq").unwrap(),
            MessageQueueType::Rabbitmq
        );
        assert_eq!(
            MessageQueueFactory::parse_type_string("redis_stream").unwrap(),
            MessageQueueType::RedisStream
        );
        assert_eq!(
            MessageQueueFactory::parse_type_string("RABBITMQ").unwrap(),
            MessageQueueType::Rabbitmq
        );
        assert!(MessageQueueFactory::parse_type_string("invalid").is_err());
    }

    #[test]
    fn test_get_type_string() {
        assert_eq!(
            MessageQueueFactory::get_type_string(&MessageQueueType::Rabbitmq),
            "rabbitmq"
        );
        assert_eq!(
            MessageQueueFactory::get_type_string(&MessageQueueType::RedisStream),
            "redis_stream"
        );
    }

    #[test]
    fn test_parse_redis_url() {
        let config = MessageQueueConfig {
            r#type: MessageQueueType::RedisStream,
            url: "redis://user:pass@localhost:6380/1".to_string(),
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 60,
            connection_timeout_seconds: 30,
        };

        let redis_config = MessageQueueFactory::parse_redis_url(&config.url, &config).unwrap();
        assert_eq!(redis_config.host, "localhost");
        assert_eq!(redis_config.port, 6380);
        assert_eq!(redis_config.database, 1);
        assert_eq!(redis_config.password, Some("pass".to_string()));
    }
}
