use async_trait::async_trait;
use crate::entities::Message;
use scheduler_errors::SchedulerResult;

/// Interface for message queue operations
#[async_trait]
pub trait MessageQueue: Send + Sync {
    async fn publish_message(&self, queue: &str, message: &Message) -> SchedulerResult<()>;
    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>>;
    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()>;
    async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()>;
    async fn create_queue(&self, queue: &str, durable: bool) -> SchedulerResult<()>;
    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()>;
    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32>;
    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()>;
}