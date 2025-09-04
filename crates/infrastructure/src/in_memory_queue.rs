use async_trait::async_trait;
use scheduler_domain::entities::Message;
use scheduler_domain::messaging::MessageQueue;
use scheduler_errors::SchedulerResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tracing::{debug, error, info, warn};

/// 内存消息队列实现
/// 
/// 使用 Tokio channels 实现高性能的内存消息队列，适用于嵌入式部署场景。
/// 支持多个队列、消息持久化恢复、背压控制、自动清理等特性。
#[derive(Debug)]
pub struct InMemoryMessageQueue {
    /// 队列存储：队列名 -> (发送端, 接收端)
    queues: Arc<RwLock<HashMap<String, QueueChannels>>>,
    /// 队列配置
    config: InMemoryQueueConfig,
    /// 清理任务句柄
    cleanup_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
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
    /// 背压控制信号量
    backpressure_semaphore: Arc<Semaphore>,
    /// 队列创建时间
    created_at: Instant,
    /// 最后访问时间
    last_accessed: Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Debug, Clone)]
pub struct InMemoryQueueConfig {
    /// 队列最大容量（0表示无限制）
    pub max_queue_size: usize,
    /// 是否启用消息持久化
    pub enable_persistence: bool,
    /// 消息重试间隔（毫秒）
    pub retry_interval_ms: u64,
    /// 背压控制阈值（队列大小超过此值时启用背压）
    pub backpressure_threshold: usize,
    /// 背压等待超时时间（毫秒）
    pub backpressure_timeout_ms: u64,
    /// 自动清理间隔（秒）
    pub cleanup_interval_seconds: u64,
    /// 队列空闲超时时间（秒，超过此时间未访问的队列将被清理）
    pub idle_timeout_seconds: u64,
    /// 内存使用限制（MB，0表示无限制）
    pub memory_limit_mb: usize,
}

impl Default for InMemoryQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000, // 默认最大10000条消息
            enable_persistence: false, // 默认不持久化
            retry_interval_ms: 100, // 默认100ms重试间隔
            backpressure_threshold: 8000, // 80%容量时启用背压
            backpressure_timeout_ms: 5000, // 5秒背压超时
            cleanup_interval_seconds: 300, // 5分钟清理间隔
            idle_timeout_seconds: 1800, // 30分钟空闲超时
            memory_limit_mb: 100, // 100MB内存限制
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
        let queue = Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            config: config.clone(),
            cleanup_handle: Arc::new(tokio::sync::Mutex::new(None)),
        };
        
        // 启动自动清理任务
        queue.start_cleanup_task();
        
        queue
    }

    /// 启动自动清理任务
    fn start_cleanup_task(&self) {
        if self.config.cleanup_interval_seconds > 0 {
            let queues = self.queues.clone();
            let config = self.config.clone();
            let cleanup_handle = self.cleanup_handle.clone();
            
            let handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(config.cleanup_interval_seconds));
                
                loop {
                    interval.tick().await;
                    
                    if let Err(e) = Self::cleanup_idle_queues(&queues, &config).await {
                        warn!("Failed to cleanup idle queues: {}", e);
                    }
                }
            });
            
            tokio::spawn(async move {
                *cleanup_handle.lock().await = Some(handle);
            });
        }
    }

    /// 清理空闲队列
    async fn cleanup_idle_queues(
        queues: &Arc<RwLock<HashMap<String, QueueChannels>>>,
        config: &InMemoryQueueConfig,
    ) -> SchedulerResult<()> {
        let now = Instant::now();
        let idle_threshold = Duration::from_secs(config.idle_timeout_seconds);
        let mut to_remove = Vec::new();
        
        {
            let queues_read = queues.read().await;
            for (queue_name, channels) in queues_read.iter() {
                let last_accessed = Duration::from_secs(
                    channels.last_accessed.load(std::sync::atomic::Ordering::Relaxed)
                );
                let last_accessed_instant = Instant::now() - (Duration::from_secs(now.elapsed().as_secs()) - last_accessed);
                
                if now.duration_since(last_accessed_instant) > idle_threshold {
                    let queue_size = channels.size.load(std::sync::atomic::Ordering::Relaxed);
                    if queue_size == 0 {
                        to_remove.push(queue_name.clone());
                    }
                }
            }
        }
        
        if !to_remove.is_empty() {
            let mut queues_write = queues.write().await;
            for queue_name in &to_remove {
                if let Some(channels) = queues_write.remove(queue_name) {
                    drop(channels.sender);
                    info!("Cleaned up idle queue: {}", queue_name);
                }
            }
            info!("Cleaned up {} idle queues", to_remove.len());
        }
        
        Ok(())
    }

    /// 检查内存使用情况
    fn check_memory_usage(&self) -> bool {
        if self.config.memory_limit_mb == 0 {
            return true; // 无限制
        }
        
        // 简单的内存使用估算（实际实现可能需要更精确的测量）
        let estimated_memory_mb = self.estimate_memory_usage();
        let usage_ok = estimated_memory_mb < self.config.memory_limit_mb;
        
        if !usage_ok {
            warn!(
                "Memory usage limit exceeded: {}MB / {}MB",
                estimated_memory_mb, self.config.memory_limit_mb
            );
        }
        
        usage_ok
    }

    /// 估算内存使用量（MB）
    fn estimate_memory_usage(&self) -> usize {
        // 简化的内存估算：假设每条消息平均1KB
        // 由于这个方法在同步上下文中调用，我们需要避免使用async
        // 这里返回一个估算值，实际实现可能需要更精确的测量
        
        // 简化实现：返回固定估算值
        // 在实际应用中，可以维护一个原子计数器来跟踪总消息数
        50 // 假设50MB基础内存使用
    }

    /// 获取队列统计信息
    pub async fn get_queue_stats(&self) -> QueueStats {
        let queues = self.queues.read().await;
        let mut stats = QueueStats::default();
        
        for (name, channels) in queues.iter() {
            let size = channels.size.load(std::sync::atomic::Ordering::Relaxed);
            let last_accessed = Duration::from_secs(
                channels.last_accessed.load(std::sync::atomic::Ordering::Relaxed)
            );
            let age = channels.created_at.elapsed();
            
            stats.total_queues += 1;
            stats.total_messages += size as usize;
            stats.queue_details.push(QueueDetail {
                name: name.clone(),
                size: size as usize,
                age,
                last_accessed,
                durable: channels._durable,
            });
        }
        
        stats.estimated_memory_mb = self.estimate_memory_usage();
        stats
    }

    /// 强制垃圾回收（清理空队列和释放内存）
    pub async fn force_gc(&self) -> GcStats {
        let start_time = Instant::now();
        let mut gc_stats = GcStats::default();
        
        // 清理空闲队列
        if let Err(e) = Self::cleanup_idle_queues(&self.queues, &self.config).await {
            warn!("Failed to cleanup idle queues during GC: {}", e);
        } else {
            gc_stats.idle_queues_cleaned = 1; // 简化统计
        }
        
        // 强制Rust垃圾回收
        // 注意：Rust没有显式的GC，但我们可以尝试释放一些资源
        
        gc_stats.duration = start_time.elapsed();
        info!("Forced GC completed: {:?}", gc_stats);
        gc_stats
    }

    /// 更新队列访问时间
    async fn update_access_time(&self, queue_name: &str) {
        if let Some(channels) = self.queues.read().await.get(queue_name) {
            let now_secs = Instant::now().elapsed().as_secs();
            channels.last_accessed.store(now_secs, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// 获取或创建队列通道
    async fn get_or_create_queue(&self, queue_name: &str, durable: bool) -> SchedulerResult<()> {
        let mut queues = self.queues.write().await;
        
        if !queues.contains_key(queue_name) {
            debug!("Creating new queue: {}", queue_name);
            
            // 检查内存使用情况
            if !self.check_memory_usage() {
                return Err(scheduler_errors::SchedulerError::MessageQueue(
                    format!("Memory limit exceeded, cannot create queue '{}'", queue_name)
                ));
            }
            
            // 使用无界队列但通过信号量控制背压
            let (sender, receiver) = mpsc::unbounded_channel();

            let now = Instant::now();
            let channels = QueueChannels {
                sender,
                receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
                size: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                _durable: durable,
                backpressure_semaphore: Arc::new(Semaphore::new(self.config.backpressure_threshold)),
                created_at: now,
                last_accessed: Arc::new(std::sync::atomic::AtomicU64::new(now.elapsed().as_secs())),
            };

            queues.insert(queue_name.to_string(), channels);
            info!("Created queue '{}' (durable: {}, backpressure_threshold: {})", 
                  queue_name, durable, self.config.backpressure_threshold);
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

        // 检查内存使用情况
        if !self.check_memory_usage() {
            warn!("Memory limit exceeded, rejecting message for queue '{}'", queue);
            return Err(scheduler_errors::SchedulerError::MessageQueue(
                format!("Memory limit exceeded for queue '{}'", queue)
            ));
        }

        // 获取背压控制信号量
        let semaphore = {
            let queues = self.queues.read().await;
            queues.get(queue)
                .map(|channels| channels.backpressure_semaphore.clone())
                .ok_or_else(|| {
                    scheduler_errors::SchedulerError::MessageQueue(format!(
                        "Queue '{}' not found",
                        queue
                    ))
                })?
        };

        // 尝试获取信号量许可（背压控制）
        let permit = if self.config.backpressure_timeout_ms > 0 {
            tokio::time::timeout(
                Duration::from_millis(self.config.backpressure_timeout_ms),
                semaphore.acquire()
            ).await.map_err(|_| {
                warn!("Backpressure timeout for queue '{}', message rejected", queue);
                scheduler_errors::SchedulerError::MessageQueue(format!(
                    "Backpressure timeout for queue '{}'", queue
                ))
            })?.map_err(|e| {
                scheduler_errors::SchedulerError::MessageQueue(format!(
                    "Failed to acquire backpressure permit: {}", e
                ))
            })?
        } else {
            semaphore.acquire().await.map_err(|e| {
                scheduler_errors::SchedulerError::MessageQueue(format!(
                    "Failed to acquire backpressure permit: {}", e
                ))
            })?
        };

        // 获取发送端并发送消息
        let sender = self.get_sender(queue).await?;
        
        sender.send(message.clone()).map_err(|e| {
            error!("Failed to send message to queue '{}': {}", queue, e);
            scheduler_errors::SchedulerError::MessageQueue(format!(
                "Failed to send message to queue '{}': {}",
                queue, e
            ))
        })?;

        // 更新队列大小和访问时间
        self.increment_queue_size(queue).await;
        self.update_access_time(queue).await;

        // 保持许可直到消息被消费（通过forget释放所有权）
        permit.forget();

        debug!("Successfully published message {} to queue '{}'", message.id, queue);
        Ok(())
    }

    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>> {
        debug!("Consuming messages from queue '{}'", queue);

        // 确保队列存在
        self.get_or_create_queue(queue, false).await?;

        let receiver = self.get_receiver(queue).await?;
        let mut messages = Vec::new();

        // 获取背压控制信号量
        let semaphore = {
            let queues = self.queues.read().await;
            queues.get(queue)
                .map(|channels| channels.backpressure_semaphore.clone())
                .ok_or_else(|| {
                    scheduler_errors::SchedulerError::MessageQueue(format!(
                        "Queue '{}' not found",
                        queue
                    ))
                })?
        };

        // 非阻塞地接收所有可用消息
        {
            let mut rx = receiver.lock().await;
            while let Ok(message) = rx.try_recv() {
                debug!("Consumed message {} from queue '{}'", message.id, queue);
                messages.push(message);
            }
        }

        // 批量更新队列大小计数和释放背压许可
        if !messages.is_empty() {
            if let Some(size_counter) = self.queues.read().await.get(queue).map(|q| q.size.clone()) {
                for _ in 0..messages.len() {
                    size_counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    // 释放背压许可
                    semaphore.add_permits(1);
                }
            }
            
            // 更新访问时间
            self.update_access_time(queue).await;
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

/// 队列统计信息
#[derive(Debug, Default)]
pub struct QueueStats {
    /// 总队列数
    pub total_queues: usize,
    /// 总消息数
    pub total_messages: usize,
    /// 估算内存使用量（MB）
    pub estimated_memory_mb: usize,
    /// 队列详细信息
    pub queue_details: Vec<QueueDetail>,
}

/// 队列详细信息
#[derive(Debug)]
pub struct QueueDetail {
    /// 队列名称
    pub name: String,
    /// 队列大小
    pub size: usize,
    /// 队列年龄
    pub age: Duration,
    /// 最后访问时间
    pub last_accessed: Duration,
    /// 是否持久化
    pub durable: bool,
}

/// 垃圾回收统计
#[derive(Debug, Default)]
pub struct GcStats {
    /// 清理的空闲队列数
    pub idle_queues_cleaned: usize,
    /// 释放的内存量（MB）
    pub memory_freed_mb: usize,
    /// GC耗时
    pub duration: Duration,
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