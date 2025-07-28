use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use scheduler_core::{
    Message, MessageQueue, MessageType, MockMessageQueue, TaskExecutionMessage, TaskRunStatus,
    TaskStatusUpdate, WorkerServiceTrait,
};
use serde_json::json;
use tokio::time::sleep;

use crate::{executors::MockTaskExecutor, WorkerService};

/// 模拟任务执行器

#[tokio::test]
async fn test_worker_service_creation() {
    let message_queue = Arc::new(MockMessageQueue::new());
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .max_concurrent_tasks(3)
    .heartbeat_interval_seconds(10)
    .poll_interval_ms(500)
    .build();

    assert_eq!(worker_service.get_supported_task_types().len(), 0);
    assert_eq!(worker_service.get_current_task_count().await, 0);
}

#[tokio::test]
async fn test_worker_service_with_executor() {
    let message_queue = Arc::new(MockMessageQueue::new());
    let executor = Arc::new(MockTaskExecutor::new("shell".to_string(), true, 100));

    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .register_executor(executor)
    .build();

    let supported_types = worker_service.get_supported_task_types();
    assert_eq!(supported_types.len(), 1);
    assert!(supported_types.contains(&"shell".to_string()));
    assert!(worker_service.can_accept_task("shell").await);
    assert!(!worker_service.can_accept_task("http").await);
}

#[tokio::test]
async fn test_worker_service_start_stop() {
    let message_queue = Arc::new(MockMessageQueue::new());
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .build();

    // 测试启动
    assert!(worker_service.start().await.is_ok());

    // 等待一小段时间让服务启动
    sleep(Duration::from_millis(100)).await;

    // 测试停止
    assert!(worker_service.stop().await.is_ok());
}

#[tokio::test]
async fn test_worker_service_task_execution() {
    let message_queue = Arc::new(MockMessageQueue::new());
    let executor = Arc::new(MockTaskExecutor::new("shell".to_string(), true, 50));

    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue.clone() as Arc<dyn MessageQueue>,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .register_executor(executor)
    .build();

    // 创建任务执行消息
    let task_execution = TaskExecutionMessage {
        task_run_id: 123,
        task_id: 456,
        task_name: "test_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({"command": "echo hello"}),
        timeout_seconds: 30,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message = Message::task_execution(task_execution);
    let _ = message_queue.add_message(message).await;

    // 轮询并执行任务
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());

    // 等待任务执行完成
    sleep(Duration::from_millis(200)).await;

    // 检查状态更新消息
    let messages = message_queue.get_messages().await;
    assert!(!messages.is_empty());

    // 应该有状态更新消息
    let status_messages: Vec<_> = messages
        .iter()
        .filter(|m| matches!(m.message_type, MessageType::StatusUpdate(_)))
        .collect();
    assert!(!status_messages.is_empty());
}

#[tokio::test]
async fn test_worker_service_concurrent_limit() {
    let message_queue = Arc::new(MockMessageQueue::new());
    let executor = Arc::new(MockTaskExecutor::new("shell".to_string(), true, 200));

    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue.clone() as Arc<dyn MessageQueue>,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .max_concurrent_tasks(2) // 限制为2个并发任务
    .register_executor(executor)
    .build();

    // 创建3个任务执行消息
    for i in 1..=3 {
        let task_execution = TaskExecutionMessage {
            task_run_id: i,
            task_id: i + 100,
            task_name: format!("test_task_{}", i),
            task_type: "shell".to_string(),
            parameters: json!({"command": format!("echo task{}", i)}),
            timeout_seconds: 30,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        message_queue.add_message(message).await;
    }

    // 轮询并执行任务
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());

    // 检查当前运行的任务数量应该不超过限制
    let current_count = worker_service.get_current_task_count().await;
    assert!(current_count <= 2);

    // 等待任务执行完成
    sleep(Duration::from_millis(300)).await;

    // 最终所有任务都应该完成
    let final_count = worker_service.get_current_task_count().await;
    assert_eq!(final_count, 0);
}

#[tokio::test]
async fn test_worker_service_status_update() {
    let message_queue = Arc::new(MockMessageQueue::new());
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue.clone() as Arc<dyn MessageQueue>,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .build();

    let status_update = TaskStatusUpdate {
        task_run_id: 789,
        status: TaskRunStatus::Completed,
        worker_id: "test-worker".to_string(),
        result: Some("任务完成".to_string()),
        error_message: None,
        timestamp: Utc::now(),
    };

    assert!(worker_service
        .send_status_update(status_update)
        .await
        .is_ok());

    let messages = message_queue.get_messages().await;
    assert_eq!(messages.len(), 1);

    if let MessageType::StatusUpdate(status_msg) = &messages[0].message_type {
        assert_eq!(status_msg.task_run_id, 789);
        assert_eq!(status_msg.status, TaskRunStatus::Completed);
        assert_eq!(status_msg.worker_id, "test-worker");
    } else {
        panic!("Expected StatusUpdate message");
    }
}

#[tokio::test]
async fn test_worker_service_with_dispatcher_config() {
    let message_queue = Arc::new(MockMessageQueue::new());
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .dispatcher_url("http://localhost:8080".to_string())
    .hostname("test-host".to_string())
    .ip_address("192.168.1.100".to_string())
    .build();

    // 测试配置是否正确设置
    assert_eq!(
        worker_service.get_dispatcher_url(),
        &Some("http://localhost:8080".to_string())
    );
    assert_eq!(worker_service.get_hostname(), "test-host");
    assert_eq!(worker_service.get_ip_address(), "192.168.1.100");
}

#[tokio::test]
async fn test_worker_service_registration_without_dispatcher() {
    let message_queue = Arc::new(MockMessageQueue::new());
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .build();

    // 没有配置Dispatcher URL时，注册应该成功但不执行实际注册
    assert!(worker_service.register_with_dispatcher().await.is_ok());
    assert!(worker_service.send_heartbeat_to_dispatcher().await.is_ok());
    assert!(worker_service.unregister_from_dispatcher().await.is_ok());
}
