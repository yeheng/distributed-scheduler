use anyhow::Result;
use scheduler_core::models::{Message, TaskExecutionMessage};
use scheduler_core::traits::MessageQueue;
use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

/// Demonstrates Redis Stream message publishing functionality
///
/// To run this example:
/// 1. Start a Redis server on localhost:6379
/// 2. Run: cargo run --example redis_stream_publish_demo
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Redis Stream Message Publishing Demo");
    println!("========================================");

    // Create Redis Stream configuration
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 10,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "demo".to_string(),
        consumer_id: "demo_publisher".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    println!("📡 Connecting to Redis at {}:{}", config.host, config.port);

    // Create message queue instance
    let queue = match RedisStreamMessageQueue::new(config).await {
        Ok(q) => {
            println!("✅ Successfully created Redis Stream message queue");
            q
        }
        Err(e) => {
            eprintln!("❌ Failed to create Redis Stream message queue: {}", e);
            eprintln!("💡 Make sure Redis is running on localhost:6379");
            return Err(e.into());
        }
    };

    // Demo 1: Publish a simple task execution message
    println!("\n📝 Demo 1: Publishing a simple task execution message");
    let task_execution = TaskExecutionMessage {
        task_run_id: 1001,
        task_id: 2001,
        task_name: "demo_backup_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({
            "command": "rsync -av /data/ /backup/",
            "timeout": 3600,
            "env": {
                "BACKUP_DIR": "/backup",
                "LOG_LEVEL": "INFO"
            }
        }),
        timeout_seconds: 3600,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message1 =
        Message::task_execution(task_execution).with_correlation_id("demo-backup-001".to_string());

    match queue.publish_message("demo_tasks", &message1).await {
        Ok(()) => {
            println!("✅ Published message {} to 'demo_tasks' queue", message1.id);
        }
        Err(e) => {
            eprintln!("❌ Failed to publish message: {}", e);
            return Err(e.into());
        }
    }

    // Demo 2: Publish multiple messages with different types
    println!("\n📝 Demo 2: Publishing multiple messages");
    let messages = vec![
        // Data processing task
        {
            let task_execution = TaskExecutionMessage {
                task_run_id: 1002,
                task_id: 2002,
                task_name: "data_processing".to_string(),
                task_type: "python".to_string(),
                parameters: json!({
                    "script": "process_data.py",
                    "input_file": "/data/raw/input.csv",
                    "output_file": "/data/processed/output.json"
                }),
                timeout_seconds: 1800,
                retry_count: 0,
                shard_index: Some(1),
                shard_total: Some(4),
            };
            Message::task_execution(task_execution)
                .with_correlation_id("demo-processing-001".to_string())
        },
        // Email notification task
        {
            let task_execution = TaskExecutionMessage {
                task_run_id: 1003,
                task_id: 2003,
                task_name: "send_notification".to_string(),
                task_type: "http".to_string(),
                parameters: json!({
                    "url": "https://api.example.com/notify",
                    "method": "POST",
                    "headers": {
                        "Content-Type": "application/json",
                        "Authorization": "Bearer token123"
                    },
                    "body": {
                        "message": "Task completed successfully",
                        "recipient": "admin@example.com"
                    }
                }),
                timeout_seconds: 30,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };
            Message::task_execution(task_execution)
                .with_correlation_id("demo-notification-001".to_string())
        },
        // Database cleanup task
        {
            let task_execution = TaskExecutionMessage {
                task_run_id: 1004,
                task_id: 2004,
                task_name: "cleanup_old_data".to_string(),
                task_type: "sql".to_string(),
                parameters: json!({
                    "query": "DELETE FROM logs WHERE created_at < NOW() - INTERVAL '30 days'",
                    "database": "production",
                    "dry_run": false
                }),
                timeout_seconds: 600,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };
            Message::task_execution(task_execution)
                .with_correlation_id("demo-cleanup-001".to_string())
        },
    ];

    for (i, message) in messages.iter().enumerate() {
        let queue_name = format!("demo_queue_{}", i + 1);

        match queue.publish_message(&queue_name, message).await {
            Ok(()) => {
                println!("✅ Published message {} to '{}'", message.id, queue_name);
            }
            Err(e) => {
                eprintln!("❌ Failed to publish message to '{}': {}", queue_name, e);
            }
        }

        // Small delay between messages
        sleep(Duration::from_millis(100)).await;
    }

    // Demo 3: Check queue sizes
    println!("\n📊 Demo 3: Checking queue sizes");
    let queues = vec!["demo_tasks", "demo_queue_1", "demo_queue_2", "demo_queue_3"];

    for queue_name in &queues {
        match queue.get_queue_size(queue_name).await {
            Ok(size) => {
                println!("📈 Queue '{}' contains {} messages", queue_name, size);
            }
            Err(e) => {
                eprintln!("❌ Failed to get size for queue '{}': {}", queue_name, e);
            }
        }
    }

    // Demo 4: Test retry mechanism with invalid queue name
    println!("\n🔄 Demo 4: Testing error handling and retry mechanism");
    let invalid_message = Message::task_execution(TaskExecutionMessage {
        task_run_id: 9999,
        task_id: 9999,
        task_name: "invalid_test".to_string(),
        task_type: "test".to_string(),
        parameters: json!({}),
        timeout_seconds: 60,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    });

    // This should fail due to invalid queue name
    match queue
        .publish_message("invalid queue name", &invalid_message)
        .await
    {
        Ok(()) => {
            println!("⚠️  Unexpected success with invalid queue name");
        }
        Err(e) => {
            println!("✅ Correctly rejected invalid queue name: {}", e);
        }
    }

    // Demo 5: Cleanup - purge test queues
    println!("\n🧹 Demo 5: Cleaning up test queues");
    for queue_name in &queues {
        match queue.purge_queue(queue_name).await {
            Ok(()) => {
                println!("🗑️  Purged queue '{}'", queue_name);
            }
            Err(e) => {
                eprintln!("❌ Failed to purge queue '{}': {}", queue_name, e);
            }
        }
    }

    println!("\n🎉 Demo completed successfully!");
    println!("💡 Check your Redis instance to see the streams that were created");

    Ok(())
}
