//! Redis Stream消息队列模块
//!
//! 这个模块提供了基于Redis Stream的轻量级消息队列实现，
//! 按照单一职责原则分解为多个子模块。

pub mod config;
pub mod connection_manager;
pub mod message_handler;
pub mod metrics_collector;
pub mod stream_operations;

// 重新导出公共接口
pub use config::RedisStreamConfig;
pub use connection_manager::RedisConnectionManager;
pub use message_handler::RedisMessageHandler;
pub use metrics_collector::RedisStreamMetrics;
pub use stream_operations::RedisStreamOperations;

use async_trait::async_trait;
use scheduler_core::{
    models::Message, traits::MessageQueue, SchedulerResult,
};
use std::sync::Arc;

/// 健康状态
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub healthy: bool,
    pub error_message: Option<String>,
}

/// Redis Stream消息队列主要实现
///
/// 协调各个子组件提供完整的消息队列功能
pub struct RedisStreamMessageQueue {
    connection_manager: Arc<RedisConnectionManager>,
    message_handler: Arc<RedisMessageHandler>,
    stream_operations: Arc<RedisStreamOperations>,
    metrics: Arc<RedisStreamMetrics>,
}

impl RedisStreamMessageQueue {
    /// 创建新的Redis Stream消息队列实例
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

    /// 获取性能指标
    pub fn metrics(&self) -> Arc<RedisStreamMetrics> {
        self.metrics.clone()
    }

    /// 健康检查
    pub async fn health_check(&self) -> SchedulerResult<HealthStatus> {
        // 尝试ping Redis连接
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
        self.stream_operations.create_queue(queue_name, durable).await
    }

    async fn delete_queue(&self, queue_name: &str) -> SchedulerResult<()> {
        self.stream_operations.delete_queue(queue_name).await
    }

    async fn publish_message(&self, queue_name: &str, message: &Message) -> SchedulerResult<()> {
        self.message_handler.publish_message(queue_name, message).await
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