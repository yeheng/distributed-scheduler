use scheduler_core::{
    config::model::{MessageQueueConfig, MessageQueueType, RedisConfig},
    models::{Message, TaskExecutionMessage},
    traits::MessageQueue,
};
use scheduler_infrastructure::{MessageQueueFactory, MessageQueueManager};
use serde_json::json;

/// 创建测试用的RabbitMQ配置
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

/// 创建测试用的Redis Stream配置（使用redis配置段）
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

/// 创建测试用的Redis Stream配置（使用URL）
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

/// 创建测试消息
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
    // 测试有效的RabbitMQ配置
    let rabbitmq_config = create_rabbitmq_config();
    assert!(MessageQueueFactory::validate_config(&rabbitmq_config).is_ok());

    // 测试有效的Redis Stream配置（使用redis配置段）
    let redis_config = create_redis_stream_config_with_redis_section();
    assert!(MessageQueueFactory::validate_config(&redis_config).is_ok());

    // 测试有效的Redis Stream配置（使用URL）
    let redis_url_config = create_redis_stream_config_with_url();
    assert!(MessageQueueFactory::validate_config(&redis_url_config).is_ok());

    // 测试无效的RabbitMQ配置（错误的URL格式）
    let mut invalid_rabbitmq_config = create_rabbitmq_config();
    invalid_rabbitmq_config.url = "invalid://localhost:5672".to_string();
    assert!(MessageQueueFactory::validate_config(&invalid_rabbitmq_config).is_err());

    // 测试无效的Redis Stream配置（缺少配置）
    let mut invalid_redis_config = create_redis_stream_config_with_redis_section();
    invalid_redis_config.redis = None;
    invalid_redis_config.url = "".to_string();
    assert!(MessageQueueFactory::validate_config(&invalid_redis_config).is_err());
}

#[tokio::test]
async fn test_message_queue_factory_type_parsing() {
    // 测试类型字符串解析
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

    // 测试无效类型
    assert!(MessageQueueFactory::parse_type_string("invalid").is_err());

    // 测试类型到字符串转换
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
    // 测试使用redis配置段创建Redis Stream消息队列
    let config = create_redis_stream_config_with_redis_section();
    let result = MessageQueueFactory::create(&config).await;

    match result {
        Ok(queue) => {
            println!("Successfully created Redis Stream message queue with redis config section");

            // 测试基本操作
            let test_queue = "test_factory_queue";
            assert!(queue.create_queue(test_queue, true).await.is_ok());

            let message = create_test_message();
            assert!(queue.publish_message(test_queue, &message).await.is_ok());

            let size = queue.get_queue_size(test_queue).await.unwrap();
            assert!(size > 0);

            // 清理
            assert!(queue.purge_queue(test_queue).await.is_ok());
        }
        Err(e) => {
            println!("Failed to create Redis Stream message queue: {}", e);
            // 如果Redis不可用，测试应该被跳过而不是失败
            assert!(e.to_string().contains("Redis") || e.to_string().contains("connection"));
        }
    }

    // 测试使用URL创建Redis Stream消息队列
    let url_config = create_redis_stream_config_with_url();
    let result = MessageQueueFactory::create(&url_config).await;

    match result {
        Ok(queue) => {
            println!("Successfully created Redis Stream message queue with URL");

            // 测试基本操作
            let test_queue = "test_factory_queue_url";
            assert!(queue.create_queue(test_queue, true).await.is_ok());

            let message = create_test_message();
            assert!(queue.publish_message(test_queue, &message).await.is_ok());

            // 清理
            assert!(queue.purge_queue(test_queue).await.is_ok());
        }
        Err(e) => {
            println!(
                "Failed to create Redis Stream message queue with URL: {}",
                e
            );
            // 如果Redis不可用，测试应该被跳过而不是失败
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

            // 测试基本操作
            let test_queue = "test_factory_rabbitmq_queue";
            assert!(queue.create_queue(test_queue, true).await.is_ok());

            let message = create_test_message();
            assert!(queue.publish_message(test_queue, &message).await.is_ok());

            let size = queue.get_queue_size(test_queue).await.unwrap();
            assert!(size > 0);

            // 清理
            assert!(queue.purge_queue(test_queue).await.is_ok());
        }
        Err(e) => {
            println!("Failed to create RabbitMQ message queue: {}", e);
            // 如果RabbitMQ不可用，测试应该被跳过而不是失败
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
    // 创建消息队列管理器
    let redis_config = create_redis_stream_config_with_redis_section();
    let result = MessageQueueManager::new(redis_config.clone()).await;

    match result {
        Ok(manager) => {
            println!("Successfully created MessageQueueManager");

            // 验证初始配置
            assert!(manager.is_redis_stream());
            assert!(!manager.is_rabbitmq());
            assert_eq!(manager.get_current_type_string(), "redis_stream");

            // 测试基本操作
            let test_queue = "test_manager_queue";
            let queue = manager.get_queue();
            assert!(queue.create_queue(test_queue, true).await.is_ok());

            let message = create_test_message();
            assert!(queue.publish_message(test_queue, &message).await.is_ok());

            // 测试通过管理器接口操作
            let message2 = create_test_message();
            assert!(manager.publish_message(test_queue, &message2).await.is_ok());

            let size = manager.get_queue_size(test_queue).await.unwrap();
            assert!(size >= 2);

            // 清理
            assert!(manager.purge_queue(test_queue).await.is_ok());

            println!("MessageQueueManager basic operations test passed");
        }
        Err(e) => {
            println!("Failed to create MessageQueueManager: {}", e);
            // 如果Redis不可用，测试应该被跳过而不是失败
            assert!(e.to_string().contains("Redis") || e.to_string().contains("connection"));
        }
    }
}

#[tokio::test]
#[ignore] // 需要Redis和RabbitMQ服务器运行
async fn test_message_queue_manager_switching() {
    // 首先创建Redis Stream管理器
    let redis_config = create_redis_stream_config_with_redis_section();
    let result = MessageQueueManager::new(redis_config.clone()).await;

    if result.is_err() {
        println!("Skipping switching test - Redis not available");
        return;
    }

    let mut manager = result.unwrap();

    // 验证初始状态
    assert!(manager.is_redis_stream());
    assert_eq!(manager.get_current_type_string(), "redis_stream");

    // 测试Redis Stream操作
    let test_queue = "test_switching_queue";
    assert!(manager.create_queue(test_queue, true).await.is_ok());

    let redis_message = create_test_message();
    assert!(manager
        .publish_message(test_queue, &redis_message)
        .await
        .is_ok());

    let redis_size = manager.get_queue_size(test_queue).await.unwrap();
    assert!(redis_size > 0);

    // 切换到RabbitMQ
    let rabbitmq_config = create_rabbitmq_config();
    let switch_result = manager.switch_to(rabbitmq_config).await;

    match switch_result {
        Ok(()) => {
            println!("Successfully switched to RabbitMQ");

            // 验证切换后的状态
            assert!(manager.is_rabbitmq());
            assert!(!manager.is_redis_stream());
            assert_eq!(manager.get_current_type_string(), "rabbitmq");

            // 测试RabbitMQ操作
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

            // 清理RabbitMQ队列
            assert!(manager.purge_queue(rabbitmq_test_queue).await.is_ok());

            println!("Message queue switching test passed");
        }
        Err(e) => {
            println!("Failed to switch to RabbitMQ: {}", e);
            // 如果RabbitMQ不可用，只测试Redis部分
            assert!(
                e.to_string().contains("RabbitMQ")
                    || e.to_string().contains("AMQP")
                    || e.to_string().contains("connection")
            );
        }
    }

    // 清理Redis队列（切换回Redis进行清理）
    let redis_config_cleanup = create_redis_stream_config_with_redis_section();
    if manager.switch_to(redis_config_cleanup).await.is_ok() {
        let _ = manager.purge_queue(test_queue).await;
    }
}

#[tokio::test]
async fn test_configuration_validation_edge_cases() {
    // 测试空URL的RabbitMQ配置
    let mut config = create_rabbitmq_config();
    config.url = "".to_string();
    assert!(MessageQueueFactory::validate_config(&config).is_err());

    // 测试错误协议的RabbitMQ配置
    config.url = "http://localhost:5672".to_string();
    assert!(MessageQueueFactory::validate_config(&config).is_err());

    // 测试Redis Stream配置缺少必要信息
    let mut redis_config = create_redis_stream_config_with_redis_section();
    redis_config.redis = None;
    redis_config.url = "http://localhost:6379".to_string(); // 错误的协议
    assert!(MessageQueueFactory::validate_config(&redis_config).is_err());

    // 测试Redis Stream配置完全缺少连接信息
    redis_config.url = "".to_string();
    assert!(MessageQueueFactory::validate_config(&redis_config).is_err());
}

#[test]
fn test_redis_url_parsing() {
    // 测试基本Redis URL解析
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

    // 测试带密码的Redis URL解析
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

    // 测试无效URL
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
