use anyhow::Result;
use scheduler_foundation::traits::MessageQueue;
use scheduler_domain::entities::*;
use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    println!("🚀 Redis Stream Message Consumption Demo");
    println!("========================================");
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "demo".to_string(),
        consumer_id: "demo_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };
    let queue = RedisStreamMessageQueue::new(config.clone()).await.unwrap();
    let demo_queue = "demo_consume_queue";

    println!("✅ Created Redis Stream message queue");
    let _ = queue.purge_queue(demo_queue).await;
    println!("🧹 Cleaned up existing queue data");
    println!("\n📤 Publishing test messages...");

    let messages: Vec<Message> = (1..=5)
        .map(|i| {
            let task_execution = TaskExecutionMessage {
                task_run_id: 1000 + i,
                task_id: 2000 + i,
                task_name: format!("demo_task_{}", i),
                task_type: "shell".to_string(),
                parameters: json!({
                    "command": format!("echo 'Demo task {} execution'", i),
                    "timeout": 30
                }),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };

            Message::task_execution(task_execution)
                .with_correlation_id(format!("demo-correlation-{}", i))
        })
        .collect();

    for (i, message) in messages.iter().enumerate() {
        queue.publish_message(demo_queue, message).await?;
        println!("  📨 Published message {}: {}", i + 1, message.id);
    }
    sleep(Duration::from_millis(100)).await;
    let queue_size = queue.get_queue_size(demo_queue).await?;
    println!("\n📊 Queue size: {} messages", queue_size);
    println!("\n📥 Consuming messages...");

    let consumed_messages = queue.consume_messages(demo_queue).await?;
    println!("  ✅ Consumed {} messages", consumed_messages.len());

    for (i, message) in consumed_messages.iter().enumerate() {
        println!(
            "  📋 Message {}: ID={}, Correlation={:?}, Type={}",
            i + 1,
            message.id,
            message.correlation_id,
            message.message_type_str()
        );

        if let MessageType::TaskExecution(task_msg) = &message.message_type {
            println!(
                "      Task: {} (ID: {})",
                task_msg.task_name, task_msg.task_id
            );
        }
    }
    println!("\n🔄 Consuming again (should get pending messages)...");

    let pending_messages = queue.consume_messages(demo_queue).await?;
    println!("  📋 Found {} pending messages", pending_messages.len());
    println!("\n👥 Demonstrating consumer groups...");
    let config2 = RedisStreamConfig {
        consumer_id: "demo_consumer_2".to_string(),
        ..config.clone()
    };
    let queue2 = RedisStreamMessageQueue::new(config2).await.unwrap();
    for i in 6..=8 {
        let task_execution = TaskExecutionMessage {
            task_run_id: 1000 + i,
            task_id: 2000 + i,
            task_name: format!("demo_task_{}", i),
            task_type: "shell".to_string(),
            parameters: json!({"command": format!("echo 'Multi-consumer task {}'", i)}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        queue.publish_message(demo_queue, &message).await?;
        println!(
            "  📨 Published message for multi-consumer test: {}",
            message.id
        );
    }

    sleep(Duration::from_millis(100)).await;
    let consumer1_messages = queue.consume_messages(demo_queue).await?;
    let consumer2_messages = queue2.consume_messages(demo_queue).await?;

    println!("  👤 Consumer 1 got {} messages", consumer1_messages.len());
    println!("  👤 Consumer 2 got {} messages", consumer2_messages.len());
    println!(
        "  📊 Total consumed: {} messages",
        consumer1_messages.len() + consumer2_messages.len()
    );
    println!("\n🧹 Cleaning up...");
    queue.purge_queue(demo_queue).await?;

    let final_size = queue.get_queue_size(demo_queue).await?;
    println!("  ✅ Final queue size: {}", final_size);

    println!("\n🎉 Demo completed successfully!");
    println!("\nKey features demonstrated:");
    println!("  ✓ Message publishing and consumption");
    println!("  ✓ Consumer group management");
    println!("  ✓ Pending message handling");
    println!("  ✓ Multiple consumers in same group");
    println!("  ✓ Message serialization/deserialization");
    println!("  ✓ Queue management operations");

    Ok(())
}
