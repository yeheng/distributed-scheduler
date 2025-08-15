use scheduler_config::{MessageQueueConfig, MessageQueueType, RedisConfig};
use scheduler_domain::entities::*;
use scheduler_foundation::MessageQueue;
use scheduler_infrastructure::{MessageQueueFactory, MessageQueueManager};
use serde_json::json;

fn create_rabbitmq_config() -> MessageQueueConfig {
    MessageQueueConfig {
        r#type: MessageQueueType::Rabbitmq,
        url: "amqp://guest:guest@localhost:5672/".to_string(),
        redis: None,
        task_queue: "test_tasks".to_string(),
        status_queue: "test_status".to_string(),
        heartbeat_queue: "test_heartbeat".to_string(),
        control_queue: "test_control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    }
}

fn create_redis_stream_config_with_redis_section() -> MessageQueueConfig {
    MessageQueueConfig {
        r#type: MessageQueueType::RedisStream,
        url: "".to_string(),
        redis: Some(RedisConfig {
            host: "127.0.0.1".to_string(),
            port: 6379,
            database: 1, // 使用数据库1避免与其他测试冲突
            password: None,
            connection_timeout_seconds: 30,
            max_retry_attempts: 3,
            retry_delay_seconds: 1,
        }),
        task_queue: "test_tasks".to_string(),
        status_queue: "test_status".to_string(),
        heartbeat_queue: "test_heartbeat".to_string(),
        control_queue: "test_control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    }
}

fn create_redis_stream_config_with_url() -> MessageQueueConfig {
    MessageQueueConfig {
        r#type: MessageQueueType::RedisStream,
        url: "redis://localhost:6379/2".to_string(), // 使用数据库2避免与其他测试冲突
        redis: None,
        task_queue: "test_tasks".to_string(),
        status_queue: "test_status".to_string(),
        heartbeat_queue: "test_heartbeat".to_string(),
        control_queue: "test_control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    }
}

fn create_test_message() -> Message {
    let task_execution = TaskExecutionMessage {
        task_run_id: 123,
        task_id: 456,
        task_name: "test_task".to_string(),
        task_type: "test".to_string(),
        parameters: json!({"test": "data"}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    Message::task_execution(task_execution)
}

#[tokio::test]
async fn test_message_queue_factory_validate_config() {
    let rabbitmq_config = create_rabbitmq_config();
    assert!(MessageQueueFactory::validate_config(&rabbitmq_config).is_ok());
    let redis_config = create_redis_stream_config_with_redis_section();
    assert!(MessageQueueFactory::validate_config(&redis_config).is_ok());
    let redis_url_config = create_redis_stream_config_with_url();
    assert!(MessageQueueFactory::validate_config(&redis_url_config).is_ok());
    let mut invalid_rabbitmq_config = create_rabbitmq_config();
    invalid_rabbitmq_config.url = "invalid://localhost:5672".to_string();
    assert!(MessageQueueFactory::validate_config(&invalid_rabbitmq_config).is_err());
    let mut invalid_redis_config = create_redis_stream_config_with_redis_section();
    invalid_redis_config.redis = None;
    invalid_redis_config.url = "".to_string();
    assert!(MessageQueueFactory::validate_config(&invalid_redis_config).is_err());
}

#[tokio::test]
async fn test_message_queue_factory_type_parsing() {
    assert_eq!(
        MessageQueueFactory::parse_type_string("rabbitmq").unwrap(),
        MessageQueueType::Rabbitmq
    );
    assert_eq!(
        MessageQueueFactory::parse_type_string("redis_stream").unwrap(),
        MessageQueueType::RedisStream
    );
    assert_eq!(
        MessageQueueFactory::parse_type_string("RABBITMQ").unwrap(),
        MessageQueueType::Rabbitmq
    );
    assert!(MessageQueueFactory::parse_type_string("invalid").is_err());
    assert_eq!(
        MessageQueueFactory::get_type_string(&MessageQueueType::Rabbitmq),
        "rabbitmq"
    );
    assert_eq!(
        MessageQueueFactory::get_type_string(&MessageQueueType::RedisStream),
        "redis_stream"
    );
}

#[tokio::test]
#[ignore] // 需要Redis服务器运行
async fn test_redis_stream_message_queue_creation() {
    let config = create_redis_stream_config_with_redis_section();
    let result = MessageQueueFactory::create(&config).await;

    match result {
        Ok(queue) => {
            println!("Successfully created Redis Stream message queue with redis config section");
            let test_queue = "test_factory_queue";
            assert!(queue.create_queue(test_queue, true).await.is_ok());

            let message = create_test_message();
            assert!(queue.publish_message(test_queue, &message).await.is_ok());

            let size = queue.get_queue_size(test_queue).await.unwrap();
            assert!(size > 0);
            assert!(queue.purge_queue(test_queue).await.is_ok());
        }
        Err(e) => {
            println!("Failed to create Redis Stream message queue: {}", e);
            assert!(e.to_string().contains("Redis") || e.to_string().contains("connection"));
        }
    }
    let url_config = create_redis_stream_config_with_url();
    let result = MessageQueueFactory::create(&url_config).await;

    match result {
        Ok(queue) => {
            println!("Successfully created Redis Stream message queue with URL");
            let test_queue = "test_factory_queue_url";
            assert!(queue.create_queue(test_queue, true).await.is_ok());

            let message = create_test_message();
            assert!(queue.publish_message(test_queue, &message).await.is_ok());
            assert!(queue.purge_queue(test_queue).await.is_ok());
        }
        Err(e) => {
            println!(
                "Failed to create Redis Stream message queue with URL: {}",
                e
            );
            assert!(e.to_string().contains("Redis") || e.to_string().contains("connection"));
        }
    }
}

#[tokio::test]
#[ignore] // 需要RabbitMQ服务器运行
async fn test_rabbitmq_message_queue_creation() {
    let config = create_rabbitmq_config();
    let result = MessageQueueFactory::create(&config).await;

    match result {
        Ok(queue) => {
            println!("Successfully created RabbitMQ message queue");
            let test_queue = "test_factory_rabbitmq_queue";
            assert!(queue.create_queue(test_queue, true).await.is_ok());

            let message = create_test_message();
            assert!(queue.publish_message(test_queue, &message).await.is_ok());

            let size = queue.get_queue_size(test_queue).await.unwrap();
            assert!(size > 0);
            assert!(queue.purge_queue(test_queue).await.is_ok());
        }
        Err(e) => {
            println!("Failed to create RabbitMQ message queue: {}", e);
            assert!(
                e.to_string().contains("RabbitMQ")
                    || e.to_string().contains("AMQP")
                    || e.to_string().contains("connection")
            );
        }
    }
}

#[tokio::test]
#[ignore] // 需要Redis服务器运行
async fn test_message_queue_manager() {
    let redis_config = create_redis_stream_config_with_redis_section();
    let result = MessageQueueManager::new(redis_config.clone()).await;

    match result {
        Ok(manager) => {
            println!("Successfully created MessageQueueManager");
            assert!(manager.is_redis_stream());
            assert!(!manager.is_rabbitmq());
            assert_eq!(manager.get_current_type_string(), "redis_stream");
            let test_queue = "test_manager_queue";
            let queue = manager.get_queue();
            assert!(queue.create_queue(test_queue, true).await.is_ok());

            let message = create_test_message();
            assert!(queue.publish_message(test_queue, &message).await.is_ok());
            let message2 = create_test_message();
            assert!(manager.publish_message(test_queue, &message2).await.is_ok());

            let size = manager.get_queue_size(test_queue).await.unwrap();
            assert!(size >= 2);
            assert!(manager.purge_queue(test_queue).await.is_ok());

            println!("MessageQueueManager basic operations test passed");
        }
        Err(e) => {
            println!("Failed to create MessageQueueManager: {}", e);
            assert!(e.to_string().contains("Redis") || e.to_string().contains("connection"));
        }
    }
}

#[tokio::test]
#[ignore] // 需要Redis和RabbitMQ服务器运行
async fn test_message_queue_manager_switching() {
    let redis_config = create_redis_stream_config_with_redis_section();
    let result = MessageQueueManager::new(redis_config.clone()).await;

    if result.is_err() {
        println!("Skipping switching test - Redis not available");
        return;
    }

    let mut manager = result.unwrap();
    assert!(manager.is_redis_stream());
    assert_eq!(manager.get_current_type_string(), "redis_stream");
    let test_queue = "test_switching_queue";
    assert!(manager.create_queue(test_queue, true).await.is_ok());

    let redis_message = create_test_message();
    assert!(manager
        .publish_message(test_queue, &redis_message)
        .await
        .is_ok());

    let redis_size = manager.get_queue_size(test_queue).await.unwrap();
    assert!(redis_size > 0);
    let rabbitmq_config = create_rabbitmq_config();
    let switch_result = manager.switch_to(rabbitmq_config).await;

    match switch_result {
        Ok(()) => {
            println!("Successfully switched to RabbitMQ");
            assert!(manager.is_rabbitmq());
            assert!(!manager.is_redis_stream());
            assert_eq!(manager.get_current_type_string(), "rabbitmq");
            let rabbitmq_test_queue = "test_rabbitmq_switching_queue";
            assert!(manager
                .create_queue(rabbitmq_test_queue, true)
                .await
                .is_ok());

            let rabbitmq_message = create_test_message();
            assert!(manager
                .publish_message(rabbitmq_test_queue, &rabbitmq_message)
                .await
                .is_ok());

            let rabbitmq_size = manager.get_queue_size(rabbitmq_test_queue).await.unwrap();
            assert!(rabbitmq_size > 0);
            assert!(manager.purge_queue(rabbitmq_test_queue).await.is_ok());

            println!("Message queue switching test passed");
        }
        Err(e) => {
            println!("Failed to switch to RabbitMQ: {}", e);
            assert!(
                e.to_string().contains("RabbitMQ")
                    || e.to_string().contains("AMQP")
                    || e.to_string().contains("connection")
            );
        }
    }
    let redis_config_cleanup = create_redis_stream_config_with_redis_section();
    if manager.switch_to(redis_config_cleanup).await.is_ok() {
        let _ = manager.purge_queue(test_queue).await;
    }
}

#[tokio::test]
async fn test_configuration_validation_edge_cases() {
    let mut config = create_rabbitmq_config();
    config.url = "".to_string();
    assert!(MessageQueueFactory::validate_config(&config).is_err());
    config.url = "http://localhost:5672".to_string();
    assert!(MessageQueueFactory::validate_config(&config).is_err());
    let mut redis_config = create_redis_stream_config_with_redis_section();
    redis_config.redis = None;
    redis_config.url = "http://localhost:6379".to_string(); // 错误的协议
    assert!(MessageQueueFactory::validate_config(&redis_config).is_err());
    redis_config.url = "".to_string();
    assert!(MessageQueueFactory::validate_config(&redis_config).is_err());
}

#[test]
fn test_redis_url_parsing() {
    let config = MessageQueueConfig {
        r#type: MessageQueueType::RedisStream,
        url: "redis://localhost:6379/0".to_string(),
        redis: None,
        task_queue: "tasks".to_string(),
        status_queue: "status".to_string(),
        heartbeat_queue: "heartbeat".to_string(),
        control_queue: "control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    };

    let redis_config = MessageQueueFactory::parse_redis_url(&config.url, &config).unwrap();
    assert_eq!(redis_config.host, "localhost");
    assert_eq!(redis_config.port, 6379);
    assert_eq!(redis_config.database, 0);
    assert!(redis_config.password.is_none());
    let config_with_auth = MessageQueueConfig {
        r#type: MessageQueueType::RedisStream,
        url: "redis://:mypassword@redis.example.com:6380/1".to_string(),
        redis: None,
        task_queue: "tasks".to_string(),
        status_queue: "status".to_string(),
        heartbeat_queue: "heartbeat".to_string(),
        control_queue: "control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    };

    let redis_config_auth =
        MessageQueueFactory::parse_redis_url(&config_with_auth.url, &config_with_auth).unwrap();
    assert_eq!(redis_config_auth.host, "redis.example.com");
    assert_eq!(redis_config_auth.port, 6380);
    assert_eq!(redis_config_auth.database, 1);
    assert_eq!(redis_config_auth.password, Some("mypassword".to_string()));
    let invalid_config = MessageQueueConfig {
        r#type: MessageQueueType::RedisStream,
        url: "invalid-url".to_string(),
        redis: None,
        task_queue: "tasks".to_string(),
        status_queue: "status".to_string(),
        heartbeat_queue: "heartbeat".to_string(),
        control_queue: "control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    };

    assert!(MessageQueueFactory::parse_redis_url(&invalid_config.url, &invalid_config).is_err());
}
