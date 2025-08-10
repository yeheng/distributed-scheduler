use chrono::Utc;
use scheduler_core::{traits::MockMessageQueue, Message, MessageQueue, TaskResult, TaskRunStatus};
use scheduler_domain::{StatusUpdateMessage, TaskControlAction, TaskControlMessage, TaskExecutionMessage, WorkerHeartbeatMessage};
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn test_message_queue_publish_and_consume() {
    let mq = MockMessageQueue::new();
    let queue_name = "test_queue";
    let task_execution = TaskExecutionMessage {
        task_run_id: 123,
        task_id: 456,
        task_name: "test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({"command": "echo hello"}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };
    let message = Message::task_execution(task_execution);
    mq.publish_message(queue_name, &message).await.unwrap();
    let consumed_messages = mq.consume_messages(queue_name).await.unwrap();
    assert_eq!(consumed_messages.len(), 1);
    assert_eq!(consumed_messages[0].id, message.id);
}

#[tokio::test]
async fn test_message_queue_ack_nack() {
    let mq = MockMessageQueue::new();
    let message_id = "test_message_123";
    mq.ack_message(message_id).await.unwrap();
    let acked = mq.get_acked_messages();
    assert_eq!(acked.len(), 1);
    assert_eq!(acked[0], message_id);
    mq.nack_message(message_id, true).await.unwrap();
    let nacked = mq.get_nacked_messages();
    assert_eq!(nacked.len(), 1);
    assert_eq!(nacked[0], message_id);
}

#[tokio::test]
async fn test_message_queue_create_delete_queue() {
    let mq = MockMessageQueue::new();
    let queue_name = "test_queue";
    mq.create_queue(queue_name, true).await.unwrap();
    let size = mq.get_queue_size(queue_name).await.unwrap();
    assert_eq!(size, 0);
    mq.delete_queue(queue_name).await.unwrap();
    let size_after_delete = mq.get_queue_size(queue_name).await.unwrap();
    assert_eq!(size_after_delete, 0);
}

#[tokio::test]
async fn test_message_queue_size_and_purge() {
    let mq = MockMessageQueue::new();
    let queue_name = "test_queue";
    mq.create_queue(queue_name, true).await.unwrap();

    let task_execution = TaskExecutionMessage {
        task_run_id: 123,
        task_id: 456,
        task_name: "test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };
    for i in 0..5 {
        let mut message = Message::task_execution(task_execution.clone());
        message.id = format!("message_{i}");
        mq.publish_message(queue_name, &message).await.unwrap();
    }
    let size = mq.get_queue_size(queue_name).await.unwrap();
    assert_eq!(size, 5);
    mq.purge_queue(queue_name).await.unwrap();
    let size_after_purge = mq.get_queue_size(queue_name).await.unwrap();
    assert_eq!(size_after_purge, 0);
}

#[tokio::test]
async fn test_message_queue_multiple_message_types() {
    let mq = MockMessageQueue::new();
    let queue_name = "mixed_queue";
    let task_execution = TaskExecutionMessage {
        task_run_id: 123,
        task_id: 456,
        task_name: "test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let status_update = StatusUpdateMessage {
        task_run_id: 789,
        status: TaskRunStatus::Completed,
        worker_id: "worker-001".to_string(),
        result: Some(TaskResult {
            success: true,
            output: Some("Success".to_string()),
            error_message: None,
            exit_code: Some(0),
            execution_time_ms: 1000,
        }),
        error_message: None,
        timestamp: Utc::now(),
    };

    let heartbeat = WorkerHeartbeatMessage {
        worker_id: "worker-002".to_string(),
        current_task_count: 3,
        system_load: Some(0.5),
        memory_usage_mb: Some(512),
        timestamp: Utc::now(),
    };

    let control = TaskControlMessage {
        task_run_id: 999,
        action: TaskControlAction::Cancel,
        requester: "admin".to_string(),
        timestamp: Utc::now(),
    };
    let messages = vec![
        Message::task_execution(task_execution),
        Message::status_update(status_update),
        Message::worker_heartbeat(heartbeat),
        Message::task_control(control),
    ];

    for message in &messages {
        mq.publish_message(queue_name, message).await.unwrap();
    }
    let consumed = mq.consume_messages(queue_name).await.unwrap();
    assert_eq!(consumed.len(), 4);
    let mut type_counts = HashMap::new();
    for message in consumed {
        let type_str = message.message_type_str();
        *type_counts.entry(type_str).or_insert(0) += 1;
    }

    assert_eq!(type_counts.get("task_execution"), Some(&1));
    assert_eq!(type_counts.get("status_update"), Some(&1));
    assert_eq!(type_counts.get("worker_heartbeat"), Some(&1));
    assert_eq!(type_counts.get("task_control"), Some(&1));
}

#[tokio::test]
async fn test_message_queue_empty_queue_operations() {
    let mq = MockMessageQueue::new();
    let queue_name = "empty_queue";
    let messages = mq.consume_messages(queue_name).await.unwrap();
    assert_eq!(messages.len(), 0);
    let size = mq.get_queue_size(queue_name).await.unwrap();
    assert_eq!(size, 0);
    mq.purge_queue(queue_name).await.unwrap();
}
