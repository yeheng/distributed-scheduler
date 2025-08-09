use anyhow::Result;
use scheduler_domain::entities::{Message, TaskExecutionMessage};
use scheduler_core::traits::MessageQueue;
use scheduler_infrastructure::redis_stream::{RedisStreamConfig, RedisStreamMessageQueue};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use tokio::time::timeout;

#[tokio::test]
#[ignore] // 需要Redis服务器运行
async fn test_concurrent_message_publishing() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "perf_test".to_string(),
        consumer_id: "perf_test_consumer".to_string(),
        pool_min_idle: 5,
        pool_max_open: 20,
        pool_timeout_seconds: 10,
    };

    let queue = Arc::new(RedisStreamMessageQueue::new(config).await?);
    let test_queue = "performance_test_queue";
    let _ = queue.purge_queue(test_queue).await;
    const NUM_WORKERS: usize = 10;
    const MESSAGES_PER_WORKER: usize = 100;
    const TOTAL_MESSAGES: usize = NUM_WORKERS * MESSAGES_PER_WORKER;

    let barrier = Arc::new(Barrier::new(NUM_WORKERS));
    let published_count = Arc::new(AtomicU64::new(0));
    let start_time = Instant::now();
    let mut handles = Vec::new();
    for worker_id in 0..NUM_WORKERS {
        let queue_clone = queue.clone();
        let barrier_clone = barrier.clone();
        let published_count_clone = published_count.clone();
        let test_queue_clone = test_queue.to_string();

        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;

            for msg_id in 0..MESSAGES_PER_WORKER {
                let task_execution = TaskExecutionMessage {
                    task_run_id: (worker_id * MESSAGES_PER_WORKER + msg_id) as i64,
                    task_id: (worker_id * 1000 + msg_id) as i64,
                    task_name: format!("perf_test_task_{}_{}", worker_id, msg_id),
                    task_type: "shell".to_string(),
                    parameters: json!({"command": format!("echo 'worker {} message {}'", worker_id, msg_id)}),
                    timeout_seconds: 300,
                    retry_count: 0,
                    shard_index: None,
                    shard_total: None,
                };

                let message = Message::task_execution(task_execution)
                    .with_correlation_id(format!("perf-test-{}-{}", worker_id, msg_id));

                match timeout(
                    Duration::from_secs(10),
                    queue_clone.publish_message(&test_queue_clone, &message),
                )
                .await
                {
                    Ok(Ok(())) => {
                        published_count_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(Err(e)) => {
                        eprintln!(
                            "Worker {} failed to publish message {}: {}",
                            worker_id, msg_id, e
                        );
                    }
                    Err(_) => {
                        eprintln!("Worker {} message {} timed out", worker_id, msg_id);
                    }
                }
            }
        });

        handles.push(handle);
    }
    for handle in handles {
        handle.await?;
    }

    let duration = start_time.elapsed();
    let final_count = published_count.load(Ordering::Relaxed);
    println!("Performance Test Results:");
    println!("  Total messages: {}", TOTAL_MESSAGES);
    println!("  Successfully published: {}", final_count);
    println!("  Duration: {:?}", duration);
    println!(
        "  Messages per second: {:.2}",
        final_count as f64 / duration.as_secs_f64()
    );
    let queue_size = queue.get_queue_size(test_queue).await?;
    println!("  Final queue size: {}", queue_size);
    let metrics = queue.metrics();
    println!(
        "  Metrics - Published: {}, Errors: {}",
        metrics
            .messages_published
            .load(std::sync::atomic::Ordering::Relaxed),
        metrics
            .connection_errors
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    queue.purge_queue(test_queue).await?;
    assert!(
        final_count > 0,
        "Should have published at least some messages"
    );
    assert!(
        final_count as f64 / TOTAL_MESSAGES as f64 > 0.8,
        "Should have published at least 80% of messages successfully"
    );

    Ok(())
}

#[tokio::test]
#[ignore] // 需要Redis服务器运行
async fn test_message_consumption_performance() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "consume_perf_test".to_string(),
        consumer_id: "consume_perf_test_consumer".to_string(),
        pool_min_idle: 3,
        pool_max_open: 15,
        pool_timeout_seconds: 10,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let test_queue = "consumption_performance_test_queue";
    let _ = queue.purge_queue(test_queue).await;
    const NUM_MESSAGES: usize = 500;
    println!("Publishing {} test messages...", NUM_MESSAGES);

    let publish_start = Instant::now();
    for i in 0..NUM_MESSAGES {
        let task_execution = TaskExecutionMessage {
            task_run_id: i as i64,
            task_id: (i + 1000) as i64,
            task_name: format!("consume_perf_test_task_{}", i),
            task_type: "shell".to_string(),
            parameters: json!({"command": format!("echo 'consume test {}'", i)}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution)
            .with_correlation_id(format!("consume-perf-test-{}", i));

        queue.publish_message(test_queue, &message).await?;
    }

    let publish_duration = publish_start.elapsed();
    println!(
        "Published {} messages in {:?} ({:.2} msg/s)",
        NUM_MESSAGES,
        publish_duration,
        NUM_MESSAGES as f64 / publish_duration.as_secs_f64()
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    let consume_start = Instant::now();
    let mut total_consumed = 0;
    let mut total_acked = 0;
    while total_consumed < NUM_MESSAGES {
        let messages =
            timeout(Duration::from_secs(5), queue.consume_messages(test_queue)).await??;

        if messages.is_empty() {
            break;
        }

        total_consumed += messages.len();
        println!(
            "Consumed batch of {} messages (total: {})",
            messages.len(),
            total_consumed
        );
        let ack_start = Instant::now();
        for message in messages {
            match timeout(Duration::from_secs(2), queue.ack_message(&message.id)).await {
                Ok(Ok(())) => {
                    total_acked += 1;
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to ack message {}: {}", message.id, e);
                }
                Err(_) => {
                    eprintln!("Ack timeout for message {}", message.id);
                }
            }
        }
        let ack_duration = ack_start.elapsed();
        println!("Acked batch in {:?}", ack_duration);
        if total_consumed < NUM_MESSAGES {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let consume_duration = consume_start.elapsed();
    println!("Consumption Performance Results:");
    println!("  Total consumed: {}", total_consumed);
    println!("  Total acked: {}", total_acked);
    println!("  Consumption duration: {:?}", consume_duration);
    println!(
        "  Consumption rate: {:.2} msg/s",
        total_consumed as f64 / consume_duration.as_secs_f64()
    );
    let metrics = queue.metrics();
    println!("  Final metrics:");
    println!(
        "    Published: {}",
        metrics
            .messages_published
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Consumed: {}",
        metrics
            .messages_consumed
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Acked: {}",
        metrics
            .messages_acked
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Errors: {}",
        metrics
            .connection_errors
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    let final_queue_size = queue.get_queue_size(test_queue).await?;
    println!("  Final queue size: {}", final_queue_size);
    queue.purge_queue(test_queue).await?;
    assert!(total_consumed > 0, "Should have consumed some messages");
    assert!(total_acked > 0, "Should have acked some messages");
    assert_eq!(
        total_consumed, total_acked,
        "All consumed messages should be acked"
    );

    Ok(())
}

#[tokio::test]
#[ignore] // 需要Redis服务器运行
async fn test_connection_pool_efficiency() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 5,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
        consumer_group_prefix: "pool_test".to_string(),
        consumer_id: "pool_test_consumer".to_string(),
        pool_min_idle: 2,
        pool_max_open: 8,
        pool_timeout_seconds: 5,
    };

    let queue = RedisStreamMessageQueue::new(config).await?;
    let test_queue = "connection_pool_test_queue";
    let _ = queue.purge_queue(test_queue).await;
    println!("Testing health check performance...");
    let health_start = Instant::now();
    const NUM_HEALTH_CHECKS: usize = 100;

    for i in 0..NUM_HEALTH_CHECKS {
        let health = queue.health_check().await?;
        if !health.healthy {
            eprintln!("Health check {} failed: {:?}", i, health.error_message);
        }
    }

    let health_duration = health_start.elapsed();
    println!(
        "Completed {} health checks in {:?} ({:.2} checks/s)",
        NUM_HEALTH_CHECKS,
        health_duration,
        NUM_HEALTH_CHECKS as f64 / health_duration.as_secs_f64()
    );
    println!("Testing queue operations performance...");
    let ops_start = Instant::now();
    const NUM_OPERATIONS: usize = 50;

    for i in 0..NUM_OPERATIONS {
        let queue_name = format!("temp_queue_{}", i);
        queue.create_queue(&queue_name, true).await?;
        let _size = queue.get_queue_size(&queue_name).await?;
        queue.delete_queue(&queue_name).await?;
    }

    let ops_duration = ops_start.elapsed();
    println!(
        "Completed {} queue operations in {:?} ({:.2} ops/s)",
        NUM_OPERATIONS * 3,
        ops_duration,
        (NUM_OPERATIONS * 3) as f64 / ops_duration.as_secs_f64()
    );
    let metrics = queue.metrics();
    println!("Connection pool efficiency metrics:");
    println!(
        "  Active connections: {}",
        metrics
            .active_connections
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "  Connection errors: {}",
        metrics
            .connection_errors
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    queue.purge_queue(test_queue).await?;

    Ok(())
}

#[tokio::test]
#[ignore] // 需要Redis服务器运行，且耗时较长
async fn test_high_load_stress() -> Result<()> {
    let config = RedisStreamConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 10,
        max_retry_attempts: 5,
        retry_delay_seconds: 1,
        consumer_group_prefix: "stress_test".to_string(),
        consumer_id: "stress_test_consumer".to_string(),
        pool_min_idle: 10,
        pool_max_open: 50,
        pool_timeout_seconds: 15,
    };

    let queue = Arc::new(RedisStreamMessageQueue::new(config).await?);
    let test_queue = "stress_test_queue";
    let _ = queue.purge_queue(test_queue).await;

    println!("Starting high load stress test...");
    const NUM_PUBLISHERS: usize = 20;
    const NUM_CONSUMERS: usize = 10;
    const MESSAGES_PER_PUBLISHER: usize = 200;
    const TEST_DURATION_SECS: u64 = 30;

    let start_time = Instant::now();
    let published_count = Arc::new(AtomicU64::new(0));
    let consumed_count = Arc::new(AtomicU64::new(0));
    let acked_count = Arc::new(AtomicU64::new(0));
    let mut publisher_handles = Vec::new();
    for publisher_id in 0..NUM_PUBLISHERS {
        let queue_clone = queue.clone();
        let published_count_clone = published_count.clone();
        let test_queue_clone = test_queue.to_string();

        let handle = tokio::spawn(async move {
            for msg_id in 0..MESSAGES_PER_PUBLISHER {
                if start_time.elapsed().as_secs() >= TEST_DURATION_SECS {
                    break;
                }

                let task_execution = TaskExecutionMessage {
                    task_run_id: (publisher_id * MESSAGES_PER_PUBLISHER + msg_id) as i64,
                    task_id: (publisher_id * 10000 + msg_id) as i64,
                    task_name: format!("stress_test_task_{}_{}", publisher_id, msg_id),
                    task_type: "shell".to_string(),
                    parameters: json!({"command": format!("echo 'stress test {} {}'", publisher_id, msg_id)}),
                    timeout_seconds: 300,
                    retry_count: 0,
                    shard_index: None,
                    shard_total: None,
                };

                let message = Message::task_execution(task_execution)
                    .with_correlation_id(format!("stress-test-{}-{}", publisher_id, msg_id));

                match timeout(
                    Duration::from_secs(5),
                    queue_clone.publish_message(&test_queue_clone, &message),
                )
                .await
                {
                    Ok(Ok(())) => {
                        published_count_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(Err(e)) => {
                        eprintln!(
                            "Publisher {} failed to publish message {}: {}",
                            publisher_id, msg_id, e
                        );
                    }
                    Err(_) => {
                        eprintln!("Publisher {} message {} timed out", publisher_id, msg_id);
                    }
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });

        publisher_handles.push(handle);
    }
    let mut consumer_handles = Vec::new();
    for _consumer_id in 0..NUM_CONSUMERS {
        let queue_clone = queue.clone();
        let consumed_count_clone = consumed_count.clone();
        let acked_count_clone = acked_count.clone();
        let test_queue_clone = test_queue.to_string();

        let handle = tokio::spawn(async move {
            while start_time.elapsed().as_secs() < TEST_DURATION_SECS {
                match timeout(
                    Duration::from_secs(3),
                    queue_clone.consume_messages(&test_queue_clone),
                )
                .await
                {
                    Ok(Ok(messages)) => {
                        if !messages.is_empty() {
                            consumed_count_clone
                                .fetch_add(messages.len() as u64, Ordering::Relaxed);

                            for message in messages {
                                match timeout(
                                    Duration::from_secs(2),
                                    queue_clone.ack_message(&message.id),
                                )
                                .await
                                {
                                    Ok(Ok(())) => {
                                        acked_count_clone.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Ok(Err(_)) | Err(_) => {
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(_)) | Err(_) => {
                    }
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        consumer_handles.push(handle);
    }
    tokio::time::sleep(Duration::from_secs(TEST_DURATION_SECS)).await;
    for handle in publisher_handles {
        let _ = handle.await;
    }
    for handle in consumer_handles {
        let _ = handle.await;
    }

    let total_duration = start_time.elapsed();
    let final_published = published_count.load(Ordering::Relaxed);
    let final_consumed = consumed_count.load(Ordering::Relaxed);
    let final_acked = acked_count.load(Ordering::Relaxed);
    println!("Stress Test Results:");
    println!("  Test duration: {:?}", total_duration);
    println!("  Messages published: {}", final_published);
    println!("  Messages consumed: {}", final_consumed);
    println!("  Messages acked: {}", final_acked);
    println!(
        "  Publish rate: {:.2} msg/s",
        final_published as f64 / total_duration.as_secs_f64()
    );
    println!(
        "  Consume rate: {:.2} msg/s",
        final_consumed as f64 / total_duration.as_secs_f64()
    );
    let metrics = queue.metrics();
    println!("  Final metrics:");
    println!(
        "    Published: {}",
        metrics
            .messages_published
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Consumed: {}",
        metrics
            .messages_consumed
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Acked: {}",
        metrics
            .messages_acked
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Nacked: {}",
        metrics
            .messages_nacked
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Connection errors: {}",
        metrics
            .connection_errors
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Active connections: {}",
        metrics
            .active_connections
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    queue.purge_queue(test_queue).await?;
    assert!(final_published > 0, "Should have published some messages");
    assert!(final_consumed > 0, "Should have consumed some messages");

    Ok(())
}
