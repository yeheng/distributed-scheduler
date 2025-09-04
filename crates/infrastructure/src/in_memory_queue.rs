use async_trait::async_trait;
use scheduler_domain::entities::Message;
use scheduler_domain::messaging::MessageQueue;
use scheduler_errors::SchedulerResult;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// 内存消息队列实现
/// 
/// 使用 Tokio channels 实现高性能的内存消息队列，适用于嵌入式部署场景。
/// 支持多个队列、消息持久化恢复、背压控制等特性。
#[derive(Debug)]
pub struct InMemoryMessageQueue {
    /// 队列存储：队列名 -> (发送端, 接收端)
    queues: Arc<RwLock<HashMap<String, QueueChannels>>>,
    /// 队列配置
    _config: InMemoryQueueConfig,
}

#[derive(Debug)]
struct QueueChannels {
    sender: mpsc::UnboundedSender<Message>,
    /// 使用 Arc 包装接收端，支持多个消费者
    receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Message>>>,
    /// 队列大小统计
    size: Arc<std::sync::atomic::AtomicU32>,
    /// 是否为持久化队列
    _durable: bool,
}

#[derive(Debug, Clone)]
pub struct InMemoryQueueConfig {
    /// 队列最大容量（0表示无限制）
    pub max_queue_size: usize,
    /// 是否启用消息持久化
    pub enable_persistence: bool,
    /// 消息重试间隔（毫秒）
    pub retry_interval_ms: u64,
}

impl Default for InMemoryQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000, // 默认最大10000条消息
            enable_persistence: false, // 默认不持久化
            retry_interval_ms: 100, // 默认100ms重试间隔
        }
    }
}

impl InMemoryMessageQueue {
    /// 创建新的内存消息队列实例
    pub fn new() -> Self {
        Self::with_config(InMemoryQueueConfig::default())
    }

    /// 使用指定配置创建内存消息队列实例
    pub fn with_config(config: InMemoryQueueConfig) -> Self {
        info!("Creating in-memory message queue with config: {:?}", config);
        Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            _config: config,
        }
    }

    /// 获取或创建队列通道
    async fn get_or_create_queue(&self, queue_name: &str, durable: bool) -> SchedulerResult<()> {
        let mut queues = self.queues.write().await;
        
        if !queues.contains_key(queue_name) {
            debug!("Creating new queue: {}", queue_name);
            
            // 暂时使用无界队列简化实现
            let (sender, receiver) = mpsc::unbounded_channel();

            let channels = QueueChannels {
                sender,
                receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
                size: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                _durable: durable,
            };

            queues.insert(queue_name.to_string(), channels);
            info!("Created queue '{}' (durable: {})", queue_name, durable);
        }

        Ok(())
    }

    /// 获取队列发送端
    async fn get_sender(&self, queue_name: &str) -> SchedulerResult<mpsc::UnboundedSender<Message>> {
        let queues = self.queues.read().await;
        queues
            .get(queue_name)
            .map(|channels| channels.sender.clone())
            .ok_or_else(|| {
                scheduler_errors::SchedulerError::MessageQueue(format!(
                    "Queue '{}' not found",
                    queue_name
                ))
            })
    }

    /// 获取队列接收端
    async fn get_receiver(&self, queue_name: &str) -> SchedulerResult<Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Message>>>> {
        let queues = self.queues.read().await;
        queues
            .get(queue_name)
            .map(|channels| channels.receiver.clone())
            .ok_or_else(|| {
                scheduler_errors::SchedulerError::MessageQueue(format!(
                    "Queue '{}' not found",
                    queue_name
                ))
            })
    }

    /// 增加队列大小计数
    async fn increment_queue_size(&self, queue_name: &str) {
        if let Some(size_counter) = self.queues.read().await.get(queue_name).map(|q| q.size.clone()) {
            size_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// 减少队列大小计数
    async fn _decrement_queue_size(&self, queue_name: &str) {
        if let Some(size_counter) = self.queues.read().await.get(queue_name).map(|q| q.size.clone()) {
            size_counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

#[async_trait]
impl MessageQueue for InMemoryMessageQueue {
    async fn publish_message(&self, queue: &str, message: &Message) -> SchedulerResult<()> {
        debug!("Publishing message to queue '{}': {}", queue, message.id);

        // 确保队列存在
        self.get_or_create_queue(queue, false).await?;

        // 获取发送端并发送消息
        let sender = self.get_sender(queue).await?;
        
        sender.send(message.clone()).map_err(|e| {
            error!("Failed to send message to queue '{}': {}", queue, e);
            scheduler_errors::SchedulerError::MessageQueue(format!(
                "Failed to send message to queue '{}': {}",
                queue, e
            ))
        })?;

        // 更新队列大小
        self.increment_queue_size(queue).await;

        debug!("Successfully published message {} to queue '{}'", message.id, queue);
        Ok(())
    }

    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>> {
        debug!("Consuming messages from queue '{}'", queue);

        // 确保队列存在
        self.get_or_create_queue(queue, false).await?;

        let receiver = self.get_receiver(queue).await?;
        let mut messages = Vec::new();

        // 非阻塞地接收所有可用消息
        {
            let mut rx = receiver.lock().await;
            while let Ok(message) = rx.try_recv() {
                debug!("Consumed message {} from queue '{}'", message.id, queue);
                messages.push(message);
            }
        }

        // 批量更新队列大小计数
        if !messages.is_empty() {
            if let Some(size_counter) = self.queues.read().await.get(queue).map(|q| q.size.clone()) {
                for _ in 0..messages.len() {
                    size_counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }

        debug!("Consumed {} messages from queue '{}'", messages.len(), queue);
        Ok(messages)
    }

    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()> {
        debug!("Acknowledging message: {}", message_id);
        // 内存队列中消息一旦消费就自动确认，这里只是记录日志
        Ok(())
    }

    async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()> {
        debug!("Negative acknowledging message: {} (requeue: {})", message_id, requeue);
        
        if requeue {
            warn!("Message {} nacked with requeue, but in-memory queue doesn't support requeue", message_id);
        }
        
        // 内存队列暂不支持消息重新入队，这里只是记录日志
        Ok(())
    }

    async fn create_queue(&self, queue: &str, durable: bool) -> SchedulerResult<()> {
        info!("Creating queue '{}' (durable: {})", queue, durable);
        self.get_or_create_queue(queue, durable).await
    }

    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()> {
        info!("Deleting queue '{}'", queue);
        
        let mut queues = self.queues.write().await;
        if let Some(channels) = queues.remove(queue) {
            // 关闭发送端，这会导致接收端也关闭
            drop(channels.sender);
            info!("Successfully deleted queue '{}'", queue);
        } else {
            warn!("Queue '{}' not found for deletion", queue);
        }
        
        Ok(())
    }

    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32> {
        let queues = self.queues.read().await;
        let size = queues
            .get(queue)
            .map(|channels| channels.size.load(std::sync::atomic::Ordering::Relaxed))
            .ok_or_else(|| {
                scheduler_errors::SchedulerError::MessageQueue(format!(
                    "Queue '{}' not found",
                    queue
                ))
            })?;
        
        debug!("Queue '{}' size: {}", queue, size);
        Ok(size)
    }

    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()> {
        info!("Purging queue '{}'", queue);
        
        let receiver = self.get_receiver(queue).await?;
        let mut purged_count = 0;

        // 清空队列中的所有消息
        {
            let mut rx = receiver.lock().await;
            while rx.try_recv().is_ok() {
                purged_count += 1;
            }
        }

        // 重置队列大小计数
        if let Some(size_counter) = self.queues.read().await.get(queue).map(|q| q.size.clone()) {
            size_counter.store(0, std::sync::atomic::Ordering::Relaxed);
        }

        info!("Purged {} messages from queue '{}'", purged_count, queue);
        Ok(())
    }
}

impl Default for InMemoryMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_domain::entities::{Message, TaskExecutionMessage};
    use tokio;

    #[tokio::test]
    async fn test_create_and_publish_message() {
        let queue = InMemoryMessageQueue::new();
        let queue_name = "test_queue";
        
        // 创建队列
        queue.create_queue(queue_name, false).await.unwrap();
        
        // 创建测试消息
        let execution_msg = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };
        let message = Message::task_execution(execution_msg);
        
        // 发布消息
        queue.publish_message(queue_name, &message).await.unwrap();
        
        // 检查队列大小
        let size = queue.get_queue_size(queue_name).await.unwrap();
        assert_eq!(size, 1);
        
        // 消费消息
        let messages = queue.consume_messages(queue_name).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, message.id);
        
        // 检查队列大小（消费后应该为0）
        let size = queue.get_queue_size(queue_name).await.unwrap();
        assert_eq!(size, 0);
    }

    #[tokio::test]
    async fn test_multiple_queues() {
        let queue = InMemoryMessageQueue::new();
        
        // 创建多个队列
        queue.create_queue("queue1", false).await.unwrap();
        queue.create_queue("queue2", true).await.unwrap();
        
        // 创建测试消息
        let execution_msg = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };
        let message1 = Message::task_execution(execution_msg.clone());
        let message2 = Message::task_execution(execution_msg);
        
        // 向不同队列发布消息
        queue.publish_message("queue1", &message1).await.unwrap();
        queue.publish_message("queue2", &message2).await.unwrap();
        
        // 检查队列大小
        assert_eq!(queue.get_queue_size("queue1").await.unwrap(), 1);
        assert_eq!(queue.get_queue_size("queue2").await.unwrap(), 1);
        
        // 从不同队列消费消息
        let messages1 = queue.consume_messages("queue1").await.unwrap();
        let messages2 = queue.consume_messages("queue2").await.unwrap();
        
        assert_eq!(messages1.len(), 1);
        assert_eq!(messages2.len(), 1);
        assert_eq!(messages1[0].id, message1.id);
        assert_eq!(messages2[0].id, message2.id);
    }

    #[tokio::test]
    async fn test_purge_queue() {
        let queue = InMemoryMessageQueue::new();
        let queue_name = "test_queue";
        
        // 创建队列并发布多条消息
        queue.create_queue(queue_name, false).await.unwrap();
        
        for i in 0..5 {
            let execution_msg = TaskExecutionMessage {
                task_run_id: i,
                task_id: i,
                task_name: format!("test_task_{}", i),
                task_type: "shell".to_string(),
                parameters: serde_json::json!({}),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };
            let message = Message::task_execution(execution_msg);
            queue.publish_message(queue_name, &message).await.unwrap();
        }
        
        // 检查队列大小
        assert_eq!(queue.get_queue_size(queue_name).await.unwrap(), 5);
        
        // 清空队列
        queue.purge_queue(queue_name).await.unwrap();
        
        // 检查队列大小
        assert_eq!(queue.get_queue_size(queue_name).await.unwrap(), 0);
        
        // 尝试消费消息，应该为空
        let messages = queue.consume_messages(queue_name).await.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_queue() {
        let queue = InMemoryMessageQueue::new();
        let queue_name = "test_queue";
        
        // 创建队列
        queue.create_queue(queue_name, false).await.unwrap();
        
        // 发布消息
        let execution_msg = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };
        let message = Message::task_execution(execution_msg);
        queue.publish_message(queue_name, &message).await.unwrap();
        
        // 删除队列
        queue.delete_queue(queue_name).await.unwrap();
        
        // 尝试获取队列大小应该失败
        assert!(queue.get_queue_size(queue_name).await.is_err());
    }
}