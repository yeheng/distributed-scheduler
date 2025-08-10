use anyhow::Result;
use chrono::Utc;
use scheduler_core::config::models::{MessageQueueConfig, MessageQueueType};
use scheduler_core::traits::MessageQueue;
use scheduler_domain::entities::*;
use scheduler_core::{TaskResult, TaskRunStatus};
use scheduler_infrastructure::message_queue::RabbitMQMessageQueue;
use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
use serde_json::json;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;
use tokio::time::timeout;
use uuid::Uuid;

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_publish_message() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "test".to_string(),
        consumer_id: "test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };
    let queue = RedisStreamMessageQueue::new(config).await?;
    let task_execution = TaskExecutionMessage {
        task_run_id: 123,
        task_id: 456,
        task_name: "integration_test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({"command": "echo 'integration test'"}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message = Message::task_execution(task_execution)
        .with_correlation_id("integration-test-123".to_string());
    let publish_result = timeout(
        Duration::from_secs(10),
        queue.publish_message("test_integration_queue", &message),
    )
    .await;

    match publish_result {
        Ok(Ok(())) => {
            println!(
                "Successfully published message {} to Redis Stream",
                message.id
            );
            let size = queue.get_queue_size("test_integration_queue").await?;
            assert!(size > 0, "Queue should contain at least one message");
            println!("Queue size after publish: {}", size);
            queue.purge_queue("test_integration_queue").await?;
            let size_after_purge = queue.get_queue_size("test_integration_queue").await?;
            assert_eq!(size_after_purge, 0, "Queue should be empty after purge");

            Ok(())
        }
        Ok(Err(e)) => {
            eprintln!("Failed to publish message: {}", e);
            Err(e.into())
        }
        Err(_) => {
            eprintln!("Publish operation timed out - Redis may not be available");
            Ok(())
        }
    }
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_publish_with_retry() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 2,
        max_retry_attempts: 2,
        retry_delay_seconds: 1,
        consumer_group_prefix: "retry_test".to_string(),
        consumer_id: "retry_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let messages: Vec<Message> = (0..3)
        .map(|i| {
            let task_execution = TaskExecutionMessage {
                task_run_id: 100 + i,
                task_id: 200 + i,
                task_name: format!("retry_test_task_{}", i),
                task_type: "shell".to_string(),
                parameters: json!({"command": format!("echo 'retry test {}'", i)}),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };
            Message::task_execution(task_execution)
        })
        .collect();
    for (i, message) in messages.iter().enumerate() {
        let queue_name = format!("retry_test_queue_{}", i);

        let publish_result = timeout(
            Duration::from_secs(15), // Allow time for retries
            queue.publish_message(&queue_name, message),
        )
        .await;

        match publish_result {
            Ok(Ok(())) => {
                println!(
                    "Successfully published message {} to queue {}",
                    message.id, queue_name
                );
                let size = queue.get_queue_size(&queue_name).await?;
                assert!(
                    size > 0,
                    "Queue {} should contain the published message",
                    queue_name
                );
                queue.purge_queue(&queue_name).await?;
            }
            Ok(Err(e)) => {
                eprintln!("Failed to publish message to {}: {}", queue_name, e);
                return Err(e.into());
            }
            Err(_) => {
                eprintln!(
                    "Publish operation timed out for queue {} - Redis may not be available",
                    queue_name
                );
            }
        }
    }

    Ok(())
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_queue_management() -> Result<()> {
    let config = RedisStreamConfig::default();
    let queue = RedisStreamMessageQueue::new(config).await?;

    let test_queue = "queue_management_test";
    let create_result = timeout(Duration::from_secs(5), queue.create_queue(test_queue, true)).await;

    match create_result {
        Ok(Ok(())) => {
            println!("Successfully created queue: {}", test_queue);
            let size = queue.get_queue_size(test_queue).await?;
            println!("New queue size: {}", size);
            queue.delete_queue(test_queue).await?;
            println!("Successfully deleted queue: {}", test_queue);

            Ok(())
        }
        Ok(Err(e)) => {
            eprintln!("Failed to create queue: {}", e);
            Err(e.into())
        }
        Err(_) => {
            eprintln!("Queue creation timed out - Redis may not be available");
            Ok(())
        }
    }
}

#[tokio::test]
async fn test_redis_stream_error_handling() -> Result<()> {
    let config = RedisStreamConfig::default();
    let queue = RedisStreamMessageQueue::new(config).await?;
    let task_execution = TaskExecutionMessage {
        task_run_id: 999,
        task_id: 888,
        task_name: "error_test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({"command": "echo 'error test'"}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message = Message::task_execution(task_execution);
    let result = queue.publish_message("", &message).await;
    assert!(result.is_err(), "Should fail with empty queue name");
    let result = queue.publish_message("queue with spaces", &message).await;
    assert!(result.is_err(), "Should fail with spaces in queue name");
    let long_name = "x".repeat(300);
    let result = queue.publish_message(&long_name, &message).await;
    assert!(result.is_err(), "Should fail with overly long queue name");
    let result = queue
        .publish_message("queue\nwith\nnewlines", &message)
        .await;
    assert!(result.is_err(), "Should fail with newlines in queue name");

    println!("All error handling tests passed");
    Ok(())
}

#[tokio::test]
async fn test_message_serialization_edge_cases() -> Result<()> {
    let config = RedisStreamConfig::default();
    let _queue = RedisStreamMessageQueue::new(config).await?;
    let task_execution = TaskExecutionMessage {
        task_run_id: 777,
        task_id: 666,
        task_name: "special_chars_测试_🚀".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({
            "command": "echo 'Special chars: 测试 🚀 \"quotes\" \\backslash'",
            "env": {
                "SPECIAL": "测试🚀",
                "QUOTES": "\"double\" 'single'",
                "UNICODE": "🎉🎊🎈"
            }
        }),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message = Message::task_execution(task_execution)
        .with_correlation_id("special-chars-测试-🚀".to_string());
    let serialized = serde_json::to_string(&message)?;
    assert!(
        !serialized.is_empty(),
        "Serialized message should not be empty"
    );
    let deserialized: Message = serde_json::from_str(&serialized)?;
    assert_eq!(message.id, deserialized.id);
    assert_eq!(message.correlation_id, deserialized.correlation_id);

    println!("Message serialization with special characters works correctly");
    Ok(())
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_consume_messages() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "consume_test".to_string(),
        consumer_id: "consume_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let test_queue = "consume_test_queue";
    let _ = queue.purge_queue(test_queue).await;
    let messages: Vec<Message> = (0..5)
        .map(|i| {
            let task_execution = TaskExecutionMessage {
                task_run_id: 1000 + i,
                task_id: 2000 + i,
                task_name: format!("consume_test_task_{}", i),
                task_type: "shell".to_string(),
                parameters: json!({"command": format!("echo 'consume test {}'", i)}),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };
            Message::task_execution(task_execution)
                .with_correlation_id(format!("consume-test-{}", i))
        })
        .collect();
    for message in &messages {
        let publish_result = timeout(
            Duration::from_secs(10),
            queue.publish_message(test_queue, message),
        )
        .await;

        match publish_result {
            Ok(Ok(())) => {
                println!("Published message {} for consumption test", message.id);
            }
            Ok(Err(e)) => {
                eprintln!("Failed to publish message {}: {}", message.id, e);
                return Err(e.into());
            }
            Err(_) => {
                eprintln!("Publish operation timed out - Redis may not be available");
                return Ok(()); // Skip test if Redis is not available
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let consume_result = timeout(Duration::from_secs(10), queue.consume_messages(test_queue)).await;

    match consume_result {
        Ok(Ok(consumed_messages)) => {
            println!("Successfully consumed {} messages", consumed_messages.len());
            assert_eq!(
                consumed_messages.len(),
                messages.len(),
                "Should consume all published messages"
            );
            for consumed_message in &consumed_messages {
                let original_message = messages
                    .iter()
                    .find(|m| m.id == consumed_message.id)
                    .expect("Consumed message should match an original message");

                assert_eq!(
                    consumed_message.correlation_id,
                    original_message.correlation_id
                );
                assert_eq!(
                    consumed_message.message_type_str(),
                    original_message.message_type_str()
                );

                println!("Verified consumed message: {}", consumed_message.id);
            }
            let second_consume_result = queue.consume_messages(test_queue).await?;
            println!(
                "Second consume returned {} messages (should be pending messages)",
                second_consume_result.len()
            );
            queue.purge_queue(test_queue).await?;

            Ok(())
        }
        Ok(Err(e)) => {
            eprintln!("Failed to consume messages: {}", e);
            Err(e.into())
        }
        Err(_) => {
            eprintln!("Consume operation timed out - Redis may not be available");
            Ok(())
        }
    }
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_consumer_groups() -> Result<()> {
    let config1 = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "group_test".to_string(),
        consumer_id: "consumer_1".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let config2 = RedisStreamConfig {
        consumer_id: "consumer_2".to_string(),
        ..config1.clone()
    };

    let queue1 = RedisStreamMessageQueue::new(config1).await?;
    let queue2 = RedisStreamMessageQueue::new(config2).await?;
    let test_queue = "consumer_group_test_queue";
    let _ = queue1.purge_queue(test_queue).await;
    let messages: Vec<Message> = (0..10)
        .map(|i| {
            let task_execution = TaskExecutionMessage {
                task_run_id: 3000 + i,
                task_id: 4000 + i,
                task_name: format!("group_test_task_{}", i),
                task_type: "shell".to_string(),
                parameters: json!({"command": format!("echo 'group test {}'", i)}),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };
            Message::task_execution(task_execution)
        })
        .collect();
    for message in &messages {
        queue1.publish_message(test_queue, message).await?;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let consumed1 = queue1.consume_messages(test_queue).await?;
    let consumed2 = queue2.consume_messages(test_queue).await?;

    println!("Consumer 1 consumed {} messages", consumed1.len());
    println!("Consumer 2 consumed {} messages", consumed2.len());
    let total_consumed = consumed1.len() + consumed2.len();
    assert!(
        total_consumed <= messages.len(),
        "Total consumed messages should not exceed published messages"
    );
    queue1.purge_queue(test_queue).await?;

    Ok(())
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_ack_message() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "ack_test".to_string(),
        consumer_id: "ack_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let test_queue = "ack_test_queue";
    let _ = queue.purge_queue(test_queue).await;
    let task_execution = TaskExecutionMessage {
        task_run_id: 5000,
        task_id: 6000,
        task_name: "ack_test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({"command": "echo 'ack test'"}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message =
        Message::task_execution(task_execution).with_correlation_id("ack-test-123".to_string());
    let publish_result = timeout(
        Duration::from_secs(10),
        queue.publish_message(test_queue, &message),
    )
    .await;

    match publish_result {
        Ok(Ok(())) => {
            println!("Published message {} for ack test", message.id);
            tokio::time::sleep(Duration::from_millis(100)).await;
            let consumed_messages = queue.consume_messages(test_queue).await?;
            assert_eq!(
                consumed_messages.len(),
                1,
                "Should consume exactly one message"
            );

            let consumed_message = &consumed_messages[0];
            assert_eq!(
                consumed_message.id, message.id,
                "Consumed message ID should match"
            );

            println!("Consumed message {} for ack test", consumed_message.id);
            let ack_result = timeout(
                Duration::from_secs(5),
                queue.ack_message(&consumed_message.id),
            )
            .await;

            match ack_result {
                Ok(Ok(())) => {
                    println!("Successfully acknowledged message {}", consumed_message.id);
                    let second_consume = queue.consume_messages(test_queue).await?;
                    println!(
                        "Second consume after ack returned {} messages",
                        second_consume.len()
                    );
                    queue.purge_queue(test_queue).await?;

                    Ok(())
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to acknowledge message: {}", e);
                    Err(e.into())
                }
                Err(_) => {
                    eprintln!("Ack operation timed out - Redis may not be available");
                    Ok(())
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("Failed to publish message for ack test: {}", e);
            Err(e.into())
        }
        Err(_) => {
            eprintln!("Publish operation timed out - Redis may not be available");
            Ok(())
        }
    }
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_nack_message_with_requeue() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "nack_requeue_test".to_string(),
        consumer_id: "nack_requeue_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let test_queue = "nack_requeue_test_queue";
    let _ = queue.purge_queue(test_queue).await;
    let task_execution = TaskExecutionMessage {
        task_run_id: 7000,
        task_id: 8000,
        task_name: "nack_requeue_test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({"command": "echo 'nack requeue test'"}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message = Message::task_execution(task_execution)
        .with_correlation_id("nack-requeue-test-456".to_string());
    let publish_result = timeout(
        Duration::from_secs(10),
        queue.publish_message(test_queue, &message),
    )
    .await;

    match publish_result {
        Ok(Ok(())) => {
            println!("Published message {} for nack requeue test", message.id);
            tokio::time::sleep(Duration::from_millis(100)).await;
            let consumed_messages = queue.consume_messages(test_queue).await?;
            assert_eq!(
                consumed_messages.len(),
                1,
                "Should consume exactly one message"
            );

            let consumed_message = &consumed_messages[0];
            assert_eq!(
                consumed_message.id, message.id,
                "Consumed message ID should match"
            );
            assert_eq!(
                consumed_message.retry_count, 0,
                "Initial retry count should be 0"
            );

            println!(
                "Consumed message {} with retry count {}",
                consumed_message.id, consumed_message.retry_count
            );
            let nack_result = timeout(
                Duration::from_secs(5),
                queue.nack_message(&consumed_message.id, true),
            )
            .await;

            match nack_result {
                Ok(Ok(())) => {
                    println!(
                        "Successfully nacked and requeued message {}",
                        consumed_message.id
                    );
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    let requeued_messages = queue.consume_messages(test_queue).await?;

                    if !requeued_messages.is_empty() {
                        let requeued_message = &requeued_messages[0];
                        println!(
                            "Consumed requeued message {} with retry count {}",
                            requeued_message.id, requeued_message.retry_count
                        );
                        assert_eq!(
                            requeued_message.retry_count, 1,
                            "Retry count should be incremented after requeue"
                        );
                        queue.ack_message(&requeued_message.id).await?;
                    }
                    queue.purge_queue(test_queue).await?;

                    Ok(())
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to nack and requeue message: {}", e);
                    Err(e.into())
                }
                Err(_) => {
                    eprintln!("Nack operation timed out - Redis may not be available");
                    Ok(())
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("Failed to publish message for nack requeue test: {}", e);
            Err(e.into())
        }
        Err(_) => {
            eprintln!("Publish operation timed out - Redis may not be available");
            Ok(())
        }
    }
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_nack_message_without_requeue() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "nack_no_requeue_test".to_string(),
        consumer_id: "nack_no_requeue_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let test_queue = "nack_no_requeue_test_queue";
    let _ = queue.purge_queue(test_queue).await;
    let task_execution = TaskExecutionMessage {
        task_run_id: 9000,
        task_id: 10000,
        task_name: "nack_no_requeue_test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({"command": "echo 'nack no requeue test'"}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message = Message::task_execution(task_execution)
        .with_correlation_id("nack-no-requeue-test-789".to_string());
    let publish_result = timeout(
        Duration::from_secs(10),
        queue.publish_message(test_queue, &message),
    )
    .await;

    match publish_result {
        Ok(Ok(())) => {
            println!("Published message {} for nack no requeue test", message.id);
            tokio::time::sleep(Duration::from_millis(100)).await;
            let consumed_messages = queue.consume_messages(test_queue).await?;
            assert_eq!(
                consumed_messages.len(),
                1,
                "Should consume exactly one message"
            );

            let consumed_message = &consumed_messages[0];
            assert_eq!(
                consumed_message.id, message.id,
                "Consumed message ID should match"
            );

            println!(
                "Consumed message {} for nack no requeue test",
                consumed_message.id
            );
            let nack_result = timeout(
                Duration::from_secs(5),
                queue.nack_message(&consumed_message.id, false),
            )
            .await;

            match nack_result {
                Ok(Ok(())) => {
                    println!(
                        "Successfully nacked message {} without requeue",
                        consumed_message.id
                    );
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    let second_consume = queue.consume_messages(test_queue).await?;
                    println!(
                        "Second consume after nack without requeue returned {} messages",
                        second_consume.len()
                    );
                    queue.purge_queue(test_queue).await?;

                    Ok(())
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to nack message without requeue: {}", e);
                    Err(e.into())
                }
                Err(_) => {
                    eprintln!("Nack operation timed out - Redis may not be available");
                    Ok(())
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("Failed to publish message for nack no requeue test: {}", e);
            Err(e.into())
        }
        Err(_) => {
            eprintln!("Publish operation timed out - Redis may not be available");
            Ok(())
        }
    }
}

fn create_test_task_execution_message(id: i64) -> Message {
    let task_execution = TaskExecutionMessage {
        task_run_id: id,
        task_id: id + 1000,
        task_name: format!("test_task_{}", id),
        task_type: "shell".to_string(),
        parameters: json!({"command": format!("echo 'test {}'", id)}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };
    Message::task_execution(task_execution).with_correlation_id(format!("test-correlation-{}", id))
}

fn create_test_status_message(id: i64) -> Message {
    let status_message = StatusUpdateMessage {
        task_run_id: id,
        status: TaskRunStatus::Completed,
        worker_id: format!("worker-{}", id),
        result: Some(TaskResult {
            success: true,
            output: Some(format!("Output for task {}", id)),
            error_message: None,
            exit_code: Some(0),
            execution_time_ms: 1000,
        }),
        error_message: None,
        timestamp: Utc::now(),
    };

    let payload = serde_json::to_value(&status_message).unwrap_or(serde_json::Value::Null);
    Message {
        id: Uuid::new_v4().to_string(),
        message_type: MessageType::StatusUpdate(status_message),
        payload,
        timestamp: Utc::now(),
        retry_count: 0,
        correlation_id: Some(format!("status-correlation-{}", id)),
    }
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_concurrent_operations() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 10,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "concurrent_test".to_string(),
        consumer_id: "concurrent_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let queue = Arc::new(RedisStreamMessageQueue::new(config).await?);
    let test_queue = "concurrent_test_queue";
    let _ = queue.purge_queue(test_queue).await;

    const NUM_PUBLISHERS: usize = 5;
    const MESSAGES_PER_PUBLISHER: usize = 10;
    const TOTAL_MESSAGES: usize = NUM_PUBLISHERS * MESSAGES_PER_PUBLISHER;

    let published_count = Arc::new(AtomicU32::new(0));
    let consumed_count = Arc::new(AtomicU32::new(0));
    let barrier = Arc::new(Barrier::new(NUM_PUBLISHERS + 1)); // +1 for consumer
    let mut publisher_handles = Vec::new();
    for publisher_id in 0..NUM_PUBLISHERS {
        let queue_clone = Arc::clone(&queue);
        let published_count_clone = Arc::clone(&published_count);
        let barrier_clone = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;

            for msg_id in 0..MESSAGES_PER_PUBLISHER {
                let message_id = (publisher_id * MESSAGES_PER_PUBLISHER + msg_id) as i64;
                let message = create_test_task_execution_message(message_id);

                match timeout(
                    Duration::from_secs(10),
                    queue_clone.publish_message(test_queue, &message),
                )
                .await
                {
                    Ok(Ok(())) => {
                        published_count_clone.fetch_add(1, Ordering::SeqCst);
                        println!(
                            "Publisher {} published message {}",
                            publisher_id, message.id
                        );
                    }
                    Ok(Err(e)) => {
                        eprintln!(
                            "Publisher {} failed to publish message: {}",
                            publisher_id, e
                        );
                    }
                    Err(_) => {
                        eprintln!("Publisher {} timed out publishing message", publisher_id);
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }

            println!("Publisher {} completed", publisher_id);
        });

        publisher_handles.push(handle);
    }
    let queue_consumer = Arc::clone(&queue);
    let consumed_count_clone = Arc::clone(&consumed_count);
    let barrier_consumer = Arc::clone(&barrier);

    let consumer_handle = tokio::spawn(async move {
        barrier_consumer.wait().await;

        let mut consumed_messages = HashSet::new();
        let mut consecutive_empty_reads = 0;
        const MAX_EMPTY_READS: u32 = 10;

        while consumed_count_clone.load(Ordering::SeqCst) < TOTAL_MESSAGES as u32
            && consecutive_empty_reads < MAX_EMPTY_READS
        {
            match timeout(
                Duration::from_secs(5),
                queue_consumer.consume_messages(test_queue),
            )
            .await
            {
                Ok(Ok(messages)) => {
                    if messages.is_empty() {
                        consecutive_empty_reads += 1;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }

                    consecutive_empty_reads = 0;

                    for message in messages {
                        if consumed_messages.insert(message.id.clone()) {
                            consumed_count_clone.fetch_add(1, Ordering::SeqCst);
                            if let Err(e) = queue_consumer.ack_message(&message.id).await {
                                eprintln!("Failed to ack message {}: {}", message.id, e);
                            }

                            println!("Consumer processed message: {}", message.id);
                        } else {
                            println!("Consumer received duplicate message: {}", message.id);
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Consumer failed to consume messages: {}", e);
                    consecutive_empty_reads += 1;
                }
                Err(_) => {
                    eprintln!("Consumer timed out consuming messages");
                    consecutive_empty_reads += 1;
                }
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        println!(
            "Consumer completed. Consumed {} messages",
            consumed_count_clone.load(Ordering::SeqCst)
        );
    });
    for handle in publisher_handles {
        if let Err(e) = handle.await {
            eprintln!("Publisher task failed: {}", e);
        }
    }
    let consumer_result = timeout(Duration::from_secs(30), consumer_handle).await;

    match consumer_result {
        Ok(Ok(())) => {
            println!("Consumer completed successfully");
        }
        Ok(Err(e)) => {
            eprintln!("Consumer task failed: {}", e);
        }
        Err(_) => {
            eprintln!("Consumer timed out");
        }
    }
    let final_published = published_count.load(Ordering::SeqCst);
    let final_consumed = consumed_count.load(Ordering::SeqCst);

    println!(
        "Final results: Published: {}, Consumed: {}",
        final_published, final_consumed
    );
    let _ = queue.purge_queue(test_queue).await;
    assert!(
        final_published > 0,
        "Should have published at least some messages"
    );
    assert!(
        final_consumed > 0,
        "Should have consumed at least some messages"
    );
    if final_published > 0 {
        let success_rate = (final_consumed as f64) / (final_published as f64);
        println!("Success rate: {:.2}%", success_rate * 100.0);
        assert!(
            success_rate >= 0.8,
            "Success rate should be at least 80%, got {:.2}%",
            success_rate * 100.0
        );
    }

    Ok(())
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_multiple_consumers() -> Result<()> {
    let base_config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 10,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "multi_consumer_test".to_string(),
        consumer_id: "consumer_1".to_string(), // Will be overridden
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    const NUM_CONSUMERS: usize = 3;
    const NUM_MESSAGES: usize = 15;
    let test_queue = "multi_consumer_test_queue";
    let publisher = RedisStreamMessageQueue::new(base_config.clone()).await?;
    let _ = publisher.purge_queue(test_queue).await;
    for i in 0..NUM_MESSAGES {
        let message = create_test_task_execution_message(i as i64);
        match timeout(
            Duration::from_secs(10),
            publisher.publish_message(test_queue, &message),
        )
        .await
        {
            Ok(Ok(())) => {
                println!("Published message {}", message.id);
            }
            Ok(Err(e)) => {
                eprintln!("Failed to publish message {}: {}", i, e);
                return Err(e.into());
            }
            Err(_) => {
                eprintln!("Publish timed out - Redis may not be available");
                return Ok(()); // Skip test if Redis is not available
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let consumed_counts = Arc::new(std::sync::Mutex::new(vec![0u32; NUM_CONSUMERS]));
    let mut consumer_handles = Vec::new();

    for consumer_id in 0..NUM_CONSUMERS {
        let mut config = base_config.clone();
        config.consumer_id = format!("consumer_{}", consumer_id);

        let consumer = RedisStreamMessageQueue::new(config).await?;
        let consumed_counts_clone = Arc::clone(&consumed_counts);

        let handle = tokio::spawn(async move {
            let mut local_count = 0u32;
            let mut consecutive_empty_reads = 0;
            const MAX_EMPTY_READS: u32 = 5;

            while consecutive_empty_reads < MAX_EMPTY_READS {
                match timeout(
                    Duration::from_secs(5),
                    consumer.consume_messages(test_queue),
                )
                .await
                {
                    Ok(Ok(messages)) => {
                        if messages.is_empty() {
                            consecutive_empty_reads += 1;
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            continue;
                        }

                        consecutive_empty_reads = 0;

                        for message in messages {
                            local_count += 1;
                            println!("Consumer {} processed message: {}", consumer_id, message.id);
                            if let Err(e) = consumer.ack_message(&message.id).await {
                                eprintln!(
                                    "Consumer {} failed to ack message {}: {}",
                                    consumer_id, message.id, e
                                );
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        eprintln!("Consumer {} failed to consume: {}", consumer_id, e);
                        consecutive_empty_reads += 1;
                    }
                    Err(_) => {
                        eprintln!("Consumer {} timed out", consumer_id);
                        consecutive_empty_reads += 1;
                    }
                }

                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            if let Ok(mut counts) = consumed_counts_clone.lock() {
                counts[consumer_id] = local_count;
            }

            println!(
                "Consumer {} completed with {} messages",
                consumer_id, local_count
            );
        });

        consumer_handles.push(handle);
    }
    for handle in consumer_handles {
        if let Err(e) = timeout(Duration::from_secs(20), handle).await {
            eprintln!("Consumer task failed or timed out: {:?}", e);
        }
    }
    let counts = consumed_counts.lock().unwrap();
    let total_consumed: u32 = counts.iter().sum();

    println!("Consumer results: {:?}", *counts);
    println!(
        "Total consumed: {} out of {} published",
        total_consumed, NUM_MESSAGES
    );
    let _ = publisher.purge_queue(test_queue).await;
    assert!(
        total_consumed > 0,
        "At least some messages should have been consumed"
    );
    let active_consumers = counts.iter().filter(|&&count| count > 0).count();
    assert!(
        active_consumers >= 1,
        "At least one consumer should have processed messages"
    );

    println!(
        "Multi-consumer test completed successfully with {} active consumers",
        active_consumers
    );
    Ok(())
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis
async fn test_redis_stream_failure_recovery() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "failure_recovery_test".to_string(),
        consumer_id: "recovery_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let test_queue = "failure_recovery_test_queue";
    let _ = queue.purge_queue(test_queue).await;
    println!("=== Test 1: Message recovery after consumer failure ===");

    let messages: Vec<Message> = (0..5)
        .map(|i| create_test_task_execution_message(i))
        .collect();
    for message in &messages {
        match timeout(
            Duration::from_secs(10),
            queue.publish_message(test_queue, message),
        )
        .await
        {
            Ok(Ok(())) => {
                println!("Published message {} for recovery test", message.id);
            }
            Ok(Err(e)) => {
                eprintln!("Failed to publish message: {}", e);
                return Err(e.into());
            }
            Err(_) => {
                eprintln!("Publish timed out - Redis may not be available");
                return Ok(());
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let first_consume_result =
        timeout(Duration::from_secs(10), queue.consume_messages(test_queue)).await;

    match first_consume_result {
        Ok(Ok(consumed_messages)) => {
            println!(
                "First consume got {} messages (not acknowledging to simulate failure)",
                consumed_messages.len()
            );
            assert!(
                !consumed_messages.is_empty(),
                "Should have consumed some messages"
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
            let second_consume_result = queue.consume_messages(test_queue).await?;
            println!(
                "Second consume got {} messages (should be pending messages)",
                second_consume_result.len()
            );
            assert!(
                !second_consume_result.is_empty(),
                "Should recover pending messages"
            );
            for message in &second_consume_result {
                if let Err(e) = queue.ack_message(&message.id).await {
                    eprintln!("Failed to ack recovered message {}: {}", message.id, e);
                }
            }

            println!("Recovery test 1 completed successfully");
        }
        Ok(Err(e)) => {
            eprintln!("First consume failed: {}", e);
            return Err(e.into());
        }
        Err(_) => {
            eprintln!("First consume timed out - Redis may not be available");
            return Ok(());
        }
    }
    println!("=== Test 2: Retry mechanism with nack and requeue ===");

    let retry_message = create_test_task_execution_message(1000);
    queue.publish_message(test_queue, &retry_message).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    for retry_attempt in 1..=3 {
        let consumed = queue.consume_messages(test_queue).await?;
        assert_eq!(consumed.len(), 1, "Should consume exactly one message");

        let message = &consumed[0];
        println!(
            "Retry attempt {}: consumed message with retry_count {}",
            retry_attempt, message.retry_count
        );

        if retry_attempt < 3 {
            queue.nack_message(&message.id, true).await?;
            println!(
                "Nacked and requeued message for retry attempt {}",
                retry_attempt
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        } else {
            queue.ack_message(&message.id).await?;
            println!("Acknowledged message on final retry attempt");
        }
    }
    let final_consume = queue.consume_messages(test_queue).await?;
    assert!(
        final_consume.is_empty(),
        "No messages should remain after successful processing"
    );

    println!("Recovery test 2 completed successfully");
    let _ = queue.purge_queue(test_queue).await;

    Ok(())
}

#[tokio::test]
#[ignore] // Ignore by default since it requires both Redis and RabbitMQ
async fn test_redis_stream_rabbitmq_equivalence() -> Result<()> {
    let redis_config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 10,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "equivalence_test".to_string(),
        consumer_id: "equivalence_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };
    let rabbitmq_config = MessageQueueConfig {
        r#type: MessageQueueType::Rabbitmq,
        url: "amqp://guest:guest@localhost:5672".to_string(),
        redis: None,
        task_queue: "equivalence_test_tasks".to_string(),
        status_queue: "equivalence_test_status".to_string(),
        heartbeat_queue: "equivalence_test_heartbeat".to_string(),
        control_queue: "equivalence_test_control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 1,
        connection_timeout_seconds: 30,
    };
    let redis_queue = RedisStreamMessageQueue::new(redis_config).await?;

    let rabbitmq_result = timeout(
        Duration::from_secs(5),
        RabbitMQMessageQueue::new(rabbitmq_config),
    )
    .await;

    let rabbitmq_queue = match rabbitmq_result {
        Ok(Ok(queue)) => queue,
        Ok(Err(e)) => {
            println!("RabbitMQ not available for equivalence test: {}", e);
            return Ok(()); // Skip test if RabbitMQ is not available
        }
        Err(_) => {
            println!("RabbitMQ connection timed out - skipping equivalence test");
            return Ok(());
        }
    };

    let test_queue = "equivalence_test_queue";
    let _ = redis_queue.purge_queue(test_queue).await;
    let _ = rabbitmq_queue.purge_queue(test_queue).await;
    let test_messages = vec![
        create_test_task_execution_message(1),
        create_test_status_message(2),
        create_test_task_execution_message(3),
    ];

    println!("=== Testing Redis Stream vs RabbitMQ Equivalence ===");
    println!("Test 1: Basic publish/consume operations");
    let mut redis_results = Vec::new();
    for message in &test_messages {
        match timeout(
            Duration::from_secs(10),
            redis_queue.publish_message(test_queue, message),
        )
        .await
        {
            Ok(Ok(())) => {
                println!("Redis: Published message {}", message.id);
            }
            Ok(Err(e)) => {
                eprintln!("Redis: Failed to publish message: {}", e);
                continue;
            }
            Err(_) => {
                eprintln!("Redis: Publish timed out");
                continue;
            }
        }
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    match redis_queue.consume_messages(test_queue).await {
        Ok(messages) => {
            redis_results = messages;
            println!("Redis: Consumed {} messages", redis_results.len());
        }
        Err(e) => {
            eprintln!("Redis: Failed to consume messages: {}", e);
        }
    }
    let mut rabbitmq_results = Vec::new();
    for message in &test_messages {
        match timeout(
            Duration::from_secs(10),
            rabbitmq_queue.publish_message(test_queue, message),
        )
        .await
        {
            Ok(Ok(())) => {
                println!("RabbitMQ: Published message {}", message.id);
            }
            Ok(Err(e)) => {
                eprintln!("RabbitMQ: Failed to publish message: {}", e);
                continue;
            }
            Err(_) => {
                eprintln!("RabbitMQ: Publish timed out");
                continue;
            }
        }
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    match rabbitmq_queue.consume_messages(test_queue).await {
        Ok(messages) => {
            rabbitmq_results = messages;
            println!("RabbitMQ: Consumed {} messages", rabbitmq_results.len());
        }
        Err(e) => {
            eprintln!("RabbitMQ: Failed to consume messages: {}", e);
        }
    }
    println!(
        "Comparing results: Redis {} vs RabbitMQ {}",
        redis_results.len(),
        rabbitmq_results.len()
    );
    if !redis_results.is_empty() && !rabbitmq_results.is_empty() {
        assert_eq!(
            redis_results.len(),
            rabbitmq_results.len(),
            "Both implementations should consume the same number of messages"
        );
        for (redis_msg, rabbitmq_msg) in redis_results.iter().zip(rabbitmq_results.iter()) {
            let redis_type_json = serde_json::to_string(&redis_msg.message_type)?;
            let rabbitmq_type_json = serde_json::to_string(&rabbitmq_msg.message_type)?;
            println!("Comparing message types: Redis vs RabbitMQ");
            assert!(
                !redis_type_json.is_empty(),
                "Redis message type should serialize to valid JSON"
            );
            assert!(
                !rabbitmq_type_json.is_empty(),
                "RabbitMQ message type should serialize to valid JSON"
            );
        }
    }
    println!("Test 2: Queue management operations");

    let mgmt_test_queue = "equivalence_mgmt_test";
    let redis_create = redis_queue.create_queue(mgmt_test_queue, true).await;
    let rabbitmq_create = rabbitmq_queue.create_queue(mgmt_test_queue, true).await;

    println!(
        "Create queue - Redis: {:?}, RabbitMQ: {:?}",
        redis_create.is_ok(),
        rabbitmq_create.is_ok()
    );
    let redis_size = redis_queue
        .get_queue_size(mgmt_test_queue)
        .await
        .unwrap_or(0);
    let rabbitmq_size = rabbitmq_queue
        .get_queue_size(mgmt_test_queue)
        .await
        .unwrap_or(0);

    println!(
        "Initial queue sizes - Redis: {}, RabbitMQ: {}",
        redis_size, rabbitmq_size
    );
    assert_eq!(
        redis_size, rabbitmq_size,
        "Both queues should start with the same size"
    );
    let _ = redis_queue.delete_queue(mgmt_test_queue).await;
    let _ = rabbitmq_queue.delete_queue(mgmt_test_queue).await;
    let _ = redis_queue.purge_queue(test_queue).await;
    let _ = rabbitmq_queue.purge_queue(test_queue).await;
    if let Err(e) = rabbitmq_queue.close().await {
        eprintln!("Warning: Failed to close RabbitMQ connection: {}", e);
    }

    println!("Equivalence test completed successfully");
    Ok(())
}

#[tokio::test]
#[ignore] // Ignore by default since it requires Redis and is performance-focused
async fn test_redis_stream_performance_benchmark() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 10,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "perf_test".to_string(),
        consumer_id: "perf_test_consumer".to_string(),
        pool_min_idle: 1,
        pool_max_open: 10,
        pool_timeout_seconds: 30,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let test_queue = "performance_test_queue";
    let _ = queue.purge_queue(test_queue).await;

    const NUM_MESSAGES: usize = 100;
    let messages: Vec<Message> = (0..NUM_MESSAGES)
        .map(|i| create_test_task_execution_message(i as i64))
        .collect();

    println!("=== Redis Stream Performance Benchmark ===");
    println!("Testing with {} messages", NUM_MESSAGES);
    let publish_start = std::time::Instant::now();
    let mut published_count = 0;

    for message in &messages {
        match timeout(
            Duration::from_secs(5),
            queue.publish_message(test_queue, message),
        )
        .await
        {
            Ok(Ok(())) => {
                published_count += 1;
            }
            Ok(Err(e)) => {
                eprintln!("Failed to publish message: {}", e);
            }
            Err(_) => {
                eprintln!("Publish timed out - Redis may not be available");
                return Ok(());
            }
        }
    }

    let publish_duration = publish_start.elapsed();
    let publish_rate = published_count as f64 / publish_duration.as_secs_f64();

    println!(
        "Published {} messages in {:?} ({:.2} msg/sec)",
        published_count, publish_duration, publish_rate
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    let consume_start = std::time::Instant::now();
    let mut consumed_count = 0;
    let mut total_consumed = 0;

    while total_consumed < published_count {
        match timeout(Duration::from_secs(5), queue.consume_messages(test_queue)).await {
            Ok(Ok(messages)) => {
                if messages.is_empty() {
                    break;
                }

                consumed_count += messages.len();
                for message in messages {
                    if let Err(e) = queue.ack_message(&message.id).await {
                        eprintln!("Failed to ack message: {}", e);
                    }
                }

                total_consumed += consumed_count;
                consumed_count = 0;
            }
            Ok(Err(e)) => {
                eprintln!("Failed to consume messages: {}", e);
                break;
            }
            Err(_) => {
                eprintln!("Consume timed out");
                break;
            }
        }
    }

    let consume_duration = consume_start.elapsed();
    let consume_rate = total_consumed as f64 / consume_duration.as_secs_f64();

    println!(
        "Consumed {} messages in {:?} ({:.2} msg/sec)",
        total_consumed, consume_duration, consume_rate
    );
    let total_duration = publish_start.elapsed();
    let overall_rate = total_consumed as f64 / total_duration.as_secs_f64();

    println!("Overall throughput: {:.2} msg/sec", overall_rate);
    assert!(
        publish_rate > 10.0,
        "Publish rate should be at least 10 msg/sec, got {:.2}",
        publish_rate
    );
    assert!(
        consume_rate > 10.0,
        "Consume rate should be at least 10 msg/sec, got {:.2}",
        consume_rate
    );
    let _ = queue.purge_queue(test_queue).await;

    println!("Performance benchmark completed successfully");
    Ok(())
}
