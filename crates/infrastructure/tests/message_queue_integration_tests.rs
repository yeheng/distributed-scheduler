use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use scheduler_config::models::{MessageQueueConfig, MessageQueueType, RedisConfig};
use scheduler_domain::entities::*;
use scheduler_foundation::traits::MessageQueue;
use scheduler_infrastructure::MessageQueueFactory;
use testcontainers::ImageExt;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::rabbitmq::RabbitMq;
use testcontainers_modules::redis::Redis;
use tokio::time::{sleep, Duration};

pub struct MessageQueueIntegrationTestSetup {
    #[allow(dead_code)]
    postgres_container: ContainerAsync<Postgres>,
    #[allow(dead_code)]
    rabbitmq_container: ContainerAsync<RabbitMq>,
    #[allow(dead_code)]
    redis_container: ContainerAsync<Redis>,
    pub rabbitmq_url: String,
    pub redis_url: String,
}

impl MessageQueueIntegrationTestSetup {
    pub async fn new() -> Result<Self> {
        let postgres_container = Postgres::default()
            .with_db_name("scheduler_mq_test")
            .with_user("test_user")
            .with_password("test_password")
            .with_tag("16-alpine")
            .start()
            .await?;
        let rabbitmq_container = RabbitMq::default()
            .with_tag("3.12-management-alpine")
            .start()
            .await?;

        let rabbitmq_port = rabbitmq_container.get_host_port_ipv4(5672).await?;
        let rabbitmq_url = format!("amqp://guest:guest@localhost:{}", rabbitmq_port);
        let redis_container = Redis::default().with_tag("7-alpine").start().await?;

        let redis_port = redis_container.get_host_port_ipv4(6379).await?;
        let redis_url = format!("redis://localhost:{}", redis_port);
        sleep(Duration::from_secs(5)).await;

        Ok(Self {
            postgres_container,
            rabbitmq_container,
            redis_container,
            rabbitmq_url,
            redis_url,
        })
    }
    pub async fn create_rabbitmq_queue(&self) -> Result<Arc<dyn MessageQueue>> {
        let config = MessageQueueConfig {
            r#type: MessageQueueType::Rabbitmq,
            url: self.rabbitmq_url.clone(),
            redis: None,
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 60,
            connection_timeout_seconds: 30,
        };
        let queue = MessageQueueFactory::create(&config).await?;
        Ok(queue)
    }
    pub async fn create_redis_stream_queue(&self) -> Result<Arc<dyn MessageQueue>> {
        let config = MessageQueueConfig {
            r#type: MessageQueueType::RedisStream,
            url: self.redis_url.clone(),
            redis: Some(RedisConfig::default()),
            task_queue: "tasks".to_string(),
            status_queue: "status".to_string(),
            heartbeat_queue: "heartbeat".to_string(),
            control_queue: "control".to_string(),
            max_retries: 3,
            retry_delay_seconds: 60,
            connection_timeout_seconds: 30,
        };
        let queue = MessageQueueFactory::create(&config).await?;
        Ok(queue)
    }
    pub async fn test_message_queue_functionality(
        &self,
        queue: Arc<dyn MessageQueue>,
        queue_name: &str,
    ) -> Result<()> {
        queue.create_queue(queue_name, true).await?;
        let task_execution_msg = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test-task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({"command": "echo 'test'"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution_msg);
        queue.publish_message(queue_name, &message).await?;
        let messages = queue.consume_messages(queue_name).await?;
        assert_eq!(messages.len(), 1);

        if let MessageType::TaskExecution(ref msg) = messages[0].message_type {
            assert_eq!(msg.task_name, "test-task");
            assert_eq!(msg.task_type, "shell");
        } else {
            return Err(anyhow::anyhow!("Expected TaskExecution message"));
        }
        let status_update_msg = StatusUpdateMessage {
            task_run_id: 1,
            status: TaskRunStatus::Completed,
            worker_id: "test-worker".to_string(),
            result: None,
            error_message: None,
            timestamp: Utc::now(),
        };

        let status_message = Message::status_update(status_update_msg);
        queue
            .publish_message("status_updates", &status_message)
            .await?;

        let status_messages = queue.consume_messages("status_updates").await?;
        assert_eq!(status_messages.len(), 1);

        if let MessageType::StatusUpdate(ref msg) = status_messages[0].message_type {
            assert_eq!(msg.status, TaskRunStatus::Completed);
            assert_eq!(msg.worker_id, "test-worker");
        } else {
            return Err(anyhow::anyhow!("Expected StatusUpdate message"));
        }

        Ok(())
    }
    pub async fn test_message_queue_error_handling(
        &self,
        queue: Arc<dyn MessageQueue>,
    ) -> Result<()> {
        let result = queue.consume_messages("non_existent_queue").await;
        assert!(result.is_err());
        let message = Message::task_execution(TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        });

        let _result = queue.publish_message("invalid/queue/name", &message).await;

        Ok(())
    }
    pub async fn test_message_queue_performance(
        &self,
        queue: Arc<dyn MessageQueue>,
        queue_name: &str,
        message_count: usize,
    ) -> Result<()> {
        queue.create_queue(queue_name, true).await?;

        let start_time = std::time::Instant::now();
        for i in 0..message_count {
            let task_execution_msg = TaskExecutionMessage {
                task_run_id: i as i64,
                task_id: 1,
                task_name: format!("test-task-{}", i),
                task_type: "shell".to_string(),
                parameters: serde_json::json!({"command": format!("echo 'test-{}'", i)}),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };

            let message = Message::task_execution(task_execution_msg);
            queue.publish_message(queue_name, &message).await?;
        }

        let publish_duration = start_time.elapsed();
        println!(
            "Published {} messages in {:?}",
            message_count, publish_duration
        );
        let consume_start = std::time::Instant::now();
        let mut consumed_count = 0;

        while consumed_count < message_count {
            let messages = queue.consume_messages(queue_name).await?;
            consumed_count += messages.len();

            if consumed_count >= message_count {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }

        let consume_duration = consume_start.elapsed();
        println!(
            "Consumed {} messages in {:?}",
            message_count, consume_duration
        );
        assert!(consumed_count >= message_count);
        assert!(publish_duration.as_secs() < 30); // Should complete within 30 seconds
        assert!(consume_duration.as_secs() < 30); // Should complete within 30 seconds

        Ok(())
    }
    pub async fn test_message_queue_durability(
        &self,
        queue: Arc<dyn MessageQueue>,
        queue_name: &str,
    ) -> Result<()> {
        queue.create_queue(queue_name, true).await?;
        let message = Message::task_execution(TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "durability-test".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({"command": "echo 'durability'"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        });

        queue.publish_message(queue_name, &message).await?;
        sleep(Duration::from_secs(1)).await;
        let messages = queue.consume_messages(queue_name).await?;
        assert_eq!(messages.len(), 1);

        if let MessageType::TaskExecution(ref msg) = messages[0].message_type {
            assert_eq!(msg.task_name, "durability-test");
        } else {
            return Err(anyhow::anyhow!("Expected TaskExecution message"));
        }

        Ok(())
    }
    pub async fn test_message_queue_concurrency(
        &self,
        queue: Arc<dyn MessageQueue>,
        queue_name: &str,
    ) -> Result<()> {
        queue.create_queue(queue_name, true).await?;

        let message_count = 100;
        let producer_count = 5;
        let consumer_count = 3;
        let mut producer_handles = Vec::new();
        for producer_id in 0..producer_count {
            let queue_name = queue_name.to_string();
            let queue_url = self.rabbitmq_url.clone(); // Use RabbitMQ for this test

            let handle = tokio::spawn(async move {
                let config = MessageQueueConfig {
                    r#type: MessageQueueType::Rabbitmq,
                    url: queue_url,
                    redis: None,
                    task_queue: "tasks".to_string(),
                    status_queue: "status".to_string(),
                    heartbeat_queue: "heartbeat".to_string(),
                    control_queue: "control".to_string(),
                    max_retries: 3,
                    retry_delay_seconds: 60,
                    connection_timeout_seconds: 30,
                };
                let queue = MessageQueueFactory::create(&config).await.unwrap();

                for i in 0..message_count {
                    let task_execution_msg = TaskExecutionMessage {
                        task_run_id: (producer_id * message_count + i) as i64,
                        task_id: 1,
                        task_name: format!("producer-{}-task-{}", producer_id, i),
                        task_type: "shell".to_string(),
                        parameters: serde_json::json!({"command": format!("echo 'producer-{}-{}'", producer_id, i)}),
                        timeout_seconds: 300,
                        retry_count: 0,
                        shard_index: None,
                        shard_total: None,
                    };

                    let message = Message::task_execution(task_execution_msg);
                    queue.publish_message(&queue_name, &message).await.unwrap();
                }
            });

            producer_handles.push(handle);
        }
        let mut consumer_handles = Vec::new();
        let consumed_messages = std::sync::Arc::new(tokio::sync::Mutex::new(0));
        for _consumer_id in 0..consumer_count {
            let queue_name = queue_name.to_string();
            let queue_url = self.rabbitmq_url.clone();
            let consumed_count = consumed_messages.clone();

            let handle = tokio::spawn(async move {
                let config = MessageQueueConfig {
                    r#type: MessageQueueType::Rabbitmq,
                    url: queue_url,
                    redis: None,
                    task_queue: "tasks".to_string(),
                    status_queue: "status".to_string(),
                    heartbeat_queue: "heartbeat".to_string(),
                    control_queue: "control".to_string(),
                    max_retries: 3,
                    retry_delay_seconds: 60,
                    connection_timeout_seconds: 30,
                };
                let queue = MessageQueueFactory::create(&config).await.unwrap();

                let mut local_consumed = 0;
                while local_consumed < message_count * producer_count / consumer_count {
                    let messages = queue.consume_messages(&queue_name).await.unwrap();
                    local_consumed += messages.len();

                    let mut count = consumed_count.lock().await;
                    *count += messages.len();
                    drop(count);

                    if local_consumed >= message_count * producer_count / consumer_count {
                        break;
                    }

                    sleep(Duration::from_millis(10)).await;
                }
            });

            consumer_handles.push(handle);
        }
        for handle in producer_handles {
            handle.await?;
        }
        for handle in consumer_handles {
            handle.await?;
        }
        let final_count = consumed_messages.lock().await;
        assert_eq!(*final_count, message_count * producer_count);

        Ok(())
    }
    pub async fn cleanup_queues(&self, queue: Arc<dyn MessageQueue>) -> Result<()> {
        let test_queue_name = "cleanup_test";
        queue.create_queue(test_queue_name, true).await?;

        let test_message = Message::task_execution(TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "cleanup-test".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({"command": "echo 'cleanup'"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        });

        queue
            .publish_message(test_queue_name, &test_message)
            .await?;
        let messages = queue.consume_messages(test_queue_name).await?;
        assert_eq!(messages.len(), 1);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rabbitmq_integration() -> Result<()> {
        let setup = MessageQueueIntegrationTestSetup::new().await?;
        let rabbitmq_queue = setup.create_rabbitmq_queue().await?;
        setup
            .test_message_queue_functionality(rabbitmq_queue.to_owned(), "test_queue")
            .await?;
        setup
            .test_message_queue_error_handling(rabbitmq_queue.to_owned())
            .await?;
        setup
            .test_message_queue_performance(rabbitmq_queue.to_owned(), "perf_test", 10)
            .await?;
        setup
            .test_message_queue_durability(rabbitmq_queue.to_owned(), "durability_test")
            .await?;
        setup
            .test_message_queue_concurrency(rabbitmq_queue.to_owned(), "concurrency_test")
            .await?;
        setup.cleanup_queues(rabbitmq_queue.to_owned()).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_redis_stream_integration() -> Result<()> {
        let setup = MessageQueueIntegrationTestSetup::new().await?;
        let redis_queue = setup.create_redis_stream_queue().await?;
        setup
            .test_message_queue_functionality(redis_queue.to_owned(), "redis_test_queue")
            .await?;
        setup
            .test_message_queue_error_handling(redis_queue.to_owned())
            .await?;
        setup
            .test_message_queue_performance(redis_queue.to_owned(), "redis_perf_test", 10)
            .await?;
        setup
            .test_message_queue_durability(redis_queue.to_owned(), "redis_durability_test")
            .await?;
        setup.cleanup_queues(redis_queue.to_owned()).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_message_queue_switching() -> Result<()> {
        let setup = MessageQueueIntegrationTestSetup::new().await?;
        let rabbitmq_queue = setup.create_rabbitmq_queue().await?;
        let redis_queue = setup.create_redis_stream_queue().await?;
        let rabbitmq_msg = Message::task_execution(TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "rabbitmq-test".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({"command": "echo 'rabbitmq'"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        });

        rabbitmq_queue
            .publish_message("switch_test", &rabbitmq_msg)
            .await?;
        let redis_msg = Message::task_execution(TaskExecutionMessage {
            task_run_id: 2,
            task_id: 2,
            task_name: "redis-test".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({"command": "echo 'redis'"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        });

        redis_queue
            .publish_message("switch_test", &redis_msg)
            .await?;
        let rabbitmq_messages = rabbitmq_queue.consume_messages("switch_test").await?;
        let redis_messages = redis_queue.consume_messages("switch_test").await?;

        assert_eq!(rabbitmq_messages.len(), 1);
        assert_eq!(redis_messages.len(), 1);
        if let MessageType::TaskExecution(ref msg) = rabbitmq_messages[0].message_type {
            assert_eq!(msg.task_name, "rabbitmq-test");
        }

        if let MessageType::TaskExecution(ref msg) = redis_messages[0].message_type {
            assert_eq!(msg.task_name, "redis-test");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_message_queue_routing() -> Result<()> {
        let setup = MessageQueueIntegrationTestSetup::new().await?;

        let rabbitmq_queue = setup.create_rabbitmq_queue().await?;
        let queues = vec!["shell_tasks", "http_tasks", "python_tasks"];

        for queue_name in &queues {
            rabbitmq_queue.create_queue(queue_name, true).await?;
        }
        let task_types = vec!["shell", "http", "python"];

        for (i, task_type) in task_types.iter().enumerate() {
            let task_execution_msg = TaskExecutionMessage {
                task_run_id: i as i64,
                task_id: 1,
                task_name: format!("{}-task", task_type),
                task_type: task_type.to_string(),
                parameters: serde_json::json!({"command": format!("echo '{}'", task_type)}),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };

            let message = Message::task_execution(task_execution_msg);
            rabbitmq_queue
                .publish_message(&format!("{}_tasks", task_type), &message)
                .await?;
        }
        for (i, queue_name) in queues.iter().enumerate() {
            let messages = rabbitmq_queue.consume_messages(queue_name).await?;
            assert_eq!(messages.len(), 1);

            if let MessageType::TaskExecution(ref msg) = messages[0].message_type {
                assert_eq!(msg.task_type, task_types[i]);
            }
        }

        Ok(())
    }
}
