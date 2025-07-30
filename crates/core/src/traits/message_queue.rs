use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;

use crate::{errors::Result, models::Message};

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

/// Mock implementation of MessageQueue for testing
#[derive(Debug, Clone)]
pub struct MockMessageQueue {
    queues: Arc<Mutex<HashMap<String, Vec<Message>>>>,
    acked_messages: Arc<Mutex<Vec<String>>>,
    nacked_messages: Arc<Mutex<Vec<String>>>,
}

impl Default for MockMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl MockMessageQueue {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            acked_messages: Arc::new(Mutex::new(Vec::new())),
            nacked_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_acked_messages(&self) -> Vec<String> {
        self.acked_messages.lock().unwrap().clone()
    }

    pub fn get_nacked_messages(&self) -> Vec<String> {
        self.nacked_messages.lock().unwrap().clone()
    }

    pub fn get_queue_messages(&self, queue: &str) -> Vec<Message> {
        self.queues
            .lock()
            .unwrap()
            .get(queue)
            .cloned()
            .unwrap_or_default()
    }

    // Add methods needed by worker tests
    pub async fn add_message(&self, message: Message) -> Result<()> {
        let mut queues = self.queues.lock().unwrap();
        queues
            .entry("default".to_string())
            .or_default()
            .push(message);
        Ok(())
    }

    pub async fn get_messages(&self) -> Vec<Message> {
        let queues = self.queues.lock().unwrap();
        queues.values().flatten().cloned().collect()
    }
}

#[async_trait]
impl MessageQueue for MockMessageQueue {
    async fn publish_message(&self, queue: &str, message: &Message) -> Result<()> {
        let mut queues = self.queues.lock().unwrap();
        queues
            .entry(queue.to_string())
            .or_default()
            .push(message.clone());
        Ok(())
    }

    async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>> {
        let mut queues = self.queues.lock().unwrap();
        let messages = queues.remove(queue).unwrap_or_default();
        Ok(messages)
    }

    async fn ack_message(&self, message_id: &str) -> Result<()> {
        self.acked_messages
            .lock()
            .unwrap()
            .push(message_id.to_string());
        Ok(())
    }

    async fn nack_message(&self, message_id: &str, _requeue: bool) -> Result<()> {
        self.nacked_messages
            .lock()
            .unwrap()
            .push(message_id.to_string());
        Ok(())
    }

    async fn create_queue(&self, queue: &str, _durable: bool) -> Result<()> {
        let mut queues = self.queues.lock().unwrap();
        queues.entry(queue.to_string()).or_default();
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> Result<()> {
        let mut queues = self.queues.lock().unwrap();
        queues.remove(queue);
        Ok(())
    }

    async fn get_queue_size(&self, queue: &str) -> Result<u32> {
        let queues = self.queues.lock().unwrap();
        let size = queues.get(queue).map(|q| q.len()).unwrap_or(0) as u32;
        Ok(size)
    }

    async fn purge_queue(&self, queue: &str) -> Result<()> {
        let mut queues = self.queues.lock().unwrap();
        if let Some(queue_messages) = queues.get_mut(queue) {
            queue_messages.clear();
        }
        Ok(())
    }
}
