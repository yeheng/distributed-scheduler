use scheduler_core::{errors::SchedulerError, models::Message, SchedulerResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, warn};

use super::config::RedisStreamConfig;
use super::connection_manager::RedisConnectionManager;
use super::metrics_collector::RedisStreamMetrics;

/// Redis消息处理器
///
/// 负责消息的发布、消费、确认和拒绝操作
pub struct RedisMessageHandler {
    connection_manager: Arc<RedisConnectionManager>,
    config: RedisStreamConfig,
    metrics: Arc<RedisStreamMetrics>,
    // 存储Message ID到Redis Stream ID和队列名的映射
    message_id_mapping: Arc<Mutex<HashMap<String, (String, String)>>>,
}

impl RedisMessageHandler {
    /// 创建新的消息处理器
    pub fn new(
        connection_manager: Arc<RedisConnectionManager>,
        config: RedisStreamConfig,
        metrics: Arc<RedisStreamMetrics>,
    ) -> Self {
        Self {
            connection_manager,
            config,
            metrics,
            message_id_mapping: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 发布消息
    pub async fn publish_message(&self, queue: &str, message: &Message) -> SchedulerResult<()> {
        debug!("Publishing message {} to queue: {}", message.id, queue);

        self.validate_queue_name(queue)?;
        self.validate_message(message)?;
        
        self.publish_message_with_retry(queue, message).await
    }

    /// 消费消息
    pub async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>> {
        debug!("Consuming messages from queue: {}", queue);

        self.validate_queue_name(queue)?;

        let mut all_messages = Vec::new();

        // 处理待处理的消息
        if let Ok(mut pending_messages) = self.consume_pending_messages(queue).await {
            debug!("Found {} pending messages in queue: {}", pending_messages.len(), queue);
            all_messages.append(&mut pending_messages);
        }

        // 读取新消息
        if let Ok(mut new_messages) = self.consume_new_messages(queue).await {
            debug!("Found {} new messages in queue: {}", new_messages.len(), queue);
            all_messages.append(&mut new_messages);
        }

        debug!("Consumed {} total messages from queue: {}", all_messages.len(), queue);
        Ok(all_messages)
    }

    /// 确认消息
    pub async fn ack_message(&self, message_id: &str) -> SchedulerResult<()> {
        let start = Instant::now();
        debug!("Acknowledging message: {}", message_id);

        let (stream_message_id, queue_name) = self.get_message_mapping(message_id)?;
        let group_name = self.get_consumer_group_name(&queue_name);

        let mut cmd = redis::cmd("XACK");
        cmd.arg(&queue_name).arg(&group_name).arg(&stream_message_id);
        
        let ack_count: i64 = self.connection_manager.execute_command(&mut cmd).await?;

        let duration = start.elapsed();
        self.metrics.record_message_acked();
        self.metrics.record_operation_duration("ack", duration.as_millis() as f64);

        if ack_count > 0 {
            debug!("Successfully acknowledged message {} in {:?}", message_id, duration);
            self.remove_message_mapping(message_id);
        } else {
            warn!("Message {} was not acknowledged (possibly already processed)", message_id);
        }

        Ok(())
    }

    /// 拒绝消息
    pub async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()> {
        let start = Instant::now();
        debug!("Nacking message: {}, requeue: {}", message_id, requeue);

        let (stream_message_id, queue_name) = self.get_message_mapping(message_id)?;

        if requeue {
            // 重新排队：将消息添加回流的末尾
            self.requeue_message(&queue_name, &stream_message_id).await?;
        }

        // 从消费者组的待处理消息中移除
        let group_name = self.get_consumer_group_name(&queue_name);
        let mut cmd = redis::cmd("XACK");
        cmd.arg(&queue_name).arg(&group_name).arg(&stream_message_id);
        
        let _: i64 = self.connection_manager.execute_command(&mut cmd).await?;

        let duration = start.elapsed();
        self.metrics.record_message_nacked();
        self.metrics.record_operation_duration("nack", duration.as_millis() as f64);

        debug!("Successfully nacked message {} in {:?}", message_id, duration);
        self.remove_message_mapping(message_id);

        Ok(())
    }

    // 私有辅助方法
    fn validate_queue_name(&self, queue: &str) -> SchedulerResult<()> {
        if queue.is_empty() {
            return Err(SchedulerError::MessageQueue("Queue name cannot be empty".to_string()));
        }
        if queue.len() > 255 {
            return Err(SchedulerError::MessageQueue("Queue name too long".to_string()));
        }
        if queue.contains(' ') || queue.contains('\n') || queue.contains('\r') {
            return Err(SchedulerError::MessageQueue("Queue name contains invalid characters".to_string()));
        }
        Ok(())
    }

    fn validate_message(&self, message: &Message) -> SchedulerResult<()> {
        if message.id.is_empty() {
            return Err(SchedulerError::MessageQueue("Message ID cannot be empty".to_string()));
        }
        Ok(())
    }

    fn serialize_message(&self, message: &Message) -> SchedulerResult<String> {
        serde_json::to_string(message).map_err(|e| {
            SchedulerError::Serialization(format!("Failed to serialize message {}: {}", message.id, e))
        })
    }

    fn deserialize_message(&self, data: &str) -> SchedulerResult<Message> {
        serde_json::from_str(data).map_err(|e| {
            SchedulerError::Serialization(format!("Failed to deserialize message data: {e}"))
        })
    }

    fn get_consumer_group_name(&self, queue: &str) -> String {
        format!("{}_{}", self.config.consumer_group_prefix, queue)
    }

    fn get_message_mapping(&self, message_id: &str) -> SchedulerResult<(String, String)> {
        let mapping = self.message_id_mapping.lock().map_err(|e| {
            SchedulerError::MessageQueue(format!("Failed to lock message mapping: {e}"))
        })?;

        mapping.get(message_id).cloned().ok_or_else(|| {
            SchedulerError::MessageQueue(format!("Message ID {message_id} not found in mapping"))
        })
    }

    fn add_message_mapping(&self, message_id: String, stream_id: String, queue_name: String) {
        if let Ok(mut mapping) = self.message_id_mapping.lock() {
            mapping.insert(message_id, (stream_id, queue_name));
        }
    }

    fn remove_message_mapping(&self, message_id: &str) {
        if let Ok(mut mapping) = self.message_id_mapping.lock() {
            mapping.remove(message_id);
        }
    }

    // 占位符方法 - 实际实现需要从原文件迁移
    async fn publish_message_with_retry(&self, _queue: &str, _message: &Message) -> SchedulerResult<()> {
        // TODO: 从原文件迁移实现
        Ok(())
    }

    async fn consume_pending_messages(&self, _queue: &str) -> SchedulerResult<Vec<Message>> {
        // TODO: 从原文件迁移实现
        Ok(vec![])
    }

    async fn consume_new_messages(&self, _queue: &str) -> SchedulerResult<Vec<Message>> {
        // TODO: 从原文件迁移实现
        Ok(vec![])
    }

    async fn requeue_message(&self, _queue_name: &str, _stream_message_id: &str) -> SchedulerResult<()> {
        // TODO: 从原文件迁移实现
        Ok(())
    }
}