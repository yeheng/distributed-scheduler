use async_trait::async_trait;
use metrics::{counter, gauge, histogram};
use redis::{Client, Connection, RedisResult};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use scheduler_core::{models::Message, traits::MessageQueue, Result, SchedulerError};

/// Redis Stream消息队列实现
///
/// 这是一个基于Redis Stream的轻量级消息队列实现，完全兼容MessageQueue trait。
/// 它提供了与RabbitMQ相同的功能，但具有更低的资源消耗和更简单的部署要求。
///
/// # 特性
///
/// - **连接管理**: 高效的Redis连接管理
/// - **消费者组**: 支持多个消费者实例的负载均衡
/// - **消息持久化**: 消息存储在Redis Stream中，支持故障恢复
/// - **性能监控**: 内置性能指标收集和监控
/// - **重试机制**: 支持消息发布和处理的自动重试
/// - **消息确认**: 支持消息的确认和拒绝机制
///
/// # 使用示例
///
/// ```rust,no_run
/// use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
/// use scheduler_core::traits::MessageQueue;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = RedisStreamConfig::default();
///     let queue = RedisStreamMessageQueue::new(config).await?;
///     
///     // 创建队列
///     queue.create_queue("test_queue", true).await?;
///     
///     // 发布消息
///     let message = /* 创建消息 */;
///     queue.publish_message("test_queue", &message).await?;
///     
///     // 消费消息
///     let messages = queue.consume_messages("test_queue").await?;
///     
///     // 确认消息
///     for message in messages {
///         queue.ack_message(&message.id).await?;
///     }
///     
///     Ok(())
/// }
/// ```
pub struct RedisStreamMessageQueue {
    client: Client,
    config: RedisStreamConfig,
    consumer_group_prefix: String,
    consumer_id: String,
    // 存储Message ID到Redis Stream ID和队列名的映射
    message_id_mapping:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, (String, String)>>>,
    // 性能监控指标
    metrics: RedisStreamMetrics,
}

/// Redis Stream性能监控指标
///
/// 用于收集和跟踪Redis Stream消息队列的性能数据。
/// 所有计数器都是原子操作，确保在多线程环境下的数据一致性。
///
/// # 指标说明
///
/// - `messages_published`: 已发布的消息总数
/// - `messages_consumed`: 已消费的消息总数
/// - `messages_acked`: 已确认的消息总数
/// - `messages_nacked`: 已拒绝的消息总数
/// - `connection_errors`: 连接错误总数
/// - `active_connections`: 当前活跃连接数
#[derive(Debug, Clone)]
pub struct RedisStreamMetrics {
    pub messages_published: Arc<AtomicU64>,
    pub messages_consumed: Arc<AtomicU64>,
    pub messages_acked: Arc<AtomicU64>,
    pub messages_nacked: Arc<AtomicU64>,
    pub connection_errors: Arc<AtomicU64>,
    pub active_connections: Arc<AtomicU32>,
}

impl Default for RedisStreamMetrics {
    fn default() -> Self {
        Self {
            messages_published: Arc::new(AtomicU64::new(0)),
            messages_consumed: Arc::new(AtomicU64::new(0)),
            messages_acked: Arc::new(AtomicU64::new(0)),
            messages_nacked: Arc::new(AtomicU64::new(0)),
            connection_errors: Arc::new(AtomicU64::new(0)),
            active_connections: Arc::new(AtomicU32::new(0)),
        }
    }
}

/// Redis Stream配置
///
/// 包含连接Redis服务器和配置消息队列行为所需的所有参数。
///
/// # 字段说明
///
/// - `host`: Redis服务器主机地址
/// - `port`: Redis服务器端口号
/// - `database`: Redis数据库编号
/// - `password`: Redis认证密码（可选）
/// - `connection_timeout_seconds`: 连接超时时间（秒）
/// - `max_retry_attempts`: 最大重试次数
/// - `retry_delay_seconds`: 重试延迟时间（秒）
/// - `consumer_group_prefix`: 消费者组名称前缀
/// - `consumer_id`: 消费者ID
/// - `pool_min_idle`: 连接池最小空闲连接数
/// - `pool_max_open`: 连接池最大连接数
/// - `pool_timeout_seconds`: 连接池获取连接超时时间（秒）
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

impl RedisStreamMessageQueue {
    /// 创建新的Redis Stream消息队列实例
    ///
    /// 此方法会建立到Redis服务器的连接，并初始化所有必要的组件。
    ///
    /// # 参数
    ///
    /// * `config` - Redis Stream配置
    ///
    /// # 返回值
    ///
    /// 成功时返回`RedisStreamMessageQueue`实例，失败时返回错误。
    ///
    /// # 错误
    ///
    /// 如果无法连接到Redis服务器或配置无效，将返回`SchedulerError::MessageQueue`错误。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RedisStreamConfig {
    ///         host: "localhost".to_string(),
    ///         port: 6379,
    ///         database: 0,
    ///         password: None,
    ///         ..RedisStreamConfig::default()
    ///     };
    ///     
    ///     let queue = RedisStreamMessageQueue::new(config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(config: RedisStreamConfig) -> Result<Self> {
        let redis_url = if let Some(password) = &config.password {
            format!(
                "redis://:{}@{}:{}/{}",
                password, config.host, config.port, config.database
            )
        } else {
            format!(
                "redis://{}:{}/{}",
                config.host, config.port, config.database
            )
        };

        let client = Client::open(redis_url).map_err(|e| {
            SchedulerError::MessageQueue(format!("Failed to create Redis client: {e}"))
        })?;

        let consumer_group_prefix = config.consumer_group_prefix.clone();
        let consumer_id = config.consumer_id.clone();

        info!("Created Redis Stream message queue with optimized configuration (timeout: {}s, max_retries: {})", 
              config.connection_timeout_seconds, config.max_retry_attempts);

        Ok(Self {
            client,
            config,
            consumer_group_prefix,
            consumer_id,
            message_id_mapping: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            metrics: RedisStreamMetrics::default(),
        })
    }

    /// 获取Redis连接
    async fn get_connection(&self) -> Result<Connection> {
        let start = Instant::now();
        debug!(
            "Creating Redis connection to {}:{}",
            self.config.host, self.config.port
        );

        // 使用tokio::time::timeout来限制连接时间
        let connection_future = tokio::task::spawn_blocking({
            let client = self.client.clone();
            move || client.get_connection()
        });

        let connection = tokio::time::timeout(
            Duration::from_secs(self.config.connection_timeout_seconds),
            connection_future,
        )
        .await
        .map_err(|_| {
            self.metrics
                .connection_errors
                .fetch_add(1, Ordering::Relaxed);
            SchedulerError::MessageQueue(format!(
                "Connection timeout after {} seconds",
                self.config.connection_timeout_seconds
            ))
        })?
        .map_err(|e| {
            self.metrics
                .connection_errors
                .fetch_add(1, Ordering::Relaxed);
            SchedulerError::MessageQueue(format!("Failed to spawn connection task: {e}"))
        })?
        .map_err(|e| {
            self.metrics
                .connection_errors
                .fetch_add(1, Ordering::Relaxed);
            SchedulerError::MessageQueue(format!("Failed to connect to Redis: {e}"))
        })?;

        // 记录连接获取时间
        let duration = start.elapsed();
        histogram!("redis_stream_connection_acquire_duration_ms")
            .record(duration.as_millis() as f64);

        // 更新活跃连接数
        self.metrics
            .active_connections
            .fetch_add(1, Ordering::Relaxed);
        gauge!("redis_stream_active_connections")
            .set(self.metrics.active_connections.load(Ordering::Relaxed) as f64);

        debug!(
            "Successfully connected to Redis at {}:{} in {:?}",
            self.config.host, self.config.port, duration
        );
        Ok(connection)
    }

    /// 确保Stream存在
    async fn ensure_stream_exists(&self, stream_key: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;

        // 使用XINFO STREAM检查Stream是否存在，这比EXISTS更准确
        let result: RedisResult<redis::Value> = redis::cmd("XINFO")
            .arg("STREAM")
            .arg(stream_key)
            .query(&mut conn);

        match result {
            Ok(_) => {
                debug!("Stream {} already exists", stream_key);
                Ok(())
            }
            Err(_) => {
                debug!("Creating new Redis Stream: {}", stream_key);
                // Stream不存在，创建一个初始条目
                let _: String = redis::cmd("XADD")
                    .arg(stream_key)
                    .arg("*")
                    .arg("_init")
                    .arg("stream_created")
                    .query(&mut conn)
                    .map_err(|e| {
                        SchedulerError::MessageQueue(format!("Failed to create stream: {e}"))
                    })?;

                info!("Created Redis Stream: {}", stream_key);
                Ok(())
            }
        }
    }

    /// 确保消费者组存在
    async fn ensure_consumer_group_exists(&self, stream_key: &str, group_name: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;

        // 首先确保Stream存在
        self.ensure_stream_exists(stream_key).await?;

        // 尝试创建消费者组，如果已存在会返回错误，我们忽略这个错误
        let result: RedisResult<String> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream_key)
            .arg(group_name)
            .arg("0") // 从Stream开始处读取
            .arg("MKSTREAM") // 如果Stream不存在则创建
            .query(&mut conn);

        match result {
            Ok(_) => {
                info!(
                    "Created consumer group: {} for stream: {}",
                    group_name, stream_key
                );
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("BUSYGROUP") {
                    debug!(
                        "Consumer group {} already exists for stream {}",
                        group_name, stream_key
                    );
                } else {
                    return Err(SchedulerError::MessageQueue(format!(
                        "Failed to create consumer group {group_name}: {e}"
                    )));
                }
            }
        }

        Ok(())
    }

    /// 处理待处理的消息（已读取但未确认的消息）
    async fn consume_pending_messages(&self, queue: &str) -> Result<Vec<Message>> {
        debug!("Consuming pending messages from queue: {}", queue);

        let group_name = self.get_consumer_group_name(queue);
        let mut conn = self.get_connection().await?;

        // 使用XREADGROUP读取待处理的消息
        let result: Vec<Vec<redis::Value>> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&group_name)
            .arg(&self.consumer_id)
            .arg("COUNT")
            .arg(100)
            .arg("STREAMS")
            .arg(queue)
            .arg("0") // 从待处理消息开始读取
            .query(&mut conn)
            .map_err(|e| {
                SchedulerError::MessageQueue(format!(
                    "Failed to consume pending messages from queue {queue}: {e}"
                ))
            })?;

        self.parse_stream_messages(result, "pending", queue).await
    }

    /// 消费新消息（未被任何消费者读取的消息）
    async fn consume_new_messages(&self, queue: &str) -> Result<Vec<Message>> {
        let group_name = self.get_consumer_group_name(queue);
        let mut conn = self.get_connection().await?;

        // 从消费者组读取新消息
        let result: Vec<Vec<redis::Value>> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&group_name)
            .arg(&self.consumer_id)
            .arg("COUNT")
            .arg(100) // 一次最多读取100条消息
            .arg("BLOCK")
            .arg(1000) // 阻塞1秒等待新消息
            .arg("STREAMS")
            .arg(queue)
            .arg(">") // 只读取新消息
            .query(&mut conn)
            .map_err(|e| {
                SchedulerError::MessageQueue(format!(
                    "Failed to consume new messages from queue {queue}: {e}"
                ))
            })?;

        self.parse_stream_messages(result, "new", queue).await
    }

    /// 解析Redis Stream消息响应
    async fn parse_stream_messages(
        &self,
        result: Vec<Vec<redis::Value>>,
        message_type: &str,
        queue: &str,
    ) -> Result<Vec<Message>> {
        let start = Instant::now();
        let mut messages = Vec::new();

        // 解析Redis响应
        for stream_data in result {
            if stream_data.len() >= 2 {
                // stream_data[0] 是stream名称，stream_data[1] 是消息列表
                if let redis::Value::Bulk(entries) = &stream_data[1] {
                    // 解析Stream条目
                    for entry in entries {
                        if let redis::Value::Bulk(entry_parts) = entry {
                            if entry_parts.len() >= 2 {
                                // entry_parts[0] 是消息ID，entry_parts[1] 是字段列表
                                let stream_message_id = match &entry_parts[0] {
                                    redis::Value::Data(id_bytes) => {
                                        String::from_utf8_lossy(id_bytes).to_string()
                                    }
                                    _ => continue,
                                };

                                if let redis::Value::Bulk(fields) = &entry_parts[1] {
                                    // 解析字段键值对
                                    let mut field_map = std::collections::HashMap::new();

                                    for i in (0..fields.len()).step_by(2) {
                                        if i + 1 < fields.len() {
                                            let key = match &fields[i] {
                                                redis::Value::Data(key_bytes) => {
                                                    String::from_utf8_lossy(key_bytes).to_string()
                                                }
                                                _ => continue,
                                            };

                                            let value = match &fields[i + 1] {
                                                redis::Value::Data(value_bytes) => {
                                                    String::from_utf8_lossy(value_bytes).to_string()
                                                }
                                                _ => continue,
                                            };

                                            field_map.insert(key, value);
                                        }
                                    }

                                    // 从字段中提取消息数据
                                    if let Some(message_data) = field_map.get("data") {
                                        match self.deserialize_message(message_data) {
                                            Ok(mut message) => {
                                                debug!(
                                                    "Consumed {} message: {} from stream ID: {}",
                                                    message_type, message.id, stream_message_id
                                                );

                                                // 更新重试次数（如果字段中有的话）
                                                if let Some(retry_count_str) =
                                                    field_map.get("retry_count")
                                                {
                                                    if let Ok(retry_count) =
                                                        retry_count_str.parse::<i32>()
                                                    {
                                                        message.retry_count = retry_count;
                                                    }
                                                }

                                                // 存储Message ID到Redis Stream ID和队列名的映射，用于后续的ACK/NACK操作
                                                if let Ok(mut mapping) =
                                                    self.message_id_mapping.lock()
                                                {
                                                    mapping.insert(
                                                        message.id.clone(),
                                                        (
                                                            stream_message_id.clone(),
                                                            queue.to_string(),
                                                        ),
                                                    );
                                                }

                                                messages.push(message);
                                            }
                                            Err(e) => {
                                                warn!("Failed to deserialize {} message from stream ID {}: {}", message_type, stream_message_id, e);
                                                counter!(
                                                    "redis_stream_deserialization_errors_total"
                                                )
                                                .increment(1);
                                                // 继续处理其他消息，不因为一个消息反序列化失败而停止
                                            }
                                        }
                                    } else {
                                        warn!(
                                            "Stream message {} missing 'data' field",
                                            stream_message_id
                                        );
                                        counter!("redis_stream_malformed_messages_total")
                                            .increment(1);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 记录性能指标
        let duration = start.elapsed();
        let message_count = messages.len() as u64;
        self.metrics
            .messages_consumed
            .fetch_add(message_count, Ordering::Relaxed);
        histogram!("redis_stream_parse_duration_ms").record(duration.as_millis() as f64);
        counter!("redis_stream_messages_consumed_total").increment(message_count);

        debug!(
            "Parsed {} {} messages from queue {} in {:?}",
            message_count, message_type, queue, duration
        );
        Ok(messages)
    }

    /// 获取消费者组信息
    async fn get_consumer_group_info(&self, queue: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let group_name = self.get_consumer_group_name(queue);

        // 获取消费者组信息
        let result: RedisResult<redis::Value> = redis::cmd("XINFO")
            .arg("GROUPS")
            .arg(queue)
            .query(&mut conn);

        match result {
            Ok(groups_info) => {
                debug!("Consumer groups for stream {}: {:?}", queue, groups_info);
            }
            Err(e) => {
                debug!("Failed to get consumer group info for {}: {}", queue, e);
            }
        }

        // 获取消费者信息
        let result: RedisResult<redis::Value> = redis::cmd("XINFO")
            .arg("CONSUMERS")
            .arg(queue)
            .arg(&group_name)
            .query(&mut conn);

        match result {
            Ok(consumers_info) => {
                debug!(
                    "Consumers in group {} for stream {}: {:?}",
                    group_name, queue, consumers_info
                );
            }
            Err(e) => {
                debug!(
                    "Failed to get consumer info for group {} in stream {}: {}",
                    group_name, queue, e
                );
            }
        }

        Ok(())
    }

    /// 获取消费者组名称
    fn get_consumer_group_name(&self, queue: &str) -> String {
        format!("{}_{}_group", self.consumer_group_prefix, queue)
    }

    /// 序列化消息
    fn serialize_message(&self, message: &Message) -> Result<String> {
        serde_json::to_string(message).map_err(|e| {
            SchedulerError::Serialization(format!(
                "Failed to serialize message {}: {}",
                message.id, e
            ))
        })
    }

    /// 反序列化消息
    fn deserialize_message(&self, data: &str) -> Result<Message> {
        serde_json::from_str(data).map_err(|e| {
            SchedulerError::Serialization(format!("Failed to deserialize message data: {e}"))
        })
    }

    /// 验证队列名称
    fn validate_queue_name(&self, queue: &str) -> Result<()> {
        if queue.is_empty() {
            return Err(SchedulerError::MessageQueue(
                "Queue name cannot be empty".to_string(),
            ));
        }

        if queue.len() > 255 {
            return Err(SchedulerError::MessageQueue(
                "Queue name too long (max 255 characters)".to_string(),
            ));
        }

        // Redis Stream键名不能包含某些特殊字符
        if queue.contains(' ') || queue.contains('\n') || queue.contains('\r') {
            return Err(SchedulerError::MessageQueue(
                "Queue name contains invalid characters".to_string(),
            ));
        }

        Ok(())
    }

    /// 检查Redis连接健康状态
    async fn check_connection_health(&self) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: String = redis::cmd("PING")
            .query(&mut conn)
            .map_err(|e| SchedulerError::MessageQueue(format!("Redis health check failed: {e}")))?;
        Ok(())
    }

    /// 带重试机制的消息发布
    async fn publish_message_with_retry(&self, queue: &str, message: &Message) -> Result<()> {
        let mut last_error = None;

        for attempt in 1..=self.config.max_retry_attempts {
            match self.try_publish_message(queue, message).await {
                Ok(()) => {
                    if attempt > 1 {
                        info!(
                            "Successfully published message {} to queue {} after {} attempts",
                            message.id, queue, attempt
                        );
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retry_attempts {
                        warn!("Failed to publish message {} to queue {} (attempt {}/{}): {}. Retrying...", 
                              message.id, queue, attempt, self.config.max_retry_attempts, last_error.as_ref().unwrap());

                        // 指数退避延迟
                        let delay =
                            Duration::from_secs(self.config.retry_delay_seconds * attempt as u64);
                        sleep(delay).await;
                    }
                }
            }
        }

        let error_msg = format!(
            "Failed to publish message {} to queue {} after {} attempts. Last error: {}",
            message.id,
            queue,
            self.config.max_retry_attempts,
            last_error.unwrap_or_else(|| SchedulerError::MessageQueue("Unknown error".to_string()))
        );
        error!("{}", error_msg);
        Err(SchedulerError::MessageQueue(error_msg))
    }

    /// 尝试发布消息（单次尝试）
    async fn try_publish_message(&self, queue: &str, message: &Message) -> Result<()> {
        let start = Instant::now();

        // 确保Stream存在
        self.ensure_stream_exists(queue).await?;

        // 序列化消息
        let serialized_message = self.serialize_message(message)?;

        // 发布消息到Redis Stream
        let mut conn = self.get_connection().await?;
        let stream_message_id: String = redis::cmd("XADD")
            .arg(queue)
            .arg("*") // 让Redis自动生成ID
            .arg("message_id")
            .arg(&message.id)
            .arg("data")
            .arg(&serialized_message)
            .arg("timestamp")
            .arg(message.timestamp.timestamp())
            .arg("retry_count")
            .arg(message.retry_count)
            .arg("correlation_id")
            .arg(message.correlation_id.as_deref().unwrap_or(""))
            .query(&mut conn)
            .map_err(|e| {
                self.metrics
                    .connection_errors
                    .fetch_add(1, Ordering::Relaxed);
                counter!("redis_stream_publish_errors_total").increment(1);
                SchedulerError::MessageQueue(format!(
                    "Failed to publish message to Redis Stream: {e}"
                ))
            })?;

        // 记录性能指标
        let duration = start.elapsed();
        self.metrics
            .messages_published
            .fetch_add(1, Ordering::Relaxed);
        histogram!("redis_stream_publish_duration_ms").record(duration.as_millis() as f64);
        counter!("redis_stream_messages_published_total").increment(1);

        debug!(
            "Published message {} to stream {} with Redis Stream ID: {} in {:?}",
            message.id, queue, stream_message_id, duration
        );
        Ok(())
    }
}

#[async_trait]
impl MessageQueue for RedisStreamMessageQueue {
    async fn publish_message(&self, queue: &str, message: &Message) -> Result<()> {
        debug!("Publishing message {} to queue: {}", message.id, queue);

        // 验证队列名称
        self.validate_queue_name(queue)?;

        // 验证消息内容
        if message.id.is_empty() {
            return Err(SchedulerError::MessageQueue(
                "Message ID cannot be empty".to_string(),
            ));
        }

        // 使用重试机制发布消息
        self.publish_message_with_retry(queue, message).await
    }

    async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>> {
        debug!("Consuming messages from queue: {}", queue);

        // 验证队列名称
        self.validate_queue_name(queue)?;

        // 确保Stream和消费者组存在
        self.ensure_stream_exists(queue).await?;
        let group_name = self.get_consumer_group_name(queue);
        self.ensure_consumer_group_exists(queue, &group_name)
            .await?;

        let mut all_messages = Vec::new();

        // 首先处理待处理的消息（已读取但未确认的消息）
        match self.consume_pending_messages(queue).await {
            Ok(mut pending_messages) => {
                debug!(
                    "Found {} pending messages in queue: {}",
                    pending_messages.len(),
                    queue
                );
                all_messages.append(&mut pending_messages);
            }
            Err(e) => {
                warn!(
                    "Failed to consume pending messages from queue {}: {}",
                    queue, e
                );
                // 继续处理新消息，不因为待处理消息失败而停止
            }
        }

        // 然后读取新消息
        match self.consume_new_messages(queue).await {
            Ok(mut new_messages) => {
                debug!(
                    "Found {} new messages in queue: {}",
                    new_messages.len(),
                    queue
                );
                all_messages.append(&mut new_messages);
            }
            Err(e) => {
                warn!("Failed to consume new messages from queue {}: {}", queue, e);
                // 如果没有待处理消息且新消息也失败，则返回错误
                if all_messages.is_empty() {
                    return Err(e);
                }
            }
        }

        debug!(
            "Consumed {} total messages from queue: {}",
            all_messages.len(),
            queue
        );
        Ok(all_messages)
    }

    async fn ack_message(&self, message_id: &str) -> Result<()> {
        let start = Instant::now();
        debug!("Acknowledging message: {}", message_id);

        // 从映射中获取Redis Stream ID和队列名
        let (stream_message_id, queue_name) = {
            let mapping = self.message_id_mapping.lock().map_err(|e| {
                SchedulerError::MessageQueue(format!("Failed to lock message mapping: {e}"))
            })?;

            mapping.get(message_id).cloned().ok_or_else(|| {
                SchedulerError::MessageQueue(format!(
                    "Message ID {message_id} not found in mapping"
                ))
            })?
        };

        // 获取消费者组名称
        let group_name = self.get_consumer_group_name(&queue_name);

        // 使用XACK确认消息
        let mut conn = self.get_connection().await?;
        let ack_count: i64 = redis::cmd("XACK")
            .arg(&queue_name)
            .arg(&group_name)
            .arg(&stream_message_id)
            .query(&mut conn)
            .map_err(|e| {
                self.metrics
                    .connection_errors
                    .fetch_add(1, Ordering::Relaxed);
                counter!("redis_stream_ack_errors_total").increment(1);
                SchedulerError::MessageQueue(format!(
                    "Failed to acknowledge message {message_id}: {e}"
                ))
            })?;

        // 记录性能指标
        let duration = start.elapsed();
        self.metrics.messages_acked.fetch_add(1, Ordering::Relaxed);
        histogram!("redis_stream_ack_duration_ms").record(duration.as_millis() as f64);
        counter!("redis_stream_messages_acked_total").increment(1);

        if ack_count > 0 {
            debug!(
                "Successfully acknowledged message {} (stream ID: {}) in {:?}",
                message_id, stream_message_id, duration
            );

            // 从映射中移除已确认的消息
            if let Ok(mut mapping) = self.message_id_mapping.lock() {
                mapping.remove(message_id);
            }
        } else {
            warn!(
                "Message {} (stream ID: {}) was not acknowledged (possibly already processed)",
                message_id, stream_message_id
            );
            counter!("redis_stream_ack_already_processed_total").increment(1);
        }

        Ok(())
    }

    async fn nack_message(&self, message_id: &str, requeue: bool) -> Result<()> {
        let start = Instant::now();
        debug!("Nacking message: {}, requeue: {}", message_id, requeue);

        // 从映射中获取Redis Stream ID和队列名
        let (stream_message_id, queue_name) = {
            let mapping = self.message_id_mapping.lock().map_err(|e| {
                SchedulerError::MessageQueue(format!("Failed to lock message mapping: {e}"))
            })?;

            mapping.get(message_id).cloned().ok_or_else(|| {
                SchedulerError::MessageQueue(format!(
                    "Message ID {message_id} not found in mapping"
                ))
            })?
        };

        let mut conn = self.get_connection().await?;

        if requeue {
            // 如果需要重新入队，先获取原始消息数据
            let group_name = self.get_consumer_group_name(&queue_name);

            // 使用XPENDING获取消息详情（用于验证消息存在）
            let _pending_info: Vec<redis::Value> = redis::cmd("XPENDING")
                .arg(&queue_name)
                .arg(&group_name)
                .arg(&stream_message_id)
                .arg(&stream_message_id)
                .arg(1)
                .query(&mut conn)
                .map_err(|e| {
                    SchedulerError::MessageQueue(format!(
                        "Failed to get pending message info for {message_id}: {e}"
                    ))
                })?;

            // 获取原始消息内容
            let message_data: Vec<redis::Value> = redis::cmd("XRANGE")
                .arg(&queue_name)
                .arg(&stream_message_id)
                .arg(&stream_message_id)
                .query(&mut conn)
                .map_err(|e| {
                    SchedulerError::MessageQueue(format!(
                        "Failed to get message data for requeue {message_id}: {e}"
                    ))
                })?;

            // 解析消息内容并重新发布
            if let Some(redis::Value::Bulk(entry_parts)) = message_data.first() {
                if entry_parts.len() >= 2 {
                    if let redis::Value::Bulk(fields) = &entry_parts[1] {
                        // 解析字段
                        let mut field_map = std::collections::HashMap::new();
                        for i in (0..fields.len()).step_by(2) {
                            if i + 1 < fields.len() {
                                let key = match &fields[i] {
                                    redis::Value::Data(key_bytes) => {
                                        String::from_utf8_lossy(key_bytes).to_string()
                                    }
                                    _ => continue,
                                };
                                let value = match &fields[i + 1] {
                                    redis::Value::Data(value_bytes) => {
                                        String::from_utf8_lossy(value_bytes).to_string()
                                    }
                                    _ => continue,
                                };
                                field_map.insert(key, value);
                            }
                        }

                        // 重新发布消息，增加重试次数
                        if let Some(message_data) = field_map.get("data") {
                            if let Ok(mut message) = self.deserialize_message(message_data) {
                                message.retry_count += 1;

                                // 重新发布消息
                                let serialized_message = self.serialize_message(&message)?;
                                let _: String = redis::cmd("XADD")
                                    .arg(&queue_name)
                                    .arg("*")
                                    .arg("message_id")
                                    .arg(&message.id)
                                    .arg("data")
                                    .arg(&serialized_message)
                                    .arg("timestamp")
                                    .arg(message.timestamp.timestamp())
                                    .arg("retry_count")
                                    .arg(message.retry_count)
                                    .arg("correlation_id")
                                    .arg(message.correlation_id.as_deref().unwrap_or(""))
                                    .query(&mut conn)
                                    .map_err(|e| {
                                        SchedulerError::MessageQueue(format!(
                                            "Failed to requeue message {message_id}: {e}"
                                        ))
                                    })?;

                                debug!(
                                    "Successfully requeued message {} with retry count {}",
                                    message_id, message.retry_count
                                );
                            }
                        }
                    }
                }
            }
        }

        // 从消费者组中删除消息（无论是否重新入队）
        let group_name = self.get_consumer_group_name(&queue_name);
        let ack_count: i64 = redis::cmd("XACK")
            .arg(&queue_name)
            .arg(&group_name)
            .arg(&stream_message_id)
            .query(&mut conn)
            .map_err(|e| {
                SchedulerError::MessageQueue(format!(
                    "Failed to ack message for nack {message_id}: {e}"
                ))
            })?;

        // 记录性能指标
        let duration = start.elapsed();
        self.metrics.messages_nacked.fetch_add(1, Ordering::Relaxed);
        histogram!("redis_stream_nack_duration_ms").record(duration.as_millis() as f64);
        counter!("redis_stream_messages_nacked_total").increment(1);

        debug!(
            "Nacked message {} (stream ID: {}), ack_count: {}, requeued: {} in {:?}",
            message_id, stream_message_id, ack_count, requeue, duration
        );

        // 从映射中移除消息
        if let Ok(mut mapping) = self.message_id_mapping.lock() {
            mapping.remove(message_id);
        }

        Ok(())
    }

    async fn create_queue(&self, queue: &str, _durable: bool) -> Result<()> {
        debug!("Creating queue: {}", queue);

        // 验证队列名称
        self.validate_queue_name(queue)?;

        // 确保Stream和消费者组存在
        self.ensure_stream_exists(queue).await?;
        let group_name = self.get_consumer_group_name(queue);
        self.ensure_consumer_group_exists(queue, &group_name)
            .await?;

        info!("Created queue: {}", queue);
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> Result<()> {
        debug!("Deleting queue: {}", queue);

        // 验证队列名称
        self.validate_queue_name(queue)?;

        let mut conn = self.get_connection().await?;
        let _: i32 = redis::cmd("DEL")
            .arg(queue)
            .query(&mut conn)
            .map_err(|e| SchedulerError::MessageQueue(format!("Failed to delete queue: {e}")))?;

        info!("Deleted queue: {}", queue);
        Ok(())
    }

    async fn get_queue_size(&self, queue: &str) -> Result<u32> {
        debug!("Getting queue size for: {}", queue);

        // 验证队列名称
        self.validate_queue_name(queue)?;

        let mut conn = self.get_connection().await?;
        let size: i64 = redis::cmd("XLEN")
            .arg(queue)
            .query(&mut conn)
            .map_err(|e| SchedulerError::MessageQueue(format!("Failed to get queue size: {e}")))?;

        debug!("Queue {} size: {}", queue, size);
        Ok(size as u32)
    }

    async fn purge_queue(&self, queue: &str) -> Result<()> {
        debug!("Purging queue: {}", queue);

        // 验证队列名称
        self.validate_queue_name(queue)?;

        let mut conn = self.get_connection().await?;
        let _: i32 = redis::cmd("XTRIM")
            .arg(queue)
            .arg("MAXLEN")
            .arg(0)
            .query(&mut conn)
            .map_err(|e| SchedulerError::MessageQueue(format!("Failed to purge queue: {e}")))?;

        info!("Purged queue: {}", queue);
        Ok(())
    }
}

impl RedisStreamMessageQueue {
    /// 获取性能监控指标的引用
    ///
    /// 返回当前实例的性能监控指标，可用于实时监控消息队列的状态。
    ///
    /// # 返回值
    ///
    /// 返回`RedisStreamMetrics`的引用，包含所有性能计数器。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
    /// # use std::sync::atomic::Ordering;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = RedisStreamConfig::default();
    /// # let queue = RedisStreamMessageQueue::new(config).await?;
    /// let metrics = queue.get_metrics();
    /// let published_count = metrics.messages_published.load(Ordering::Relaxed);
    /// println!("Published messages: {}", published_count);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_metrics(&self) -> &RedisStreamMetrics {
        &self.metrics
    }

    /// 获取性能监控指标的快照
    ///
    /// 创建当前所有性能指标的快照，适用于日志记录、报告生成或监控系统集成。
    /// 快照是原子操作的结果，确保数据的一致性。
    ///
    /// # 返回值
    ///
    /// 返回`RedisStreamMetricsSnapshot`，包含所有指标的当前值。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = RedisStreamConfig::default();
    /// # let queue = RedisStreamMessageQueue::new(config).await?;
    /// let snapshot = queue.get_metrics_snapshot();
    /// println!("Messages published: {}", snapshot.messages_published);
    /// println!("Messages consumed: {}", snapshot.messages_consumed);
    /// println!("Active connections: {}", snapshot.active_connections);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_metrics_snapshot(&self) -> RedisStreamMetricsSnapshot {
        RedisStreamMetricsSnapshot {
            messages_published: self.metrics.messages_published.load(Ordering::Relaxed),
            messages_consumed: self.metrics.messages_consumed.load(Ordering::Relaxed),
            messages_acked: self.metrics.messages_acked.load(Ordering::Relaxed),
            messages_nacked: self.metrics.messages_nacked.load(Ordering::Relaxed),
            connection_errors: self.metrics.connection_errors.load(Ordering::Relaxed),
            active_connections: self.metrics.active_connections.load(Ordering::Relaxed),
        }
    }

    /// 重置性能监控指标
    pub fn reset_metrics(&self) {
        self.metrics.messages_published.store(0, Ordering::Relaxed);
        self.metrics.messages_consumed.store(0, Ordering::Relaxed);
        self.metrics.messages_acked.store(0, Ordering::Relaxed);
        self.metrics.messages_nacked.store(0, Ordering::Relaxed);
        self.metrics.connection_errors.store(0, Ordering::Relaxed);
        // Note: We don't reset active_connections as it represents current state
    }

    /// 检查Redis连接和服务健康状态
    ///
    /// 执行Redis PING命令来验证连接是否正常工作。
    /// 这个方法可以用于健康检查端点或监控系统。
    ///
    /// # 返回值
    ///
    /// 返回`RedisHealthStatus`，包含健康状态、响应时间和可能的错误信息。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = RedisStreamConfig::default();
    /// # let queue = RedisStreamMessageQueue::new(config).await?;
    /// let health = queue.health_check().await?;
    /// if health.healthy {
    ///     println!("Redis is healthy, response time: {}ms", health.response_time_ms);
    /// } else {
    ///     println!("Redis is unhealthy: {:?}", health.error_message);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health_check(&self) -> Result<RedisHealthStatus> {
        let start = Instant::now();

        match self.check_connection_health().await {
            Ok(()) => {
                let duration = start.elapsed();
                Ok(RedisHealthStatus {
                    healthy: true,
                    response_time_ms: duration.as_millis() as u64,
                    error_message: None,
                })
            }
            Err(e) => {
                let duration = start.elapsed();
                Ok(RedisHealthStatus {
                    healthy: false,
                    response_time_ms: duration.as_millis() as u64,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }
}

/// Redis Stream性能监控指标快照
///
/// 包含某个时间点的所有性能指标值。与`RedisStreamMetrics`不同，
/// 这是一个普通的结构体，不包含原子类型，适合序列化和传输。
///
/// # 用途
///
/// - 日志记录和审计
/// - 监控系统集成
/// - 性能报告生成
/// - API响应数据
#[derive(Debug, Clone)]
pub struct RedisStreamMetricsSnapshot {
    pub messages_published: u64,
    pub messages_consumed: u64,
    pub messages_acked: u64,
    pub messages_nacked: u64,
    pub connection_errors: u64,
    pub active_connections: u32,
}

/// Redis连接健康状态
///
/// 表示Redis服务器连接的健康状态，包括响应时间和错误信息。
///
/// # 字段说明
///
/// - `healthy`: 连接是否健康
/// - `response_time_ms`: PING命令的响应时间（毫秒）
/// - `error_message`: 如果不健康，包含错误信息
#[derive(Debug, Clone)]
pub struct RedisHealthStatus {
    pub healthy: bool,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_core::models::{Message, TaskExecutionMessage};
    use serde_json::json;

    #[test]
    fn test_redis_stream_config_default() {
        let config = RedisStreamConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 6379);
        assert_eq!(config.database, 0);
        assert!(config.password.is_none());
        assert_eq!(config.connection_timeout_seconds, 30);
        assert_eq!(config.max_retry_attempts, 3);
        assert_eq!(config.retry_delay_seconds, 1);
        assert_eq!(config.consumer_group_prefix, "scheduler");
        assert_eq!(config.consumer_id, "default_consumer");
    }

    #[tokio::test]
    async fn test_redis_stream_message_queue_creation() {
        let config = RedisStreamConfig::default();
        let result = RedisStreamMessageQueue::new(config).await;

        // 这个测试可能会失败如果没有Redis服务器运行，但至少验证了构造函数的逻辑
        match result {
            Ok(queue) => {
                assert_eq!(queue.consumer_group_prefix, "scheduler");
                assert_eq!(queue.consumer_id, "default_consumer");
            }
            Err(_) => {
                // 如果Redis不可用，这是预期的
                // 在集成测试中我们会使用testcontainers来测试实际连接
            }
        }
    }

    #[tokio::test]
    async fn test_consumer_group_name_generation() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        let group_name = queue.get_consumer_group_name("test_queue");
        assert_eq!(group_name, "scheduler_test_queue_group");
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        let serialized = queue.serialize_message(&message).unwrap();
        let deserialized = queue.deserialize_message(&serialized).unwrap();

        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.retry_count, deserialized.retry_count);
        assert_eq!(message.message_type_str(), deserialized.message_type_str());
    }

    #[tokio::test]
    async fn test_queue_name_validation() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        // Valid queue names
        assert!(queue.validate_queue_name("valid_queue").is_ok());
        assert!(queue.validate_queue_name("queue-123").is_ok());
        assert!(queue.validate_queue_name("queue.name").is_ok());

        // Invalid queue names
        assert!(queue.validate_queue_name("").is_err());
        assert!(queue.validate_queue_name("queue with spaces").is_err());
        assert!(queue.validate_queue_name("queue\nwith\nnewlines").is_err());
        assert!(queue.validate_queue_name(&"x".repeat(256)).is_err());
    }

    #[tokio::test]
    async fn test_redis_stream_config_with_password() {
        let config = RedisStreamConfig {
            host: "localhost".to_string(),
            port: 6379,
            database: 1,
            password: Some("secret".to_string()),
            connection_timeout_seconds: 10,
            max_retry_attempts: 5,
            retry_delay_seconds: 2,
            consumer_group_prefix: "test".to_string(),
            consumer_id: "test_consumer".to_string(),
            pool_min_idle: 1,
            pool_max_open: 5,
            pool_timeout_seconds: 10,
        };

        let result = RedisStreamMessageQueue::new(config).await;

        // This should create the client successfully even if Redis is not running
        match result {
            Ok(queue) => {
                assert_eq!(queue.consumer_group_prefix, "test");
                assert_eq!(queue.consumer_id, "test_consumer");
            }
            Err(_) => {
                // Connection creation might fail if Redis is not available, which is fine for unit tests
            }
        }
    }

    #[tokio::test]
    async fn test_message_with_correlation_id_serialization() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution)
            .with_correlation_id("test-correlation-123".to_string());

        let serialized = queue.serialize_message(&message).unwrap();
        let deserialized = queue.deserialize_message(&serialized).unwrap();

        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.correlation_id, deserialized.correlation_id);
        assert_eq!(
            message.correlation_id,
            Some("test-correlation-123".to_string())
        );
    }

    #[tokio::test]
    async fn test_consumer_group_management() {
        let config = RedisStreamConfig {
            consumer_group_prefix: "test_prefix".to_string(),
            consumer_id: "test_consumer".to_string(),
            ..RedisStreamConfig::default()
        };

        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        // Test consumer group name generation
        let group_name = queue.get_consumer_group_name("test_queue");
        assert_eq!(group_name, "test_prefix_test_queue_group");

        // Test with different queue names
        assert_eq!(
            queue.get_consumer_group_name("tasks"),
            "test_prefix_tasks_group"
        );
        assert_eq!(
            queue.get_consumer_group_name("status_updates"),
            "test_prefix_status_updates_group"
        );
    }

    #[tokio::test]
    async fn test_message_id_mapping() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        // Test that message ID mapping is initialized
        assert!(queue.message_id_mapping.lock().is_ok());

        // Test adding mappings
        {
            let mut mapping = queue.message_id_mapping.lock().unwrap();
            mapping.insert(
                "msg-123".to_string(),
                ("1234567890-0".to_string(), "test_queue".to_string()),
            );
            mapping.insert(
                "msg-456".to_string(),
                ("1234567891-0".to_string(), "another_queue".to_string()),
            );
        }

        // Test retrieving mappings
        {
            let mapping = queue.message_id_mapping.lock().unwrap();
            assert_eq!(
                mapping.get("msg-123"),
                Some(&("1234567890-0".to_string(), "test_queue".to_string()))
            );
            assert_eq!(
                mapping.get("msg-456"),
                Some(&("1234567891-0".to_string(), "another_queue".to_string()))
            );
            assert_eq!(mapping.get("msg-999"), None);
        }
    }

    #[tokio::test]
    async fn test_ack_message_mapping_not_found() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        // Test that ack_message returns error when message ID is not found in mapping
        let result = queue.ack_message("non-existent-message").await;

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Message ID non-existent-message not found in mapping"));
        }
    }

    #[tokio::test]
    async fn test_nack_message_mapping_not_found() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        // Test that nack_message returns error when message ID is not found in mapping
        let result = queue.nack_message("non-existent-message", true).await;

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Message ID non-existent-message not found in mapping"));
        }
    }

    #[tokio::test]
    async fn test_message_mapping_operations() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        // Test adding and retrieving message mappings
        {
            let mut mapping = queue.message_id_mapping.lock().unwrap();
            mapping.insert(
                "msg-123".to_string(),
                ("1234567890-0".to_string(), "test_queue".to_string()),
            );
        }

        // Test that mapping exists
        {
            let mapping = queue.message_id_mapping.lock().unwrap();
            assert!(mapping.contains_key("msg-123"));
            let (stream_id, queue_name) = mapping.get("msg-123").unwrap();
            assert_eq!(stream_id, "1234567890-0");
            assert_eq!(queue_name, "test_queue");
        }
    }

    #[tokio::test]
    async fn test_queue_management_methods_exist() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config)
            .await
            .expect("Should create queue instance");

        // Test that all queue management methods exist and have correct signatures
        // We're testing method existence and basic validation, not Redis connectivity

        // Test with invalid queue names to trigger validation errors
        let empty_queue_result = queue.create_queue("", true).await;
        assert!(
            empty_queue_result.is_err(),
            "Should fail with empty queue name"
        );

        let long_queue_name = "x".repeat(300);
        let long_name_result = queue.delete_queue(&long_queue_name).await;
        assert!(
            long_name_result.is_err(),
            "Should fail with overly long queue name"
        );

        let invalid_chars_result = queue.get_queue_size("queue with spaces").await;
        assert!(
            invalid_chars_result.is_err(),
            "Should fail with invalid characters in queue name"
        );

        let newline_queue_result = queue.purge_queue("queue\nwith\nnewlines").await;
        assert!(
            newline_queue_result.is_err(),
            "Should fail with newlines in queue name"
        );

        println!("All queue management methods are properly implemented with validation");
    }

    #[tokio::test]
    async fn test_queue_management_validation_comprehensive() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config)
            .await
            .expect("Should create queue instance");

        // Test all four queue management methods with various invalid inputs
        let long_queue_name = "x".repeat(300);
        let test_cases = vec![
            ("", "empty queue name"),
            ("queue with spaces", "spaces in queue name"),
            ("queue\nwith\nnewlines", "newlines in queue name"),
            ("queue\rwith\rcarriage", "carriage returns in queue name"),
            (long_queue_name.as_str(), "overly long queue name"),
        ];

        for (invalid_queue, description) in test_cases {
            // Test create_queue validation
            let create_result = queue.create_queue(invalid_queue, true).await;
            assert!(
                create_result.is_err(),
                "create_queue should fail with {}",
                description
            );

            // Test delete_queue validation
            let delete_result = queue.delete_queue(invalid_queue).await;
            assert!(
                delete_result.is_err(),
                "delete_queue should fail with {}",
                description
            );

            // Test get_queue_size validation
            let size_result = queue.get_queue_size(invalid_queue).await;
            assert!(
                size_result.is_err(),
                "get_queue_size should fail with {}",
                description
            );

            // Test purge_queue validation
            let purge_result = queue.purge_queue(invalid_queue).await;
            assert!(
                purge_result.is_err(),
                "purge_queue should fail with {}",
                description
            );
        }

        // Test with valid queue names (should not fail due to validation, but will fail due to no Redis)
        let valid_queues = vec!["valid_queue", "task-queue", "queue.name", "queue123"];

        for valid_queue in valid_queues {
            // These should pass validation but fail on Redis connection
            let create_result = queue.create_queue(valid_queue, true).await;
            // We expect Redis connection error, not validation error
            if let Err(e) = create_result {
                let error_msg = e.to_string();
                assert!(
                    !error_msg.contains("Queue name"),
                    "Should not be a queue name validation error for '{}': {}",
                    valid_queue,
                    error_msg
                );
            }
        }

        println!("All queue management methods have proper validation");
    }

    #[tokio::test]
    async fn test_consume_messages_validation() {
        let config = RedisStreamConfig::default();
        let queue = RedisStreamMessageQueue::new(config).await.unwrap();

        // Test queue name validation in consume_messages context
        // These should be valid queue names for consumption
        assert!(queue.validate_queue_name("valid_queue").is_ok());
        assert!(queue.validate_queue_name("task_queue").is_ok());
        assert!(queue.validate_queue_name("status-updates").is_ok());

        // These should be invalid
        assert!(queue.validate_queue_name("").is_err());
        assert!(queue.validate_queue_name("queue with spaces").is_err());
        assert!(queue.validate_queue_name("queue\nwith\nnewlines").is_err());
    }
}
