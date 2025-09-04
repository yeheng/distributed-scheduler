pub mod config;
pub mod connection_manager;
pub mod message_handler;
pub mod metrics_collector;
pub mod stream_operations;
pub use config::RedisStreamConfig;
pub use connection_manager::RedisConnectionManager;
pub use message_handler::RedisMessageHandler;
pub use metrics_collector::RedisStreamMetrics;
pub use stream_operations::RedisStreamOperations;

use async_trait::async_trait;
use scheduler_domain::entities::Message;
use scheduler_domain::messaging::MessageQueue;
use scheduler_errors::SchedulerResult;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub healthy: bool,
    pub error_message: Option<String>,
}

pub struct RedisStreamMessageQueue {
    connection_manager: Arc<RedisConnectionManager>,
    message_handler: Arc<RedisMessageHandler>,
    stream_operations: Arc<RedisStreamOperations>,
    metrics: Arc<RedisStreamMetrics>,
}

impl RedisStreamMessageQueue {
    pub async fn new(config: RedisStreamConfig) -> SchedulerResult<Self> {
        let metrics = Arc::new(RedisStreamMetrics::default());
        let connection_manager = Arc::new(RedisConnectionManager::new(config.clone()).await?);
        let message_handler = Arc::new(RedisMessageHandler::new(
            connection_manager.clone(),
            config.clone(),
            metrics.clone(),
        ));
        let stream_operations = Arc::new(RedisStreamOperations::new(
            connection_manager.clone(),
            config,
            metrics.clone(),
        ));

        Ok(Self {
            connection_manager,
            message_handler,
            stream_operations,
            metrics,
        })
    }
    pub fn metrics(&self) -> Arc<RedisStreamMetrics> {
        self.metrics.clone()
    }
    pub async fn health_check(&self) -> SchedulerResult<HealthStatus> {
        let ping_result = self.connection_manager.ping().await;

        match ping_result {
            Ok(_) => Ok(HealthStatus {
                healthy: true,
                error_message: None,
            }),
            Err(e) => Ok(HealthStatus {
                healthy: false,
                error_message: Some(e.to_string()),
            }),
        }
    }
}

#[async_trait]
impl MessageQueue for RedisStreamMessageQueue {
    async fn create_queue(&self, queue_name: &str, durable: bool) -> SchedulerResult<()> {
        self.stream_operations
            .create_queue(queue_name, durable)
            .await
    }

    async fn delete_queue(&self, queue_name: &str) -> SchedulerResult<()> {
        self.stream_operations.delete_queue(queue_name).await
    }

    async fn publish_message(&self, queue_name: &str, message: &Message) -> SchedulerResult<()> {
        self.message_handler
            .publish_message(queue_name, message)
            .await
    }

    async fn consume_messages(&self, queue_name: &str) -> SchedulerResult<Vec<Message>> {
        self.message_handler.consume_messages(queue_name).await
    }

    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()> {
        self.message_handler.ack_message(message_id).await
    }

    async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()> {
        self.message_handler.nack_message(message_id, requeue).await
    }

    async fn get_queue_size(&self, queue_name: &str) -> SchedulerResult<u32> {
        let size = self.stream_operations.get_queue_size(queue_name).await?;
        Ok(size as u32)
    }

    async fn purge_queue(&self, queue_name: &str) -> SchedulerResult<()> {
        self.stream_operations.purge_queue(queue_name).await?;
        Ok(())
    }
}
