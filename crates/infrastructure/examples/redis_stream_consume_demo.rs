use anyhow::Result;
use scheduler_core::models::{Message, TaskExecutionMessage};
use scheduler_core::traits::MessageQueue;
use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

/// Demo showing Redis Stream message consumption functionality
///
/// This example demonstrates:
/// 1. Creating a Redis Stream message queue
/// 2. Publishing messages to a queue
/// 3. Consuming messages from the queue
/// 4. Consumer group management
/// 5. Handling pending messages
///
/// To run this demo:
/// 1. Start Redis server: `redis-server`
/// 2. Run: `cargo run --example redis_stream_consume_demo`
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Redis Stream Message Consumption Demo");
    println!("========================================");

    // Create Redis Stream configuration
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
    };

    // Create message queue instance
    let queue = RedisStreamMessageQueue::new(config.clone())?;
    let demo_queue = "demo_consume_queue";

    println!("✅ Created Redis Stream message queue");

    // Clean up any existing data
    let _ = queue.purge_queue(demo_queue).await;
    println!("🧹 Cleaned up existing queue data");

    // Step 1: Create and publish test messages
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

    // Wait a bit for messages to be available
    sleep(Duration::from_millis(100)).await;

    // Step 2: Check queue size
    let queue_size = queue.get_queue_size(demo_queue).await?;
    println!("\n📊 Queue size: {} messages", queue_size);

    // Step 3: Consume messages
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

        if let scheduler_core::models::MessageType::TaskExecution(task_msg) = &message.message_type
        {
            println!(
                "      Task: {} (ID: {})",
                task_msg.task_name, task_msg.task_id
            );
        }
    }

    // Step 4: Try consuming again (should get pending messages)
    println!("\n🔄 Consuming again (should get pending messages)...");

    let pending_messages = queue.consume_messages(demo_queue).await?;
    println!("  📋 Found {} pending messages", pending_messages.len());

    // Step 5: Demonstrate consumer group functionality
    println!("\n👥 Demonstrating consumer groups...");

    // Create a second consumer with different ID
    let config2 = RedisStreamConfig {
        consumer_id: "demo_consumer_2".to_string(),
        ..config.clone()
    };
    let queue2 = RedisStreamMessageQueue::new(config2)?;

    // Publish more messages
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

    // Both consumers consume from the same group
    let consumer1_messages = queue.consume_messages(demo_queue).await?;
    let consumer2_messages = queue2.consume_messages(demo_queue).await?;

    println!("  👤 Consumer 1 got {} messages", consumer1_messages.len());
    println!("  👤 Consumer 2 got {} messages", consumer2_messages.len());
    println!(
        "  📊 Total consumed: {} messages",
        consumer1_messages.len() + consumer2_messages.len()
    );

    // Step 6: Clean up
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
