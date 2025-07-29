use async_trait::async_trait;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
    Consumer, Queue,
};
use scheduler_core::{
    config::model::MessageQueueConfig, models::Message, traits::MessageQueue, Result,
    SchedulerError,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// RabbitMQ消息队列实现
pub struct RabbitMQMessageQueue {
    connection: Connection,
    channel: Arc<Mutex<Channel>>,
    config: MessageQueueConfig,
}

impl RabbitMQMessageQueue {
    /// 创建新的RabbitMQ消息队列实例
    pub async fn new(config: MessageQueueConfig) -> Result<Self> {
        let connection = Connection::connect(&config.url, ConnectionProperties::default())
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("连接RabbitMQ失败: {e}")))?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("创建通道失败: {e}")))?;

        info!("成功连接到RabbitMQ: {}", config.url);

        let queue = Self {
            connection,
            channel: Arc::new(Mutex::new(channel)),
            config,
        };

        // 初始化队列
        queue.initialize_queues().await?;

        Ok(queue)
    }

    /// 初始化所有必需的队列
    async fn initialize_queues(&self) -> Result<()> {
        let channel = self.channel.lock().await;

        // 创建任务队列
        self.declare_queue(&channel, &self.config.task_queue, true)
            .await?;

        // 创建状态更新队列
        self.declare_queue(&channel, &self.config.status_queue, true)
            .await?;

        // 创建心跳队列
        self.declare_queue(&channel, &self.config.heartbeat_queue, true)
            .await?;

        // 创建控制队列
        self.declare_queue(&channel, &self.config.control_queue, true)
            .await?;

        info!("所有队列初始化完成");
        Ok(())
    }

    /// 声明队列
    async fn declare_queue(
        &self,
        channel: &Channel,
        queue_name: &str,
        durable: bool,
    ) -> Result<Queue> {
        let queue = channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable,
                    exclusive: false,
                    auto_delete: false,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                SchedulerError::MessageQueue(format!("声明队列 {queue_name} 失败: {e}"))
            })?;

        debug!("队列 {} 声明成功", queue_name);
        Ok(queue)
    }

    /// 序列化消息
    fn serialize_message(&self, message: &Message) -> Result<Vec<u8>> {
        serde_json::to_vec(message)
            .map_err(|e| SchedulerError::Serialization(format!("序列化消息失败: {e}")))
    }

    /// 反序列化消息
    fn deserialize_message(&self, data: &[u8]) -> Result<Message> {
        serde_json::from_slice(data)
            .map_err(|e| SchedulerError::Serialization(format!("反序列化消息: {e}")))
    }

    /// 创建消费者
    pub async fn create_consumer(&self, queue: &str, consumer_tag: &str) -> Result<Consumer> {
        let channel = self.channel.lock().await;
        let consumer = channel
            .basic_consume(
                queue,
                consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("创建消费者失败: {e}")))?;

        debug!("为队列 {} 创建消费者: {}", queue, consumer_tag);
        Ok(consumer)
    }

    /// 获取连接状态
    pub fn is_connected(&self) -> bool {
        self.connection.status().connected()
    }

    /// 关闭连接
    pub async fn close(&self) -> Result<()> {
        self.connection
            .close(200, "正常关闭")
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("关闭连接失败: {e}")))?;

        info!("RabbitMQ连接已关闭");
        Ok(())
    }
}

#[async_trait]
impl MessageQueue for RabbitMQMessageQueue {
    /// 发布消息到指定队列
    async fn publish_message(&self, queue: &str, message: &Message) -> Result<()> {
        let channel = self.channel.lock().await;
        let payload = self.serialize_message(message)?;

        let confirm = channel
            .basic_publish(
                "",
                queue,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default().with_delivery_mode(2), // 2 = persistent
            )
            .await
            .map_err(|e| {
                SchedulerError::MessageQueue(format!("发布消息到队列 {queue} 失败: {e}"))
            })?;

        // 等待确认
        confirm
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("消息发布确认失败: {e}")))?;

        debug!("消息已发布到队列: {}", queue);
        Ok(())
    }

    /// 从指定队列消费消息
    async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>> {
        let channel = self.channel.lock().await;

        // 获取单个消息
        let get_result = channel.basic_get(queue, BasicGetOptions::default()).await;

        match get_result {
            Ok(Some(delivery)) => {
                let message = self.deserialize_message(&delivery.data)?;

                // 自动确认消息
                let channel = self.channel.lock().await;
                channel
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await
                    .map_err(|e| SchedulerError::MessageQueue(format!("确认消息失败: {e}")))?;

                Ok(vec![message])
            }
            Ok(None) => {
                // 队列为空或没有消息
                Ok(vec![])
            }
            Err(e) => {
                // 检查是否是队列不存在的错误
                let error_msg = e.to_string();
                if error_msg.contains("NOT_FOUND") || error_msg.contains("404") {
                    // 队列不存在，返回空结果而不是错误
                    debug!("队列 {} 不存在，返回空结果", queue);
                    Ok(vec![])
                } else {
                    // 其他错误应该抛出
                    Err(SchedulerError::MessageQueue(format!(
                        "从队列 {queue} 获取消息失败: {e}"
                    )))
                }
            }
        }
    }

    /// 确认消息处理完成
    async fn ack_message(&self, message_id: &str) -> Result<()> {
        // 在实际实现中，需要跟踪delivery_tag
        // 这里简化处理
        debug!("确认消息: {}", message_id);
        Ok(())
    }

    /// 拒绝消息并重新入队
    async fn nack_message(&self, message_id: &str, requeue: bool) -> Result<()> {
        // 在实际实现中，需要跟踪delivery_tag
        // 这里简化处理
        debug!("拒绝消息: {}, 重新入队: {}", message_id, requeue);
        Ok(())
    }

    /// 创建队列
    async fn create_queue(&self, queue: &str, durable: bool) -> Result<()> {
        let channel = self.channel.lock().await;
        self.declare_queue(&channel, queue, durable).await?;
        Ok(())
    }

    /// 删除队列
    async fn delete_queue(&self, queue: &str) -> Result<()> {
        let channel = self.channel.lock().await;
        channel
            .queue_delete(queue, QueueDeleteOptions::default())
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("删除队列 {queue} 失败: {e}")))?;

        debug!("队列 {} 已删除", queue);
        Ok(())
    }

    /// 获取队列中的消息数量
    async fn get_queue_size(&self, queue: &str) -> Result<u32> {
        let channel = self.channel.lock().await;
        let queue_info = channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await;

        match queue_info {
            Ok(info) => Ok(info.message_count()),
            Err(e) => {
                // 检查是否是队列不存在的错误
                let error_msg = e.to_string();
                if error_msg.contains("NOT_FOUND") || error_msg.contains("404") {
                    // 队列不存在，返回0而不是错误
                    debug!("队列 {} 不存在，返回大小为0", queue);
                    Ok(0)
                } else {
                    // 其他错误应该抛出
                    Err(SchedulerError::MessageQueue(format!(
                        "获取队列 {queue} 信息失败: {e}"
                    )))
                }
            }
        }
    }

    /// 清空队列
    async fn purge_queue(&self, queue: &str) -> Result<()> {
        let channel = self.channel.lock().await;
        channel
            .queue_purge(queue, QueuePurgeOptions::default())
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("清空队列 {queue} 失败: {e}")))?;

        debug!("队列 {} 已清空", queue);
        Ok(())
    }
}
