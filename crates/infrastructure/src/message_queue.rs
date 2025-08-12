use async_trait::async_trait;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
    Consumer, Queue,
};
use scheduler_core::traits::MessageQueue;
use scheduler_core::{config::models::MessageQueueConfig, SchedulerResult};
use scheduler_domain::entities::Message;
use scheduler_errors::SchedulerError;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, instrument};

use crate::{message_queue_context, error_handling::{
    RepositoryErrorHelpers, RepositoryOperation,
}, timeout_handler::TimeoutUtils};

pub struct RabbitMQMessageQueue {
    connection: Connection,
    channel: Arc<Mutex<Channel>>,
    config: MessageQueueConfig,
}

impl RabbitMQMessageQueue {
    #[instrument(skip(config))]
    pub async fn new(config: MessageQueueConfig) -> SchedulerResult<Self> {
        let context = message_queue_context!(RepositoryOperation::Create)
            .with_additional_info(format!("连接到RabbitMQ: {}", config.url));

        let connection = TimeoutUtils::message_queue(
            async {
                Connection::connect(&config.url, ConnectionProperties::default())
                    .await
                    .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))
            },
            &format!("连接到RabbitMQ: {}", config.url),
        )
        .await?;

        let channel = TimeoutUtils::message_queue(
            async {
                connection
                    .create_channel()
                    .await
                    .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))
            },
            "创建RabbitMQ通道",
        )
        .await?;

        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &format!("RabbitMQ连接: {}", config.url),
            Some("连接创建成功")
        );

        let queue = Self {
            connection,
            channel: Arc::new(Mutex::new(channel)),
            config,
        };
        queue.initialize_queues().await?;

        Ok(queue)
    }
    #[instrument(skip(self))]
    async fn initialize_queues(&self) -> SchedulerResult<()> {
        let context = message_queue_context!(RepositoryOperation::Execute)
            .with_additional_info("初始化所有RabbitMQ队列".to_string());

        let channel = self.channel.lock().await;
        self.declare_queue(&channel, &self.config.task_queue, true)
            .await?;
        self.declare_queue(&channel, &self.config.status_queue, true)
            .await?;
        self.declare_queue(&channel, &self.config.heartbeat_queue, true)
            .await?;
        self.declare_queue(&channel, &self.config.control_queue, true)
            .await?;

        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &"队列初始化",
            Some("所有队列初始化完成")
        );
        Ok(())
    }
    #[instrument(skip(self, channel), fields(queue_name = %queue_name, durable = %durable))]
    async fn declare_queue(
        &self,
        channel: &Channel,
        queue_name: &str,
        durable: bool,
    ) -> SchedulerResult<Queue> {
        let context = message_queue_context!(RepositoryOperation::Create, queue = queue_name)
            .with_additional_info(format!("声明队列，持久化: {}", durable));

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
            .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &format!("队列声明: {}", queue_name),
            Some(&format!("持久化: {}", durable))
        );
        Ok(queue)
    }
    #[instrument(skip(self, message), fields(message_id = %message.id, message_type = %message.message_type_str()))]
    fn serialize_message(&self, message: &Message) -> SchedulerResult<Vec<u8>> {
        let context = message_queue_context!(RepositoryOperation::Execute)
            .with_message_id(message.id.clone())
            .with_message_type(message.message_type_str().to_string())
            .with_additional_info(format!("序列化消息: {}", message.id));

        serde_json::to_vec(message)
            .map_err(|e| {
                let error_msg = format!("序列化消息失败: {e}");
                RepositoryErrorHelpers::log_operation_warning_message_queue(
                    context.clone(),
                    &format!("消息序列化: {}", message.id),
                    &error_msg
                );
                SchedulerError::Serialization(error_msg)
            })
    }
    
    #[instrument(skip(self, data), fields(data_size = %data.len()))]
    fn deserialize_message(&self, data: &[u8]) -> SchedulerResult<Message> {
        let context = message_queue_context!(RepositoryOperation::Execute)
            .with_additional_info(format!("反序列化消息，数据大小: {}", data.len()));

        serde_json::from_slice(data)
            .map_err(|e| {
                let error_msg = format!("反序列化消息失败: {e}");
                RepositoryErrorHelpers::log_operation_warning_message_queue(
                    context.clone(),
                    &"消息反序列化",
                    &error_msg
                );
                SchedulerError::Serialization(error_msg)
            })
    }
    #[instrument(skip(self), fields(queue = %queue, consumer_tag = %consumer_tag))]
    pub async fn create_consumer(
        &self,
        queue: &str,
        consumer_tag: &str,
    ) -> SchedulerResult<Consumer> {
        let context = message_queue_context!(RepositoryOperation::Create, queue = queue)
            .with_additional_info(format!("创建消费者: {}", consumer_tag));

        let channel = self.channel.lock().await;
        let consumer = channel
            .basic_consume(
                queue,
                consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &format!("消费者创建: {}", consumer_tag),
            Some(&format!("队列: {}", queue))
        );
        Ok(consumer)
    }
    pub fn is_connected(&self) -> bool {
        self.connection.status().connected()
    }
    #[instrument(skip(self))]
    pub async fn close(&self) -> SchedulerResult<()> {
        let context = message_queue_context!(RepositoryOperation::Delete)
            .with_additional_info("关闭RabbitMQ连接".to_string());

        self.connection
            .close(200, "正常关闭")
            .await
            .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &"RabbitMQ连接关闭",
            Some("正常关闭连接")
        );
        Ok(())
    }
}

#[async_trait]
impl MessageQueue for RabbitMQMessageQueue {
    #[instrument(skip(self, message), fields(
        queue = %queue,
        message_id = %message.id,
        message_type = %message.message_type_str(),
    ))]
    async fn publish_message(&self, queue: &str, message: &Message) -> SchedulerResult<()> {
        let context = message_queue_context!(RepositoryOperation::Create, queue = queue)
            .with_message_id(message.id.clone())
            .with_message_type(message.message_type_str().to_string())
            .with_additional_info(format!("发布消息到队列: {}", queue));

        let channel = self.channel.lock().await;
        let payload = self.serialize_message(message)?;

        TimeoutUtils::message_queue(
            async {
                let confirm = channel
                    .basic_publish(
                        "",
                        queue,
                        BasicPublishOptions::default(),
                        &payload,
                        BasicProperties::default().with_delivery_mode(2), // 2 = persistent
                    )
                    .await
                    .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))?;
                
                confirm
                    .await
                    .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))
            },
            &format!("发布消息到队列 '{}'", queue),
        )
        .await?;

        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &format!("消息发布: {}", message.id),
            Some(&format!("队列: {}, 类型: {}", queue, message.message_type_str()))
        );
        Ok(())
    }
    #[instrument(skip(self), fields(queue = %queue))]
    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>> {
        let context = message_queue_context!(RepositoryOperation::Read, queue = queue)
            .with_additional_info(format!("从队列消费消息: {}", queue));

        let channel = self.channel.lock().await;
        let get_result = channel.basic_get(queue, BasicGetOptions::default()).await;

        match get_result {
            Ok(Some(delivery)) => {
                let message = self.deserialize_message(&delivery.data)?;
                let channel = self.channel.lock().await;
                channel
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await
                    .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))?;

                debug!("从队列 {} 消费消息成功: {}", queue, message.id);
                Ok(vec![message])
            }
            Ok(None) => {
                debug!("队列 {} 为空，返回空结果", queue);
                Ok(vec![])
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("NOT_FOUND") || error_msg.contains("404") {
                    debug!("队列 {} 不存在，返回空结果", queue);
                    Ok(vec![])
                } else {
                    let error = SchedulerError::MessageQueue(format!("从队列 {queue} 获取消息失败: {e}"));
                    RepositoryErrorHelpers::log_operation_warning_message_queue(
                        context,
                        &format!("消息消费失败: 队列 {}", queue),
                        &error_msg
                    );
                    Err(error)
                }
            }
        }
    }
    #[instrument(skip(self), fields(message_id = %message_id))]
    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()> {
        let _context = message_queue_context!(RepositoryOperation::Update)
            .with_message_id(message_id.to_string())
            .with_additional_info(format!("确认消息: {}", message_id));

        debug!("确认消息: {}", message_id);
        Ok(())
    }
    
    #[instrument(skip(self), fields(message_id = %message_id, requeue = %requeue))]
    async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()> {
        let _context = message_queue_context!(RepositoryOperation::Update)
            .with_message_id(message_id.to_string())
            .with_additional_info(format!("拒绝消息: {}, 重新入队: {}", message_id, requeue));

        debug!("拒绝消息: {}, 重新入队: {}", message_id, requeue);
        Ok(())
    }
    #[instrument(skip(self), fields(queue = %queue, durable = %durable))]
    async fn create_queue(&self, queue: &str, durable: bool) -> SchedulerResult<()> {
        let context = message_queue_context!(RepositoryOperation::Create, queue = queue)
            .with_additional_info(format!("创建队列，持久化: {}", durable));

        let channel = self.channel.lock().await;
        self.declare_queue(&channel, queue, durable).await?;
        
        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &format!("队列创建: {}", queue),
            Some(&format!("持久化: {}", durable))
        );
        Ok(())
    }
    #[instrument(skip(self), fields(queue = %queue))]
    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()> {
        let context = message_queue_context!(RepositoryOperation::Delete, queue = queue)
            .with_additional_info(format!("删除队列: {}", queue));

        let channel = self.channel.lock().await;
        channel
            .queue_delete(queue, QueueDeleteOptions::default())
            .await
            .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &format!("队列删除: {}", queue),
            Some("队列删除成功")
        );
        Ok(())
    }
    #[instrument(skip(self), fields(queue = %queue))]
    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32> {
        let context = message_queue_context!(RepositoryOperation::Read, queue = queue)
            .with_additional_info(format!("获取队列大小: {}", queue));

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
            Ok(info) => {
                let size = info.message_count();
                debug!("获取队列 {} 大小成功: {}", queue, size);
                Ok(size)
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("NOT_FOUND") || error_msg.contains("404") {
                    debug!("队列 {} 不存在，返回大小为0", queue);
                    Ok(0)
                } else {
                    let error = SchedulerError::MessageQueue(format!("获取队列 {queue} 信息失败: {e}"));
                    RepositoryErrorHelpers::log_operation_warning_message_queue(
                        context,
                        &format!("获取队列大小失败: {}", queue),
                        &error_msg
                    );
                    Err(error)
                }
            }
        }
    }
    #[instrument(skip(self), fields(queue = %queue))]
    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()> {
        let context = message_queue_context!(RepositoryOperation::Delete, queue = queue)
            .with_additional_info(format!("清空队列: {}", queue));

        let channel = self.channel.lock().await;
        channel
            .queue_purge(queue, QueuePurgeOptions::default())
            .await
            .map_err(|e| RepositoryErrorHelpers::message_queue_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success_message_queue(
            context,
            &format!("队列清空: {}", queue),
            Some("队列清空成功")
        );
        Ok(())
    }
}
