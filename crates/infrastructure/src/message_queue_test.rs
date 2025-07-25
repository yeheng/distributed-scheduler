#[cfg(test)]
mod message_queue_test {
    use super::*;
    use scheduler_core::{config::MessageQueueConfig, models::Message};
    use tokio;

    fn get_test_config() -> MessageQueueConfig {
        MessageQueueConfig {
            url: "amqp://guest:guest@localhost:5672".to_string(),
            task_queue: "test_tasks".to_string(),
            status_queue: "test_status".to_string(),
            heartbeat_queue: "test_heartbeat".to_string(),
            control_queue: "test_control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 1,
            connection_timeout_seconds: 30,
        }
    }

    fn create_test_message() -> Message {
        Message {
            id: "test-message-1".to_string(),
            message_type: "task_dispatch".to_string(),
            payload: serde_json::json!({
                "task_id": 1,
                "worker_id": "worker-001"
            }),
            timestamp: chrono::Utc::now(),
            retry_count: 0,
        }
    }

    #[tokio::test]
    #[ignore] // 需要运行RabbitMQ服务
    async fn test_rabbitmq_connection() {
        let config = get_test_config();
        let result = RabbitMQMessageQueue::new(config).await;
        
        match result {
            Ok(queue) => {
                assert!(queue.is_connected());
                let _ = queue.close().await;
            }
            Err(e) => {
                println!("连接失败 (可能RabbitMQ未运行): {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // 需要运行RabbitMQ服务
    async fn test_publish_and_consume_message() {
        let config = get_test_config();
        let queue = RabbitMQMessageQueue::new(config).await.unwrap();
        
        let test_message = create_test_message();
        let queue_name = "test_queue";

        // 创建测试队列
        queue.create_queue(queue_name, false).await.unwrap();

        // 发布消息
        queue.publish_message(queue_name, &test_message).await.unwrap();

        // 消费消息
        let messages = queue.consume_messages(queue_name).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, test_message.id);
        assert_eq!(messages[0].message_type, test_message.message_type);

        // 清理
        queue.delete_queue(queue_name).await.unwrap();
        let _ = queue.close().await;
    }

    #[tokio::test]
    #[ignore] // 需要运行RabbitMQ服务
    async fn test_queue_operations() {
        let config = get_test_config();
        let queue = RabbitMQMessageQueue::new(config).await.unwrap();
        
        let queue_name = "test_operations_queue";

        // 创建队列
        queue.create_queue(queue_name, false).await.unwrap();

        // 检查队列大小
        let size = queue.get_queue_size(queue_name).await.unwrap();
        assert_eq!(size, 0);

        // 发布几条消息
        for i in 0..3 {
            let mut message = create_test_message();
            message.id = format!("test-message-{}", i);
            queue.publish_message(queue_name, &message).await.unwrap();
        }

        // 检查队列大小
        let size = queue.get_queue_size(queue_name).await.unwrap();
        assert_eq!(size, 3);

        // 清空队列
        queue.purge_queue(queue_name).await.unwrap();

        // 检查队列大小
        let size = queue.get_queue_size(queue_name).await.unwrap();
        assert_eq!(size, 0);

        // 删除队列
        queue.delete_queue(queue_name).await.unwrap();
        let _ = queue.close().await;
    }

    #[tokio::test]
    #[ignore] // 需要运行RabbitMQ服务
    async fn test_message_persistence() {
        let config = get_test_config();
        let queue = RabbitMQMessageQueue::new(config).await.unwrap();
        
        let queue_name = "test_persistence_queue";
        let test_message = create_test_message();

        // 创建持久化队列
        queue.create_queue(queue_name, true).await.unwrap();

        // 发布持久化消息
        queue.publish_message(queue_name, &test_message).await.unwrap();

        // 检查消息是否存在
        let size = queue.get_queue_size(queue_name).await.unwrap();
        assert_eq!(size, 1);

        // 清理
        queue.delete_queue(queue_name).await.unwrap();
        let _ = queue.close().await;
    }
}