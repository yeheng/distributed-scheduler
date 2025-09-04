use async_trait::async_trait;
use scheduler_domain::entities::{Message, MessageType};
use scheduler_domain::messaging::MessageQueue;
use scheduler_errors::SchedulerResult;
use std::collections::HashMap;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// 全局内存使用量统计（字节）
    total_memory_usage: Arc<AtomicUsize>,
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
    /// 精确内存使用量统计（字节）
    memory_usage_bytes: Arc<AtomicUsize>,
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
            total_memory_usage: Arc::new(AtomicUsize::new(0)),
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

    /// 精确计算内存使用量（MB）
    fn estimate_memory_usage(&self) -> usize {
        let total_bytes = self.total_memory_usage.load(Ordering::Relaxed);
        let base_overhead = self.calculate_base_overhead();
        // 转换为MB，向上取整
        (total_bytes + base_overhead + 1024 * 1024 - 1) / (1024 * 1024)
    }

    /// 计算基础开销（队列管理结构等）
    fn calculate_base_overhead(&self) -> usize {
        // 基础结构大小
        let base_size = mem::size_of::<Self>();
        // 估算HashMap和其他管理结构的开销
        // 每个队列的管理开销大约包括：
        // - HashMap entry: ~48 bytes
        // - QueueChannels struct: ~200 bytes
        // - Arc/Mutex 开销: ~100 bytes
        let queue_count = self.get_queue_count_sync();
        let queue_overhead = queue_count * 350; // 每个队列约350字节开销
        
        base_size + queue_overhead
    }

    /// 同步获取队列数量（用于内存计算）
    fn get_queue_count_sync(&self) -> usize {
        // 由于这个方法在同步上下文中调用，我们使用try_read来避免阻塞
        match self.queues.try_read() {
            Ok(queues) => queues.len(),
            Err(_) => 0, // 如果无法获取锁，返回0作为保守估计
        }
    }

    /// 计算单个消息的内存使用量
    fn calculate_message_size(message: &Message) -> usize {
        // 基础Message结构大小
        let base_size = mem::size_of::<Message>();
        
        // 计算消息ID和其他字符串字段的大小
        let id_size = message.id.len();
        let correlation_id_size = message.correlation_id.as_ref().map_or(0, |s| s.len());
        let trace_headers_size = message.trace_headers.as_ref().map_or(0, |headers| {
            headers.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>()
        });
        
        // 计算payload的大小
        let payload_size = Self::estimate_json_size(&message.payload);
        
        // 计算message_type中的动态内容大小
        let message_type_size = match &message.message_type {
            MessageType::TaskExecution(exec_msg) => {
                exec_msg.task_name.len() + 
                exec_msg.task_type.len() +
                Self::estimate_json_size(&exec_msg.parameters)
            },
            MessageType::StatusUpdate(status_msg) => {
                status_msg.worker_id.len() +
                status_msg.error_message.as_ref().map_or(0, |s| s.len())
            },
            MessageType::WorkerHeartbeat(heartbeat_msg) => {
                heartbeat_msg.worker_id.len()
            },
            MessageType::TaskControl(control_msg) => {
                control_msg.requester.len()
            },
        };
        
        base_size + id_size + correlation_id_size + trace_headers_size + payload_size + message_type_size
    }

    /// 估算JSON值的内存使用量
    fn estimate_json_size(value: &serde_json::Value) -> usize {
        match value {
            serde_json::Value::Null => 0,
            serde_json::Value::Bool(_) => 1,
            serde_json::Value::Number(_) => 8, // 假设最大为f64
            serde_json::Value::String(s) => s.len(),
            serde_json::Value::Array(arr) => {
                arr.iter().map(Self::estimate_json_size).sum::<usize>() + 
                mem::size_of::<Vec<serde_json::Value>>()
            },
            serde_json::Value::Object(obj) => {
                obj.iter().map(|(k, v)| k.len() + Self::estimate_json_size(v)).sum::<usize>() +
                mem::size_of::<serde_json::Map<String, serde_json::Value>>()
            },
        }
    }

    /// 更新内存使用统计
    async fn update_memory_usage(&self, queue_name: &str, size_delta: isize) {
        // 更新全局统计
        if size_delta > 0 {
            self.total_memory_usage.fetch_add(size_delta as usize, Ordering::Relaxed);
        } else {
            self.total_memory_usage.fetch_sub((-size_delta) as usize, Ordering::Relaxed);
        }
        
        // 更新队列级别统计
        let queues_read = self.queues.read().await;
        if let Some(channels) = queues_read.get(queue_name) {
            if size_delta > 0 {
                channels.memory_usage_bytes.fetch_add(size_delta as usize, Ordering::Relaxed);
            } else {
                channels.memory_usage_bytes.fetch_sub((-size_delta) as usize, Ordering::Relaxed);
            }
        }
    }

    /// 获取队列统计信息
    pub async fn get_queue_stats(&self) -> QueueStats {
        let queues = self.queues.read().await;
        let mut stats = QueueStats::default();
        
        for (name, channels) in queues.iter() {
            let size = channels.size.load(std::sync::atomic::Ordering::Relaxed);
            let memory_bytes = channels.memory_usage_bytes.load(std::sync::atomic::Ordering::Relaxed);
            let last_accessed = Duration::from_secs(
                channels.last_accessed.load(std::sync::atomic::Ordering::Relaxed)
            );
            let age = channels.created_at.elapsed();
            
            stats.total_queues += 1;
            stats.total_messages += size as usize;
            stats.queue_details.push(QueueDetail {
                name: name.clone(),
                size: size as usize,
                memory_bytes,
                age,
                last_accessed,
                durable: channels._durable,
            });
        }
        
        stats.estimated_memory_mb = self.estimate_memory_usage();
        stats.total_memory_bytes = self.total_memory_usage.load(Ordering::Relaxed);
        stats
    }

    /// 强制垃圾回收（清理空队列和释放内存）
    pub async fn force_gc(&self) -> GcStats {
        let start_time = Instant::now();
        let mut gc_stats = GcStats::default();
        
        // 记录GC前的内存使用量
        let memory_before = self.total_memory_usage.load(Ordering::Relaxed);
        
        // 清理空闲队列
        let cleanup_result = Self::cleanup_idle_queues_with_stats(&self.queues, &self.config).await;
        match cleanup_result {
            Ok(cleanup_stats) => {
                gc_stats.idle_queues_cleaned = cleanup_stats.queues_removed;
                gc_stats.memory_freed_mb = cleanup_stats.memory_freed_bytes / (1024 * 1024);
            }
            Err(e) => {
                warn!("Failed to cleanup idle queues during GC: {}", e);
            }
        }
        
        // 记录GC后的内存使用量
        let memory_after = self.total_memory_usage.load(Ordering::Relaxed);
        gc_stats.memory_freed_mb = (memory_before.saturating_sub(memory_after)) / (1024 * 1024);
        
        gc_stats.duration = start_time.elapsed();
        info!("Forced GC completed: {:?}", gc_stats);
        gc_stats
    }

    /// 带统计信息的清理空闲队列
    async fn cleanup_idle_queues_with_stats(
        queues: &Arc<RwLock<HashMap<String, QueueChannels>>>,
        config: &InMemoryQueueConfig,
    ) -> SchedulerResult<CleanupStats> {
        let now = Instant::now();
        let idle_threshold = Duration::from_secs(config.idle_timeout_seconds);
        let mut to_remove = Vec::new();
        let mut total_memory_freed = 0;
        
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
                        let memory_bytes = channels.memory_usage_bytes.load(std::sync::atomic::Ordering::Relaxed);
                        to_remove.push((queue_name.clone(), memory_bytes));
                        total_memory_freed += memory_bytes;
                    }
                }
            }
        }
        
        let queues_removed = to_remove.len();
        if !to_remove.is_empty() {
            let mut queues_write = queues.write().await;
            for (queue_name, _) in &to_remove {
                if let Some(channels) = queues_write.remove(queue_name) {
                    drop(channels.sender);
                    info!("Cleaned up idle queue: {}", queue_name);
                }
            }
            info!("Cleaned up {} idle queues, freed {} bytes", queues_removed, total_memory_freed);
        }
        
        Ok(CleanupStats {
            queues_removed,
            memory_freed_bytes: total_memory_freed,
        })
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
                memory_usage_bytes: Arc::new(AtomicUsize::new(0)),
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

        // 计算消息大小
        let message_size = Self::calculate_message_size(message);
        
        // 获取发送端并发送消息
        let sender = self.get_sender(queue).await?;
        
        sender.send(message.clone()).map_err(|e| {
            error!("Failed to send message to queue '{}': {}", queue, e);
            scheduler_errors::SchedulerError::MessageQueue(format!(
                "Failed to send message to queue '{}': {}",
                queue, e
            ))
        })?;

        // 更新队列大小、内存使用量和访问时间
        self.increment_queue_size(queue).await;
        self.update_memory_usage(queue, message_size as isize).await;
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

        // 批量更新队列大小计数、内存使用量和释放背压许可
        if !messages.is_empty() {
            if let Some(size_counter) = self.queues.read().await.get(queue).map(|q| q.size.clone()) {
                let mut total_memory_freed = 0;
                for message in &messages {
                    size_counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    // 释放背压许可
                    semaphore.add_permits(1);
                    // 计算释放的内存
                    total_memory_freed += Self::calculate_message_size(message);
                }
                
                // 更新内存使用统计
                if total_memory_freed > 0 {
                    self.update_memory_usage(queue, -(total_memory_freed as isize)).await;
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
        let mut purged_messages = Vec::new();

        // 清空队列中的所有消息
        {
            let mut rx = receiver.lock().await;
            while let Ok(message) = rx.try_recv() {
                purged_messages.push(message);
            }
        }

        // 计算释放的内存
        let mut total_memory_freed = 0;
        for message in &purged_messages {
            total_memory_freed += Self::calculate_message_size(message);
        }

        // 重置队列大小计数和内存使用量
        if let Some(channels) = self.queues.read().await.get(queue) {
            channels.size.store(0, std::sync::atomic::Ordering::Relaxed);
            channels.memory_usage_bytes.store(0, std::sync::atomic::Ordering::Relaxed);
        }

        // 更新全局内存统计
        if total_memory_freed > 0 {
            self.total_memory_usage.fetch_sub(total_memory_freed, Ordering::Relaxed);
        }

        info!("Purged {} messages from queue '{}', freed {} bytes", 
              purged_messages.len(), queue, total_memory_freed);
        Ok(())
    }
}

impl Default for InMemoryMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryMessageQueue {
    /// 获取详细的内存使用信息
    pub async fn get_memory_info(&self) -> MemoryInfo {
        let queues = self.queues.read().await;
        let mut queue_memory_details = Vec::new();
        let mut total_queue_memory = 0;
        
        for (name, channels) in queues.iter() {
            let memory_bytes = channels.memory_usage_bytes.load(Ordering::Relaxed);
            let message_count = channels.size.load(Ordering::Relaxed) as usize;
            total_queue_memory += memory_bytes;
            
            queue_memory_details.push(QueueMemoryDetail {
                queue_name: name.clone(),
                memory_bytes,
                message_count,
                average_message_size: if message_count > 0 { memory_bytes / message_count } else { 0 },
            });
        }
        
        let base_overhead = self.calculate_base_overhead();
        let total_memory = self.total_memory_usage.load(Ordering::Relaxed);
        
        MemoryInfo {
            total_memory_bytes: total_memory,
            total_memory_mb: (total_memory as f64) / (1024.0 * 1024.0),
            queue_memory_bytes: total_queue_memory,
            base_overhead_bytes: base_overhead,
            queue_count: queues.len(),
            queue_details: queue_memory_details,
        }
    }
}

/// 内存使用详细信息
#[derive(Debug)]
pub struct MemoryInfo {
    /// 总内存使用量（字节）
    pub total_memory_bytes: usize,
    /// 总内存使用量（MB）
    pub total_memory_mb: f64,
    /// 队列消息内存使用量（字节）
    pub queue_memory_bytes: usize,
    /// 基础开销（字节）
    pub base_overhead_bytes: usize,
    /// 队列数量
    pub queue_count: usize,
    /// 队列内存详情
    pub queue_details: Vec<QueueMemoryDetail>,
}

/// 队列内存详情
#[derive(Debug)]
pub struct QueueMemoryDetail {
    /// 队列名称
    pub queue_name: String,
    /// 内存使用量（字节）
    pub memory_bytes: usize,
    /// 消息数量
    pub message_count: usize,
    /// 平均消息大小（字节）
    pub average_message_size: usize,
}

impl MemoryInfo {
    /// 获取内存使用效率（消息内存 / 总内存）
    pub fn memory_efficiency(&self) -> f64 {
        if self.total_memory_bytes > 0 {
            (self.queue_memory_bytes as f64) / (self.total_memory_bytes as f64)
        } else {
            0.0
        }
    }
    
    /// 获取开销比例（基础开销 / 总内存）
    pub fn overhead_ratio(&self) -> f64 {
        if self.total_memory_bytes > 0 {
            (self.base_overhead_bytes as f64) / (self.total_memory_bytes as f64)
        } else {
            0.0
        }
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
    /// 精确内存使用量（字节）
    pub total_memory_bytes: usize,
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
    /// 队列内存使用量（字节）
    pub memory_bytes: usize,
    /// 队列年龄
    pub age: Duration,
    /// 最后访问时间
    pub last_accessed: Duration,
    /// 是否持久化
    pub durable: bool,
}

impl QueueDetail {
    /// 获取队列内存使用量（MB）
    pub fn memory_mb(&self) -> f64 {
        self.memory_bytes as f64 / (1024.0 * 1024.0)
    }
    
    /// 获取平均每条消息的内存使用量（字节）
    pub fn average_message_size(&self) -> usize {
        if self.size > 0 {
            self.memory_bytes / self.size
        } else {
            0
        }
    }
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

/// 清理统计
#[derive(Debug, Default)]
struct CleanupStats {
    /// 移除的队列数
    queues_removed: usize,
    /// 释放的内存字节数
    memory_freed_bytes: usize,
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

    #[tokio::test]
    async fn test_accurate_memory_calculation() {
        let queue = InMemoryMessageQueue::new();
        let queue_name = "memory_test_queue";
        
        // 创建队列
        queue.create_queue(queue_name, false).await.unwrap();
        
        // 获取初始内存统计
        let initial_stats = queue.get_queue_stats().await;
        let initial_memory = queue.get_memory_info().await;
        
        println!("初始内存使用: {} bytes", initial_memory.total_memory_bytes);
        assert_eq!(initial_stats.total_messages, 0);
        assert_eq!(initial_stats.total_memory_bytes, 0); // 没有消息时应该为0
        
        // 创建不同大小的测试消息
        let small_msg = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "small".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({"cmd": "echo"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };
        
        let large_msg = TaskExecutionMessage {
            task_run_id: 2,
            task_id: 2,
            task_name: "large_task_with_very_long_name_for_testing_memory_calculation".to_string(),
            task_type: "python_script_execution".to_string(),
            parameters: serde_json::json!({
                "script": "print('Hello, World!')",
                "args": ["--verbose", "--output", "/tmp/result.txt"],
                "env": {
                    "PYTHONPATH": "/usr/local/lib/python3.9",
                    "DEBUG": "true",
                    "LARGE_DATA": (0..100).map(|i| format!("data_{}", i)).collect::<Vec<_>>()
                }
            }),
            timeout_seconds: 3600,
            retry_count: 3,
            shard_index: Some(1),
            shard_total: Some(4),
        };
        
        let small_message = Message::task_execution(small_msg);
        let large_message = Message::task_execution(large_msg);
        
        // 发布小消息
        queue.publish_message(queue_name, &small_message).await.unwrap();
        
        let stats_after_small = queue.get_queue_stats().await;
        let memory_after_small = queue.get_memory_info().await;
        
        println!("发布小消息后内存使用: {} bytes", memory_after_small.total_memory_bytes);
        assert_eq!(stats_after_small.total_messages, 1);
        assert!(stats_after_small.total_memory_bytes > 0);
        
        // 验证队列级别的内存统计
        let queue_detail = &stats_after_small.queue_details[0];
        assert_eq!(queue_detail.name, queue_name);
        assert_eq!(queue_detail.size, 1);
        assert!(queue_detail.memory_bytes > 0);
        assert!(queue_detail.average_message_size() > 0);
        
        let small_message_memory = stats_after_small.total_memory_bytes;
        
        // 发布大消息
        queue.publish_message(queue_name, &large_message).await.unwrap();
        
        let stats_after_large = queue.get_queue_stats().await;
        let memory_after_large = queue.get_memory_info().await;
        
        println!("发布大消息后内存使用: {} bytes", memory_after_large.total_memory_bytes);
        assert_eq!(stats_after_large.total_messages, 2);
        assert!(stats_after_large.total_memory_bytes > small_message_memory);
        
        // 验证大消息确实比小消息占用更多内存
        let large_message_memory = stats_after_large.total_memory_bytes - small_message_memory;
        println!("小消息内存: {} bytes, 大消息内存: {} bytes", small_message_memory, large_message_memory);
        assert!(large_message_memory > small_message_memory, "大消息应该比小消息占用更多内存");
        
        // 测试内存效率计算
        assert!(memory_after_large.memory_efficiency() > 0.0);
        assert!(memory_after_large.overhead_ratio() >= 0.0);
        
        // 消费消息并验证内存释放
        let consumed_messages = queue.consume_messages(queue_name).await.unwrap();
        assert_eq!(consumed_messages.len(), 2);
        
        let stats_after_consume = queue.get_queue_stats().await;
        let memory_after_consume = queue.get_memory_info().await;
        
        println!("消费消息后内存使用: {} bytes", memory_after_consume.total_memory_bytes);
        assert_eq!(stats_after_consume.total_messages, 0);
        assert_eq!(stats_after_consume.total_memory_bytes, 0); // 消费后应该释放所有消息内存
        
        // 验证队列级别的内存统计也被重置
        let queue_detail_after_consume = &stats_after_consume.queue_details[0];
        assert_eq!(queue_detail_after_consume.size, 0);
        assert_eq!(queue_detail_after_consume.memory_bytes, 0);
        assert_eq!(queue_detail_after_consume.average_message_size(), 0);
    }

    #[tokio::test]
    async fn test_memory_calculation_with_purge() {
        let queue = InMemoryMessageQueue::new();
        let queue_name = "purge_test_queue";
        
        // 创建队列并发布多条消息
        queue.create_queue(queue_name, false).await.unwrap();
        
        for i in 0..3 {
            let execution_msg = TaskExecutionMessage {
                task_run_id: i,
                task_id: i,
                task_name: format!("test_task_{}", i),
                task_type: "shell".to_string(),
                parameters: serde_json::json!({"data": format!("test_data_{}", i)}),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };
            let message = Message::task_execution(execution_msg);
            queue.publish_message(queue_name, &message).await.unwrap();
        }
        
        // 验证内存使用
        let stats_before_purge = queue.get_queue_stats().await;
        assert_eq!(stats_before_purge.total_messages, 3);
        assert!(stats_before_purge.total_memory_bytes > 0);
        
        let memory_before_purge = stats_before_purge.total_memory_bytes;
        
        // 清空队列
        queue.purge_queue(queue_name).await.unwrap();
        
        // 验证内存被释放
        let stats_after_purge = queue.get_queue_stats().await;
        assert_eq!(stats_after_purge.total_messages, 0);
        assert_eq!(stats_after_purge.total_memory_bytes, 0);
        
        println!("清空前内存: {} bytes, 清空后内存: {} bytes", 
                memory_before_purge, stats_after_purge.total_memory_bytes);
    }
}