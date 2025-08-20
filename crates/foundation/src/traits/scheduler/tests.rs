#[cfg(test)]
mod tests {
    use crate::traits::scheduler::*;
    use chrono::Utc;
    use scheduler_domain::entities::{Message, Task, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
    use serde_json::json;
    use std::collections::HashMap;

    // Helper function to create test data
    fn create_test_task() -> Task {
        Task {
            id: 1,
            name: "test_task".to_string(),
            task_type: "test_type".to_string(),
            schedule: "0 0 * * *".to_string(),
            parameters: json!({"command": "echo test"}),
            timeout_seconds: 300,
            max_retries: 3,
            status: scheduler_domain::entities::TaskStatus::Active,
            dependencies: Vec::new(),
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_task_run() -> TaskRun {
        TaskRun {
            id: 1,
            task_id: 1,
            status: TaskRunStatus::Pending,
            worker_id: Some("worker-1".to_string()),
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: Utc::now(),
        }
    }

    fn create_test_worker() -> WorkerInfo {
        WorkerInfo {
            id: "worker-1".to_string(),
            hostname: "test-host".to_string(),
            ip_address: "127.0.0.1".to_string(),
            supported_task_types: vec!["test_type".to_string()],
            max_concurrent_tasks: 5,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
        }
    }

    fn create_test_message() -> Message {
        use scheduler_domain::entities::MessageType;
        
        Message {
            id: "test-message-id".to_string(),
            message_type: MessageType::TaskExecution(scheduler_domain::entities::TaskExecutionMessage {
                task_run_id: 1,
                task_id: 1,
                task_name: "test_task".to_string(),
                task_type: "test_type".to_string(),
                parameters: json!({"command": "echo test"}),
                timeout_seconds: 300,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            }),
            payload: serde_json::json!({"test": "data"}),
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: Some("test-correlation".to_string()),
            trace_headers: Some(HashMap::new()),
        }
    }

    #[test]
    fn test_scheduler_stats_creation() {
        let stats = SchedulerStats {
            total_tasks: 100,
            active_tasks: 50,
            running_task_runs: 10,
            pending_task_runs: 5,
            uptime_seconds: 3600,
            last_schedule_time: Some(Utc::now()),
        };

        assert_eq!(stats.total_tasks, 100);
        assert_eq!(stats.active_tasks, 50);
        assert_eq!(stats.running_task_runs, 10);
        assert_eq!(stats.pending_task_runs, 5);
        assert_eq!(stats.uptime_seconds, 3600);
        assert!(stats.last_schedule_time.is_some());
    }

    #[test]
    fn test_scheduler_stats_default() {
        let stats = SchedulerStats {
            total_tasks: 0,
            active_tasks: 0,
            running_task_runs: 0,
            pending_task_runs: 0,
            uptime_seconds: 0,
            last_schedule_time: None,
        };

        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.active_tasks, 0);
        assert_eq!(stats.running_task_runs, 0);
        assert_eq!(stats.pending_task_runs, 0);
        assert_eq!(stats.uptime_seconds, 0);
        assert!(stats.last_schedule_time.is_none());
    }

    #[test]
    fn test_dispatch_stats_creation() {
        let stats = DispatchStats {
            total_dispatched: 100,
            successful_dispatched: 90,
            failed_dispatched: 10,
            redispatched: 5,
            avg_dispatch_time_ms: 50.5,
        };

        assert_eq!(stats.total_dispatched, 100);
        assert_eq!(stats.successful_dispatched, 90);
        assert_eq!(stats.failed_dispatched, 10);
        assert_eq!(stats.redispatched, 5);
        assert_eq!(stats.avg_dispatch_time_ms, 50.5);
    }

    #[test]
    fn test_dispatch_stats_calculations() {
        let stats = DispatchStats {
            total_dispatched: 100,
            successful_dispatched: 80,
            failed_dispatched: 20,
            redispatched: 10,
            avg_dispatch_time_ms: 100.0,
        };

        // Verify the relationships
        assert_eq!(stats.successful_dispatched + stats.failed_dispatched, stats.total_dispatched);
        assert!(stats.redispatched <= stats.failed_dispatched);
        assert!(stats.avg_dispatch_time_ms > 0.0);
    }

    #[test]
    fn test_worker_load_stats_creation() {
        let stats = WorkerLoadStats {
            worker_id: "worker-1".to_string(),
            current_task_count: 3,
            max_concurrent_tasks: 5,
            system_load: Some(2.5),
            memory_usage_mb: Some(1024),
            last_heartbeat: Utc::now(),
        };

        assert_eq!(stats.worker_id, "worker-1");
        assert_eq!(stats.current_task_count, 3);
        assert_eq!(stats.max_concurrent_tasks, 5);
        assert_eq!(stats.system_load, Some(2.5));
        assert_eq!(stats.memory_usage_mb, Some(1024));
    }

    #[test]
    fn test_worker_load_stats_utilization() {
        let stats = WorkerLoadStats {
            worker_id: "worker-1".to_string(),
            current_task_count: 3,
            max_concurrent_tasks: 5,
            system_load: Some(2.5),
            memory_usage_mb: Some(1024),
            last_heartbeat: Utc::now(),
        };

        let utilization = stats.current_task_count as f64 / stats.max_concurrent_tasks as f64;
        assert_eq!(utilization, 0.6); // 3/5 = 60%
        assert!(utilization <= 1.0);
    }

    #[test]
    fn test_worker_heartbeat_creation() {
        let heartbeat = WorkerHeartbeat {
            current_task_count: 2,
            system_load: Some(1.5),
            memory_usage_mb: Some(512),
            timestamp: Utc::now(),
        };

        assert_eq!(heartbeat.current_task_count, 2);
        assert_eq!(heartbeat.system_load, Some(1.5));
        assert_eq!(heartbeat.memory_usage_mb, Some(512));
    }

    #[test]
    fn test_worker_heartbeat_with_optional_fields() {
        let heartbeat = WorkerHeartbeat {
            current_task_count: 0,
            system_load: None,
            memory_usage_mb: None,
            timestamp: Utc::now(),
        };

        assert_eq!(heartbeat.current_task_count, 0);
        assert!(heartbeat.system_load.is_none());
        assert!(heartbeat.memory_usage_mb.is_none());
    }

    #[test]
    fn test_task_execution_context_creation() {
        let context = TaskExecutionContext {
            task_run: create_test_task_run(),
            task_type: "test_type".to_string(),
            parameters: HashMap::new(),
            timeout_seconds: 300,
            environment: HashMap::new(),
            working_directory: Some("/tmp".to_string()),
            resource_limits: ResourceLimits::default(),
        };

        assert_eq!(context.task_type, "test_type");
        assert_eq!(context.timeout_seconds, 300);
        assert_eq!(context.working_directory, Some("/tmp".to_string()));
        assert!(context.parameters.is_empty());
        assert!(context.environment.is_empty());
    }

    #[test]
    fn test_task_execution_context_with_parameters() {
        let mut parameters = HashMap::new();
        parameters.insert("param1".to_string(), json!("value1"));
        parameters.insert("param2".to_string(), json!(42));

        let mut environment = HashMap::new();
        environment.insert("ENV_VAR".to_string(), "value".to_string());

        let context = TaskExecutionContext {
            task_run: create_test_task_run(),
            task_type: "test_type".to_string(),
            parameters,
            timeout_seconds: 600,
            environment,
            working_directory: None,
            resource_limits: ResourceLimits {
                max_memory_mb: Some(2048),
                max_cpu_percent: Some(80.0),
                max_disk_mb: None,
                max_network_kbps: None,
            },
        };

        assert_eq!(context.parameters.len(), 2);
        assert_eq!(context.environment.len(), 1);
        assert_eq!(context.timeout_seconds, 600);
        assert_eq!(context.resource_limits.max_memory_mb, Some(2048));
        assert_eq!(context.resource_limits.max_cpu_percent, Some(80.0));
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();

        assert!(limits.max_memory_mb.is_none());
        assert!(limits.max_cpu_percent.is_none());
        assert!(limits.max_disk_mb.is_none());
        assert!(limits.max_network_kbps.is_none());
    }

    #[test]
    fn test_resource_limits_with_values() {
        let limits = ResourceLimits {
            max_memory_mb: Some(4096),
            max_cpu_percent: Some(90.0),
            max_disk_mb: Some(10000),
            max_network_kbps: Some(1000),
        };

        assert_eq!(limits.max_memory_mb, Some(4096));
        assert_eq!(limits.max_cpu_percent, Some(90.0));
        assert_eq!(limits.max_disk_mb, Some(10000));
        assert_eq!(limits.max_network_kbps, Some(1000));
    }

    #[test]
    fn test_executor_status_creation() {
        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(), json!("1.0.0"));
        metadata.insert("author".to_string(), json!("test"));

        let status = ExecutorStatus {
            name: "test_executor".to_string(),
            version: "1.0.0".to_string(),
            healthy: true,
            running_tasks: 2,
            supported_task_types: vec!["type1".to_string(), "type2".to_string()],
            last_health_check: Utc::now(),
            metadata,
        };

        assert_eq!(status.name, "test_executor");
        assert_eq!(status.version, "1.0.0");
        assert!(status.healthy);
        assert_eq!(status.running_tasks, 2);
        assert_eq!(status.supported_task_types.len(), 2);
        assert_eq!(status.metadata.len(), 2);
    }

    #[test]
    fn test_executor_status_unhealthy() {
        let status = ExecutorStatus {
            name: "failed_executor".to_string(),
            version: "1.0.0".to_string(),
            healthy: false,
            running_tasks: 0,
            supported_task_types: vec![],
            last_health_check: Utc::now(),
            metadata: HashMap::new(),
        };

        assert!(!status.healthy);
        assert_eq!(status.running_tasks, 0);
        assert!(status.supported_task_types.is_empty());
        assert!(status.metadata.is_empty());
    }

    #[tokio::test]
    async fn test_mock_message_queue_default() {
        let queue = MockMessageQueue::default();
        
        // Test initial state
        assert_eq!(queue.get_queue_messages("test").await.len(), 0);
        assert_eq!(queue.get_acked_messages().await.len(), 0);
        assert_eq!(queue.get_nacked_messages().await.len(), 0);
    }

    #[tokio::test]
    async fn test_mock_message_queue_new() {
        let queue = MockMessageQueue::new();
        
        // Test initial state
        assert_eq!(queue.get_queue_messages("test").await.len(), 0);
        assert_eq!(queue.get_acked_messages().await.len(), 0);
        assert_eq!(queue.get_nacked_messages().await.len(), 0);
    }

    #[tokio::test]
    async fn test_mock_message_queue_publish_and_consume() {
        let queue = MockMessageQueue::new();
        let message = create_test_message();
        
        // Publish message
        let result = queue.publish_message("test_queue", &message).await;
        assert!(result.is_ok());
        
        // Consume message
        let consumed = queue.consume_messages("test_queue").await;
        assert!(consumed.is_ok());
        assert_eq!(consumed.as_ref().unwrap().len(), 1);
        
        // Queue should be empty after consumption
        let consumed_again = queue.consume_messages("test_queue").await;
        assert!(consumed_again.is_ok());
        assert_eq!(consumed_again.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_mock_message_queue_ack_and_nack() {
        let queue = MockMessageQueue::new();
        let message = create_test_message();
        
        // Publish message
        queue.publish_message("test_queue", &message).await.unwrap();
        
        // Consume message
        let consumed = queue.consume_messages("test_queue").await.unwrap();
        let consumed_message = &consumed[0];
        
        // Ack message
        let result = queue.ack_message(&consumed_message.id).await;
        assert!(result.is_ok());
        
        // Verify acked message
        let acked = queue.get_acked_messages().await;
        assert_eq!(acked.len(), 1);
        assert_eq!(acked[0], consumed_message.id);
        
        // Publish another message for nack test
        queue.publish_message("test_queue", &message).await.unwrap();
        let consumed = queue.consume_messages("test_queue").await.unwrap();
        let consumed_message = &consumed[0];
        
        // Nack message
        let result = queue.nack_message(&consumed_message.id, true).await;
        assert!(result.is_ok());
        
        // Verify nacked message
        let nacked = queue.get_nacked_messages().await;
        assert_eq!(nacked.len(), 1);
        assert_eq!(nacked[0], consumed_message.id);
    }

    #[tokio::test]
    async fn test_mock_message_queue_create_and_delete() {
        let queue = MockMessageQueue::new();
        
        // Create queue
        let result = queue.create_queue("new_queue", true).await;
        assert!(result.is_ok());
        
        // Queue should exist and be empty
        let size = queue.get_queue_size("new_queue").await;
        assert!(size.is_ok());
        assert_eq!(size.unwrap(), 0);
        
        // Delete queue
        let result = queue.delete_queue("new_queue").await;
        assert!(result.is_ok());
        
        // Queue should no longer exist
        let size = queue.get_queue_size("new_queue").await;
        assert!(size.is_ok());
        assert_eq!(size.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_mock_message_queue_purge() {
        let queue = MockMessageQueue::new();
        let message = create_test_message();
        
        // Publish multiple messages
        for _ in 0..5 {
            queue.publish_message("purge_test", &message).await.unwrap();
        }
        
        // Verify queue size
        let size = queue.get_queue_size("purge_test").await;
        assert_eq!(size.unwrap(), 5);
        
        // Purge queue
        let result = queue.purge_queue("purge_test").await;
        assert!(result.is_ok());
        
        // Verify queue is empty
        let size = queue.get_queue_size("purge_test").await;
        assert_eq!(size.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_mock_message_queue_multiple_queues() {
        let queue = MockMessageQueue::new();
        let message = create_test_message();
        
        // Publish to different queues
        queue.publish_message("queue1", &message).await.unwrap();
        queue.publish_message("queue2", &message).await.unwrap();
        queue.publish_message("queue1", &message).await.unwrap();
        
        // Verify queue sizes
        assert_eq!(queue.get_queue_size("queue1").await.unwrap(), 2);
        assert_eq!(queue.get_queue_size("queue2").await.unwrap(), 1);
        assert_eq!(queue.get_queue_size("queue3").await.unwrap(), 0);
        
        // Consume from specific queues
        let consumed1 = queue.consume_messages("queue1").await.unwrap();
        let consumed2 = queue.consume_messages("queue2").await.unwrap();
        
        assert_eq!(consumed1.len(), 2);
        assert_eq!(consumed2.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_message_queue_helper_methods() {
        let queue = MockMessageQueue::new();
        let message = create_test_message();
        
        // Test add_message
        queue.add_message(message.clone()).await.unwrap();
        let all_messages = queue.get_messages().await;
        assert_eq!(all_messages.len(), 1);
        
        // Test get_queue_messages
        let queue_messages = queue.get_queue_messages("default").await;
        assert_eq!(queue_messages.len(), 1);
        
        // Test direct message access
        queue.publish_message("direct_test", &message).await.unwrap();
        let direct_messages = queue.get_queue_messages("direct_test").await;
        assert_eq!(direct_messages.len(), 1);
    }

    #[test]
    fn test_trait_objects_are_send_sync() {
        // Test that trait objects are Send and Sync
        fn assert_send_sync<T: Send + Sync>() {}
        
        // This should compile if the traits are properly defined
        assert_send_sync::<Box<dyn TaskControlService>>();
        assert_send_sync::<Box<dyn TaskSchedulerService>>();
        assert_send_sync::<Box<dyn TaskDispatchService>>();
        assert_send_sync::<Box<dyn WorkerManagementService>>();
        assert_send_sync::<Box<dyn WorkerServiceTrait>>();
        assert_send_sync::<Box<dyn TaskDispatchStrategy>>();
        assert_send_sync::<Box<dyn StateListenerService>>();
        assert_send_sync::<Box<dyn MessageQueue>>();
        assert_send_sync::<Box<dyn TaskExecutor>>();
        assert_send_sync::<Box<dyn ExecutorRegistry>>();
    }

    #[test]
    fn test_data_structures_are_clone() {
        // Test that data structures can be cloned
        let stats = SchedulerStats {
            total_tasks: 100,
            active_tasks: 50,
            running_task_runs: 10,
            pending_task_runs: 5,
            uptime_seconds: 3600,
            last_schedule_time: Some(Utc::now()),
        };
        
        let cloned_stats = stats.clone();
        assert_eq!(stats.total_tasks, cloned_stats.total_tasks);
        assert_eq!(stats.active_tasks, cloned_stats.active_tasks);
        
        let dispatch_stats = DispatchStats {
            total_dispatched: 100,
            successful_dispatched: 90,
            failed_dispatched: 10,
            redispatched: 5,
            avg_dispatch_time_ms: 50.5,
        };
        
        let cloned_dispatch_stats = dispatch_stats.clone();
        assert_eq!(dispatch_stats.total_dispatched, cloned_dispatch_stats.total_dispatched);
        assert_eq!(dispatch_stats.avg_dispatch_time_ms, cloned_dispatch_stats.avg_dispatch_time_ms);
    }

    #[test]
    fn test_data_structures_debug() {
        // Test that data structures implement Debug
        let stats = SchedulerStats {
            total_tasks: 1,
            active_tasks: 1,
            running_task_runs: 1,
            pending_task_runs: 1,
            uptime_seconds: 1,
            last_schedule_time: Some(Utc::now()),
        };
        
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("SchedulerStats"));
        assert!(debug_str.contains("total_tasks"));
        
        let worker_stats = WorkerLoadStats {
            worker_id: "test".to_string(),
            current_task_count: 1,
            max_concurrent_tasks: 1,
            system_load: Some(1.0),
            memory_usage_mb: Some(1024),
            last_heartbeat: Utc::now(),
        };
        
        let debug_str = format!("{:?}", worker_stats);
        assert!(debug_str.contains("WorkerLoadStats"));
        assert!(debug_str.contains("worker_id"));
    }
}