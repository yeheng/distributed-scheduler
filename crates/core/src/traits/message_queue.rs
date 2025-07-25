use async_trait::async_trait;

use crate::{models::Message, Result};

/// 消息队列抽象接口
#[async_trait]
pub trait MessageQueue: Send + Sync {
    /// 发布消息到指定队列
    async fn publish_message(&self, queue: &str, message: &Message) -> Result<()>;

    /// 从指定队列消费消息
    async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>>;

    /// 确认消息处理完成
    async fn ack_message(&self, message_id: &str) -> Result<()>;

    /// 拒绝消息并重新入队
    async fn nack_message(&self, message_id: &str, requeue: bool) -> Result<()>;

    /// 创建队列
    async fn create_queue(&self, queue: &str, durable: bool) -> Result<()>;

    /// 删除队列
    async fn delete_queue(&self, queue: &str) -> Result<()>;

    /// 获取队列中的消息数量
    async fn get_queue_size(&self, queue: &str) -> Result<u32>;

    /// 清空队列
    async fn purge_queue(&self, queue: &str) -> Result<()>;
}
