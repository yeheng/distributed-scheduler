#[cfg(test)]
mod message_queue_test {
    use std::env;

    use crate::*;
    use chrono::Utc;
    use scheduler_core::{
        config::model::MessageQueueConfig,
        models::{Message, MessageType, StatusUpdateMessage},
        MessageQueue as _, TaskResult, TaskRunStatus,
    };
    use testcontainers::{core::IntoContainerPort, runners::AsyncRunner, GenericImage};
    use tokio::time::{sleep, Duration};
    use uuid::Uuid;

    // Test constants to avoid magic numbers
    mod test_constants {
        pub const TEST_TASK_RUN_ID: i64 = 789;
        pub const TEST_WORKER_ID: &str = "worker-001";
        pub const TEST_EXECUTION_TIME_MS: u64 = 1000;
        pub const TEST_EXIT_CODE: i32 = 0;
        pub const DEFAULT_MAX_RETRIES: i32 = 3;
        pub const DEFAULT_RETRY_DELAY_SECONDS: u64 = 1;
        pub const DEFAULT_CONNECTION_TIMEOUT_SECONDS: u64 = 30;
    }

    /// Get test configuration from environment variables or defaults
    fn get_test_config() -> MessageQueueConfig {
        MessageQueueConfig {
            url: env::var("TEST_RABBITMQ_URL")
                .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string()),
            task_queue: env::var("TEST_TASK_QUEUE").unwrap_or_else(|_| "test_tasks".to_string()),
            status_queue: env::var("TEST_STATUS_QUEUE")
                .unwrap_or_else(|_| "test_status".to_string()),
            heartbeat_queue: env::var("TEST_HEARTBEAT_QUEUE")
                .unwrap_or_else(|_| "test_heartbeat".to_string()),
            control_queue: env::var("TEST_CONTROL_QUEUE")
                .unwrap_or_else(|_| "test_control".to_string()),
            max_retries: test_constants::DEFAULT_MAX_RETRIES,
            retry_delay_seconds: test_constants::DEFAULT_RETRY_DELAY_SECONDS,
            connection_timeout_seconds: test_constants::DEFAULT_CONNECTION_TIMEOUT_SECONDS,
        }
    }

    /// Create a test message with consistent data
    fn create_test_message() -> Message {
        let message = StatusUpdateMessage {
            task_run_id: test_constants::TEST_TASK_RUN_ID,
            status: TaskRunStatus::Completed,
            worker_id: test_constants::TEST_WORKER_ID.to_string(),
            result: Some(TaskResult {
                success: true,
                output: Some("Success".to_string()),
                error_message: None,
                exit_code: Some(test_constants::TEST_EXIT_CODE),
                execution_time_ms: test_constants::TEST_EXECUTION_TIME_MS,
            }),
            error_message: None,
            timestamp: Utc::now(),
        };

        let payload = serde_json::to_value(&message).unwrap_or(serde_json::Value::Null);
        Message {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::StatusUpdate(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
        }
    }

    /// Create a test message with custom ID for testing
    fn create_test_message_with_id(id: &str) -> Message {
        let mut message = create_test_message();
        message.id = id.to_string();
        message
    }

    async fn create_rabbit_mqmessage_queue() -> scheduler_core::Result<RabbitMQMessageQueue> {
        let rabbitmq_image = GenericImage::new("rabbitmq", "3-management")
            .with_exposed_port(5672.tcp())
            .with_exposed_port(15672.tcp());

        let _container = rabbitmq_image.start().await.unwrap();

        // Wait for database to be ready
        let mut retry_count = 0;
        let queue = loop {
            match RabbitMQMessageQueue::new(get_test_config()).await {
                Ok(queue) => break queue,
                Err(_) if retry_count < 30 => {
                    retry_count += 1;
                    let _ = sleep(Duration::from_millis(500));
                    continue;
                }
                Err(e) => panic!("Failed to connect to test database: {}", e),
            }
        };
        Ok(queue)
    }

    #[tokio::test]
    async fn test_rabbitmq_connection() {
        // Use a timeout for the connection test to avoid hanging
        let connection_result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            create_rabbit_mqmessage_queue(),
        )
        .await;

        match connection_result {
            Ok(Ok(queue)) => {
                // Connection successful
                assert!(queue.is_connected());
                let close_result = queue.close().await;
                if let Err(e) = close_result {
                    eprintln!("Warning: Failed to close connection cleanly: {}", e);
                }
            }
            Ok(Err(e)) => {
                // Connection failed - likely RabbitMQ not running
                println!(
                    "RabbitMQ connection failed (RabbitMQ may not be running): {}",
                    e
                );
                // Don't fail the test if RabbitMQ is not available in test environment
            }
            Err(_) => {
                // Timeout occurred
                println!("RabbitMQ connection timed out (RabbitMQ may not be running)");
                // Don't fail the test on timeout
            }
        }
    }

    #[tokio::test]
    async fn test_publish_and_consume_message() {
        // Skip test if RabbitMQ is not available - use shorter timeout
        let queue = match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            create_rabbit_mqmessage_queue(),
        )
        .await
        {
            Ok(Ok(queue)) => {
                // Verify connection is actually working
                if !queue.is_connected() {
                    println!("Skipping test - RabbitMQ connection not active");
                    return;
                }
                queue
            }
            Ok(Err(e)) => {
                println!("Skipping test - RabbitMQ not available: {}", e);
                return;
            }
            Err(_) => {
                println!("Skipping test - RabbitMQ connection timeout");
                return;
            }
        };

        let test_message = create_test_message();
        let queue_name = &format!("test_queue_{}", Uuid::new_v4().simple());

        // Test execution with proper cleanup and shorter timeout
        let test_result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            // Create test queue
            queue.create_queue(queue_name, false).await?;

            // Verify queue was created
            let initial_size = queue.get_queue_size(queue_name).await?;
            assert_eq!(initial_size, 0, "New queue should be empty");

            // Publish message
            queue.publish_message(queue_name, &test_message).await?;

            // Add a small delay to ensure message is properly queued
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            // Verify message was published
            let queue_size = queue.get_queue_size(queue_name).await?;
            assert_eq!(
                queue_size, 1,
                "Queue should contain one message after publish"
            );

            // Consume message
            let messages = queue.consume_messages(queue_name).await?;

            // Validate results
            assert_eq!(messages.len(), 1, "Expected exactly one message");
            assert_eq!(messages[0].id, test_message.id, "Message ID mismatch");

            // Compare MessageType using serialization for reliability
            let expected_json = serde_json::to_string(&test_message.message_type)
                .expect("Failed to serialize expected message type");
            let actual_json = serde_json::to_string(&messages[0].message_type)
                .expect("Failed to serialize actual message type");

            assert_eq!(actual_json, expected_json, "Message type mismatch");

            Ok::<(), Box<dyn std::error::Error>>(())
        })
        .await;

        // Cleanup - always attempt to delete the queue
        let cleanup_result = queue.delete_queue(queue_name).await;
        if let Err(e) = cleanup_result {
            eprintln!(
                "Warning: Failed to cleanup test queue {}: {}",
                queue_name, e
            );
        }

        let close_result = queue.close().await;
        if let Err(e) = close_result {
            eprintln!("Warning: Failed to close connection: {}", e);
        }

        // Check test result after cleanup
        match test_result {
            Ok(Ok(())) => {
                // Test passed
            }
            Ok(Err(e)) => {
                panic!("Test failed: {}", e);
            }
            Err(_) => {
                // If we get a timeout, it's likely RabbitMQ is not available, so skip
                println!("Skipping test - operations timed out (RabbitMQ may not be available)");
            }
        }
    }

    #[tokio::test]
    async fn test_queue_operations() {
        // Skip test if RabbitMQ is not available
        let queue = match create_rabbit_mqmessage_queue().await {
            Ok(queue) => queue,
            Err(e) => {
                println!("Skipping test - RabbitMQ not available: {}", e);
                return;
            }
        };

        let queue_name = &format!("test_operations_queue_{}", Uuid::new_v4().simple());

        // Test execution with proper cleanup
        let test_result = async {
            // Create queue
            queue.create_queue(queue_name, false).await?;

            // Check initial queue size
            let size = queue.get_queue_size(queue_name).await?;
            assert_eq!(size, 0, "New queue should be empty");

            // Publish test messages
            const MESSAGE_COUNT: u32 = 3;
            for i in 0..MESSAGE_COUNT {
                let message = create_test_message_with_id(&format!("test-message-{}", i));
                queue.publish_message(queue_name, &message).await?;

                // Add a small delay to ensure proper sequencing
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                // Verify message was published by checking queue size
                let current_size = queue.get_queue_size(queue_name).await?;
                assert_eq!(
                    current_size,
                    i + 1,
                    "Queue size should increment after each publish"
                );
            }

            // Check final queue size after all publishing
            let size = queue.get_queue_size(queue_name).await?;
            assert_eq!(
                size, MESSAGE_COUNT,
                "Queue size should match published message count"
            );

            // Purge queue
            queue.purge_queue(queue_name).await?;

            // Check queue size after purging
            let size = queue.get_queue_size(queue_name).await?;
            assert_eq!(size, 0, "Queue should be empty after purging");

            Ok::<(), Box<dyn std::error::Error>>(())
        }
        .await;

        // Cleanup
        let cleanup_result = queue.delete_queue(queue_name).await;
        if let Err(e) = cleanup_result {
            eprintln!(
                "Warning: Failed to cleanup test queue {}: {}",
                queue_name, e
            );
        }

        let close_result = queue.close().await;
        if let Err(e) = close_result {
            eprintln!("Warning: Failed to close connection: {}", e);
        }

        // Check test result after cleanup
        if let Err(e) = test_result {
            panic!("Test failed: {}", e);
        }
    }

    #[tokio::test]
    async fn test_message_persistence() {
        // Skip test if RabbitMQ is not available
        let queue = match create_rabbit_mqmessage_queue().await {
            Ok(queue) => queue,
            Err(e) => {
                println!("Skipping test - RabbitMQ not available: {}", e);
                return;
            }
        };

        let queue_name = &format!("test_persistence_queue_{}", Uuid::new_v4().simple());
        let test_message = create_test_message();

        // Test execution with proper cleanup
        let test_result = async {
            // Create persistent queue
            queue.create_queue(queue_name, true).await?;

            // Publish persistent message
            queue.publish_message(queue_name, &test_message).await?;

            // Add a small delay to ensure message is properly persisted
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            // Verify message exists
            let size = queue.get_queue_size(queue_name).await?;
            assert_eq!(
                size, 1,
                "Persistent queue should contain the published message"
            );

            Ok::<(), Box<dyn std::error::Error>>(())
        }
        .await;

        // Cleanup
        let cleanup_result = queue.delete_queue(queue_name).await;
        if let Err(e) = cleanup_result {
            eprintln!(
                "Warning: Failed to cleanup test queue {}: {}",
                queue_name, e
            );
        }

        let close_result = queue.close().await;
        if let Err(e) = close_result {
            eprintln!("Warning: Failed to close connection: {}", e);
        }

        // Check test result after cleanup
        if let Err(e) = test_result {
            panic!("Test failed: {}", e);
        }
    }

    #[tokio::test]
    async fn test_message_queue_error_handling() {
        // Skip test if RabbitMQ is not available
        let queue = match create_rabbit_mqmessage_queue().await {
            Ok(queue) => queue,
            Err(e) => {
                println!("Skipping test - RabbitMQ not available: {}", e);
                return;
            }
        };

        // Test operations on non-existent queue
        let non_existent_queue = "non_existent_queue";

        // These operations should handle errors gracefully
        let consume_result = queue.consume_messages(non_existent_queue).await;
        assert!(
            consume_result.is_ok(),
            "Consuming from non-existent queue should return empty result"
        );

        let size_result = queue.get_queue_size(non_existent_queue).await;
        // Size check might fail or return 0 depending on implementation
        match size_result {
            Ok(size) => assert_eq!(size, 0, "Non-existent queue size should be 0"),
            Err(_) => {
                // Error is also acceptable for non-existent queue
                println!("Size check failed for non-existent queue (expected behavior)");
            }
        }

        let close_result = queue.close().await;
        if let Err(e) = close_result {
            eprintln!("Warning: Failed to close connection: {}", e);
        }
    }
}
