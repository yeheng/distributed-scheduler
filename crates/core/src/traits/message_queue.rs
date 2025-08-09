//! 消息队列抽象接口定义
//!
//! 此模块定义了任务调度系统中消息队列的统一抽象接口，支持多种消息队列实现：
//! - Redis Stream
//! - RabbitMQ
//! - Apache Kafka
//! - 内存队列（测试用）
//!
//! ## 设计原则
//!
//! ### 接口统一
//! 提供统一的消息队列操作接口，屏蔽不同消息队列系统的实现差异：
//! - 标准的发布/订阅模式
//! - 消息确认和重试机制
//! - 队列管理和监控接口
//! - 异步非阻塞操作
//!
//! ### 可靠性保证
//! 确保消息的可靠传递和处理：
//! - 消息持久化支持
//! - 消息确认机制（ACK/NACK）
//! - 死信队列处理
//! - 消息重试和去重
//!
//! ### 高性能
//! 支持高并发和大吞吐量场景：
//! - 批量消息处理
//! - 异步非阻塞I/O
//! - 连接池和资源复用
//! - 背压控制和流量整形
//!
//! ## 消息生命周期
//!
//! 1. **发布** - 生产者发布消息到队列
//! 2. **投递** - 队列将消息投递给消费者
//! 3. **处理** - 消费者处理消息业务逻辑
//! 4. **确认** - 消费者确认消息处理完成
//! 5. **清理** - 队列清理已确认的消息
//!
//! ## 使用示例
//!
//! ### 基本消息发布和消费
//!
//! ```rust
//! use scheduler_core::traits::MessageQueue;
//! use scheduler_domain::entities::Message;
//!
//! async fn message_workflow(queue: &dyn MessageQueue) -> SchedulerResult<()> {
//!     // 1. 创建队列
//!     queue.create_queue("task_queue", true).await?;
//!     
//!     // 2. 发布消息
//!     let message = Message::new("task_execution", serde_json::json!({
//!         "task_id": 123,
//!         "worker_id": "worker-001",
//!         "priority": "high"
//!     }));
//!     queue.publish_message("task_queue", &message).await?;
//!     
//!     // 3. 消费消息
//!     let messages = queue.consume_messages("task_queue").await?;
//!     for msg in messages {
//!         println!("处理消息: {}", msg.id);
//!         
//!         // 4. 处理业务逻辑
//!         match process_task_message(&msg).await {
//!             Ok(_) => {
//!                 // 5. 确认消息
//!                 queue.ack_message(&msg.id).await?;
//!             }
//!             Err(e) => {
//!                 // 6. 拒绝消息并重新入队
//!                 eprintln!("处理失败: {}", e);
//!                 queue.nack_message(&msg.id, true).await?;
//!             }
//!         }
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### 队列监控和管理
//!
//! ```rust
//! async fn monitor_queues(queue: &dyn MessageQueue) -> SchedulerResult<()> {
//!     let queue_names = ["task_queue", "status_queue", "notification_queue"];
//!     
//!     for queue_name in &queue_names {
//!         let size = queue.get_queue_size(queue_name).await?;
//!         println!("队列 {} 当前消息数: {}", queue_name, size);
//!         
//!         // 清理积压消息
//!         if size > 10000 {
//!             println!("队列 {} 消息积压，执行清理", queue_name);
//!             queue.purge_queue(queue_name).await?;
//!         }
//!     }
//!     
//!     Ok(())
//! }
//! ```

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;

use crate::{models::Message, SchedulerResult};

/// 消息队列抽象接口
#[async_trait]
pub trait MessageQueue: Send + Sync {
    /// 发布消息到指定队列
    async fn publish_message(&self, queue: &str, message: &Message) -> SchedulerResult<()>;

    /// 从指定队列消费消息
    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>>;

    /// 确认消息处理完成
    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()>;

    /// 拒绝消息并重新入队
    async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()>;

    /// 创建队列
    async fn create_queue(&self, queue: &str, durable: bool) -> SchedulerResult<()>;

    /// 删除队列
    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()>;

    /// 获取队列中的消息数量
    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32>;

    /// 清空队列
    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()>;
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
    pub async fn add_message(&self, message: Message) -> SchedulerResult<()> {
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
    async fn publish_message(&self, queue: &str, message: &Message) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().unwrap();
        queues
            .entry(queue.to_string())
            .or_default()
            .push(message.clone());
        Ok(())
    }

    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>> {
        let mut queues = self.queues.lock().unwrap();
        let messages = queues.remove(queue).unwrap_or_default();
        Ok(messages)
    }

    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()> {
        self.acked_messages
            .lock()
            .unwrap()
            .push(message_id.to_string());
        Ok(())
    }

    async fn nack_message(&self, message_id: &str, _requeue: bool) -> SchedulerResult<()> {
        self.nacked_messages
            .lock()
            .unwrap()
            .push(message_id.to_string());
        Ok(())
    }

    async fn create_queue(&self, queue: &str, _durable: bool) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().unwrap();
        queues.entry(queue.to_string()).or_default();
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().unwrap();
        queues.remove(queue);
        Ok(())
    }

    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32> {
        let queues = self.queues.lock().unwrap();
        let size = queues.get(queue).map(|q| q.len()).unwrap_or(0) as u32;
        Ok(size)
    }

    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().unwrap();
        if let Some(queue_messages) = queues.get_mut(queue) {
            queue_messages.clear();
        }
        Ok(())
    }
}
