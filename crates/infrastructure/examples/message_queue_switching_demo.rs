use scheduler_core::{
    config::models::{MessageQueueConfig, MessageQueueType, RedisConfig},
    traits::MessageQueue,
};
use scheduler_domain::entities::*;
use scheduler_infrastructure::{MessageQueueFactory, MessageQueueManager};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

fn create_sample_message(content: &str) -> Message {
    let task_execution = TaskExecutionMessage {
        task_run_id: 123,
        task_id: 456,
        task_name: "demo_task".to_string(),
        task_type: "demo".to_string(),
        parameters: json!({"message": content, "timestamp": chrono::Utc::now()}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    Message::task_execution(task_execution)
}

fn create_redis_config() -> MessageQueueConfig {
    MessageQueueConfig {
        r#type: MessageQueueType::RedisStream,
        url: "redis://localhost:6379/0".to_string(),
        redis: Some(RedisConfig {
            host: "127.0.0.1".to_string(),
            port: 6379,
            database: 0,
            password: None,
            connection_timeout_seconds: 30,
            max_retry_attempts: 3,
            retry_delay_seconds: 1,
        }),
        task_queue: "demo_tasks".to_string(),
        status_queue: "demo_status".to_string(),
        heartbeat_queue: "demo_heartbeat".to_string(),
        control_queue: "demo_control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    }
}

fn create_rabbitmq_config() -> MessageQueueConfig {
    MessageQueueConfig {
        r#type: MessageQueueType::Rabbitmq,
        url: "amqp://guest:guest@localhost:5672/".to_string(),
        redis: None,
        task_queue: "demo_tasks".to_string(),
        status_queue: "demo_status".to_string(),
        heartbeat_queue: "demo_heartbeat".to_string(),
        control_queue: "demo_control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    }
}

async fn demonstrate_basic_operations(
    queue: &dyn MessageQueue,
    queue_name: &str,
    queue_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 演示 {} 基本操作 ===", queue_type);
    println!("1. 创建队列: {}", queue_name);
    queue.create_queue(queue_name, true).await?;
    println!("2. 发布消息到队列");
    let message1 = create_sample_message(&format!("Hello from {}", queue_type));
    let message2 = create_sample_message(&format!("Second message from {}", queue_type));

    queue.publish_message(queue_name, &message1).await?;
    queue.publish_message(queue_name, &message2).await?;

    println!("   已发布消息: {}", message1.id);
    println!("   已发布消息: {}", message2.id);
    let size = queue.get_queue_size(queue_name).await?;
    println!("3. 队列大小: {}", size);
    println!("4. 消费消息");
    let messages = queue.consume_messages(queue_name).await?;
    println!("   消费到 {} 条消息", messages.len());

    for (i, msg) in messages.iter().enumerate() {
        println!("   消息 {}: ID={}, 内容={:?}", i + 1, msg.id, msg.payload);
    }
    if !messages.is_empty() {
        println!("5. 确认第一条消息");
        queue.ack_message(&messages[0].id).await?;
        println!("   已确认消息: {}", messages[0].id);
    }
    println!("6. 清空队列");
    queue.purge_queue(queue_name).await?;

    let final_size = queue.get_queue_size(queue_name).await?;
    println!("   清空后队列大小: {}", final_size);

    Ok(())
}

async fn demonstrate_factory_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏭 消息队列工厂演示");
    println!("{}", "=".repeat(50));
    println!("\n--- 配置验证演示 ---");
    let redis_config = create_redis_config();
    let rabbitmq_config = create_rabbitmq_config();

    println!(
        "验证Redis Stream配置: {:?}",
        MessageQueueFactory::validate_config(&redis_config).is_ok()
    );
    println!(
        "验证RabbitMQ配置: {:?}",
        MessageQueueFactory::validate_config(&rabbitmq_config).is_ok()
    );
    println!("\n--- 创建Redis Stream消息队列 ---");
    match MessageQueueFactory::create(&redis_config).await {
        Ok(redis_queue) => {
            println!("✅ 成功创建Redis Stream消息队列");
            demonstrate_basic_operations(redis_queue.as_ref(), "demo_redis_queue", "Redis Stream")
                .await?;
        }
        Err(e) => {
            println!("❌ 创建Redis Stream消息队列失败: {}", e);
            println!("   请确保Redis服务器正在运行");
        }
    }
    println!("\n--- 创建RabbitMQ消息队列 ---");
    match MessageQueueFactory::create(&rabbitmq_config).await {
        Ok(rabbitmq_queue) => {
            println!("✅ 成功创建RabbitMQ消息队列");
            demonstrate_basic_operations(
                rabbitmq_queue.as_ref(),
                "demo_rabbitmq_queue",
                "RabbitMQ",
            )
            .await?;
        }
        Err(e) => {
            println!("❌ 创建RabbitMQ消息队列失败: {}", e);
            println!("   请确保RabbitMQ服务器正在运行");
        }
    }

    Ok(())
}

async fn demonstrate_runtime_switching() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔄 运行时切换演示");
    println!("{}", "=".repeat(50));
    let redis_config = create_redis_config();
    let manager_result = MessageQueueManager::new(redis_config).await;

    let mut manager = match manager_result {
        Ok(mgr) => {
            println!("✅ 成功创建消息队列管理器 (初始类型: Redis Stream)");
            mgr
        }
        Err(e) => {
            println!("❌ 创建消息队列管理器失败: {}", e);
            println!("   尝试使用RabbitMQ配置...");

            let rabbitmq_config = create_rabbitmq_config();
            match MessageQueueManager::new(rabbitmq_config).await {
                Ok(mgr) => {
                    println!("✅ 成功创建消息队列管理器 (初始类型: RabbitMQ)");
                    mgr
                }
                Err(e) => {
                    println!("❌ 创建消息队列管理器失败: {}", e);
                    println!("   请确保至少有一个消息队列服务器正在运行");
                    return Ok(());
                }
            }
        }
    };
    println!("\n--- 当前配置信息 ---");
    println!("消息队列类型: {}", manager.get_current_type_string());
    println!("是否为RabbitMQ: {}", manager.is_rabbitmq());
    println!("是否为Redis Stream: {}", manager.is_redis_stream());
    let test_queue = "switching_demo_queue";
    println!("\n--- 测试当前配置 ---");

    if let Err(e) = manager.create_queue(test_queue, true).await {
        println!("创建队列失败: {}", e);
        return Ok(());
    }

    let message = create_sample_message("测试消息 - 切换前");
    if let Err(e) = manager.publish_message(test_queue, &message).await {
        println!("发布消息失败: {}", e);
        return Ok(());
    }

    println!("✅ 成功发布消息: {}", message.id);

    let size = manager.get_queue_size(test_queue).await.unwrap_or(0);
    println!("队列大小: {}", size);
    println!("\n--- 尝试切换消息队列类型 ---");
    let switch_config = if manager.is_redis_stream() {
        println!("当前使用Redis Stream，尝试切换到RabbitMQ...");
        create_rabbitmq_config()
    } else {
        println!("当前使用RabbitMQ，尝试切换到Redis Stream...");
        create_redis_config()
    };

    match manager.switch_to(switch_config).await {
        Ok(()) => {
            println!("✅ 成功切换消息队列类型");
            println!("新的消息队列类型: {}", manager.get_current_type_string());
            println!("\n--- 测试切换后的配置 ---");
            let new_test_queue = "switched_demo_queue";

            if manager.create_queue(new_test_queue, true).await.is_ok() {
                let new_message = create_sample_message("测试消息 - 切换后");
                if manager
                    .publish_message(new_test_queue, &new_message)
                    .await
                    .is_ok()
                {
                    println!("✅ 切换后成功发布消息: {}", new_message.id);

                    let new_size = manager.get_queue_size(new_test_queue).await.unwrap_or(0);
                    println!("新队列大小: {}", new_size);
                    let _ = manager.purge_queue(new_test_queue).await;
                } else {
                    println!("❌ 切换后发布消息失败");
                }
            } else {
                println!("❌ 切换后创建队列失败");
            }
        }
        Err(e) => {
            println!("❌ 切换消息队列类型失败: {}", e);
            println!("   可能目标消息队列服务器未运行");
        }
    }
    let _ = manager.purge_queue(test_queue).await;

    Ok(())
}

async fn demonstrate_config_files() {
    println!("\n📄 配置文件演示");
    println!("{}", "=".repeat(50));

    println!("本演示展示了如何使用配置文件来配置消息队列:");
    println!();
    println!("1. Redis Stream配置文件: config/redis-stream.toml");
    println!("   - 使用 type = \"redis_stream\"");
    println!("   - 可以通过URL或redis配置段指定连接信息");
    println!();
    println!("2. RabbitMQ配置文件: config/rabbitmq.toml");
    println!("   - 使用 type = \"rabbitmq\"");
    println!("   - 通过URL指定AMQP连接信息");
    println!();
    println!("配置文件示例内容:");
    println!("```toml");
    println!("[message_queue]");
    println!("type = \"redis_stream\"  # 或 \"rabbitmq\"");
    println!("url = \"redis://localhost:6379/0\"");
    println!("task_queue = \"tasks\"");
    println!("# ... 其他配置");
    println!("```");
    println!();
    println!("使用配置文件:");
    println!("```rust");
    println!("let config = AppConfig::load(Some(\"config/redis-stream.toml\"))?;");
    println!("let manager = MessageQueueManager::new(config.message_queue).await?;");
    println!("```");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🚀 消息队列配置切换机制演示");
    println!("{}", "=".repeat(60));
    println!("本演示展示了如何在RabbitMQ和Redis Stream之间切换");
    println!("请确保至少有一个消息队列服务器正在运行");
    println!();
    if let Err(e) = demonstrate_factory_usage().await {
        println!("工厂演示出错: {}", e);
    }
    sleep(Duration::from_secs(1)).await;
    if let Err(e) = demonstrate_runtime_switching().await {
        println!("运行时切换演示出错: {}", e);
    }
    demonstrate_config_files().await;

    println!("\n✨ 演示完成！");
    println!("要在实际项目中使用，请:");
    println!("1. 根据需要选择配置文件 (config/redis-stream.toml 或 config/rabbitmq.toml)");
    println!("2. 使用 MessageQueueManager 进行运行时管理");
    println!("3. 使用 MessageQueueFactory 进行静态创建");

    Ok(())
}
