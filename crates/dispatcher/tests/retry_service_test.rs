use chrono::{Duration, Utc};
use std::sync::Arc;

use scheduler_domain::repositories::{TaskRepository, TaskRunRepository, WorkerRepository};
use scheduler_domain::{TaskRunStatus, TaskStatus, WorkerStatus};
use scheduler_errors::*;
use scheduler_testing_utils::{
    MockMessageQueue, MockTaskRepository, MockTaskRunRepository, MockWorkerRepository, TaskBuilder,
    TaskRunBuilder, WorkerInfoBuilder,
};

use scheduler_dispatcher::retry_service::{RetryConfig, RetryService, TaskRetryService};

#[tokio::test]
async fn test_handle_failed_task_with_retries_available() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let task = TaskBuilder::new()
        .with_id(1)
        .with_name("test_task_1")
        .with_task_type("shell")
        .with_max_retries(3)
        .with_status(TaskStatus::Active)
        .build();

    task_repo.create(&task).await.unwrap();

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_retry_count(1)
        .failed()
        .build();

    task_run_repo.create(&task_run).await.unwrap();

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue,
        "test_queue".to_string(),
        None,
    );

    let result = retry_service.handle_failed_task(1).await;
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true indicating retry was scheduled

    let pending_runs = task_run_repo.get_pending_runs(None).await.unwrap();
    assert_eq!(pending_runs.len(), 1);
    assert_eq!(pending_runs[0].retry_count, 2); // Should be incremented
    assert_eq!(pending_runs[0].task_id, 1);
}

#[tokio::test]
async fn test_handle_failed_task_max_retries_exceeded() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let task = TaskBuilder::new()
        .with_id(1)
        .with_name("test_task_1")
        .with_task_type("shell")
        .with_max_retries(2)
        .with_status(TaskStatus::Active)
        .build();

    task_repo.create(&task).await.unwrap();

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_retry_count(2) // at max
        .failed()
        .build();

    task_run_repo.create(&task_run).await.unwrap();

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue,
        "test_queue".to_string(),
        None,
    );

    let result = retry_service.handle_failed_task(1).await;
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false indicating no retry

    let pending_runs = task_run_repo.get_pending_runs(None).await.unwrap();
    assert_eq!(pending_runs.len(), 0);
}

#[tokio::test]
async fn test_handle_failed_task_inactive_task() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let task = TaskBuilder::new()
        .with_id(1)
        .with_name("test_task_1")
        .with_task_type("shell")
        .with_max_retries(3)
        .inactive() // Task is inactive
        .build();

    task_repo.create(&task).await.unwrap();

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_retry_count(1)
        .failed()
        .build();

    task_run_repo.create(&task_run).await.unwrap();

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue,
        "test_queue".to_string(),
        None,
    );

    let result = retry_service.handle_failed_task(1).await;
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false for inactive task

    let pending_runs = task_run_repo.get_pending_runs(None).await.unwrap();
    assert_eq!(pending_runs.len(), 0);
}

#[tokio::test]
async fn test_handle_timeout_task() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let task = TaskBuilder::new()
        .with_id(1)
        .with_name("test_task_1")
        .with_task_type("shell")
        .with_max_retries(3)
        .with_status(TaskStatus::Active)
        .build();

    task_repo.create(&task).await.unwrap();

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_retry_count(0)
        .with_status(TaskRunStatus::Timeout)
        .build();

    task_run_repo.create(&task_run).await.unwrap();

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue,
        "test_queue".to_string(),
        None,
    );

    let result = retry_service.handle_timeout_task(1).await;
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true indicating retry was scheduled

    let pending_runs = task_run_repo.get_pending_runs(None).await.unwrap();
    assert_eq!(pending_runs.len(), 1);
    assert_eq!(pending_runs[0].retry_count, 1);
}

#[tokio::test]
async fn test_handle_worker_failure() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let task = TaskBuilder::new()
        .with_id(1)
        .with_name("test_task_1")
        .with_task_type("shell")
        .with_max_retries(3)
        .with_status(TaskStatus::Active)
        .build();

    task_repo.create(&task).await.unwrap();

    let worker = WorkerInfoBuilder::new()
        .with_id("worker-1")
        .with_status(WorkerStatus::Alive)
        .build();

    worker_repo.register(&worker).await.unwrap();

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_retry_count(0)
        .with_worker_id("worker-1")
        .running()
        .build();

    task_run_repo.create(&task_run).await.unwrap();

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue.clone(),
        "test_queue".to_string(),
        None,
    );

    let result = retry_service.handle_worker_failure("worker-1").await;
    assert!(result.is_ok());

    let reassigned_tasks = result.unwrap();
    assert_eq!(reassigned_tasks.len(), 1);
    assert_eq!(reassigned_tasks[0].retry_count, 1);

    let original_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(original_task.status, TaskRunStatus::Failed);

    let messages = message_queue.get_all_messages();
    assert_eq!(messages.len(), 1);
}

#[tokio::test]
async fn test_scan_retry_tasks() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let task = TaskBuilder::new()
        .with_id(1)
        .with_name("test_task_1")
        .with_task_type("shell")
        .with_max_retries(3)
        .with_status(TaskStatus::Active)
        .build();

    task_repo.create(&task).await.unwrap();

    let past_time = Utc::now() - Duration::minutes(5);
    let retry_task_run = TaskRunBuilder::new()
        .with_id(2)
        .with_task_id(1)
        .with_retry_count(1)
        .with_status(TaskRunStatus::Pending)
        .with_scheduled_at(past_time)
        .build();

    task_run_repo.create(&retry_task_run).await.unwrap();

    let future_time = Utc::now() + Duration::minutes(5);
    let future_retry_task_run = TaskRunBuilder::new()
        .with_id(3)
        .with_task_id(1)
        .with_retry_count(1)
        .with_status(TaskRunStatus::Pending)
        .with_scheduled_at(future_time)
        .build();

    task_run_repo.create(&future_retry_task_run).await.unwrap();

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue.clone(),
        "test_queue".to_string(),
        None,
    );

    let result = retry_service.scan_retry_tasks().await;
    assert!(result.is_ok());

    let retry_tasks = result.unwrap();
    assert_eq!(retry_tasks.len(), 1); // Only the ready task should be returned

    let messages = message_queue.get_all_messages();
    assert_eq!(messages.len(), 1);

    let dispatched_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(dispatched_task.status, TaskRunStatus::Dispatched);
}

#[tokio::test]
async fn test_calculate_next_retry_time() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let config = RetryConfig {
        base_interval_seconds: 60,
        max_interval_seconds: 3600,
        backoff_multiplier: 2.0,
        jitter_factor: 0.0, // No jitter for predictable testing
    };

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo,
        message_queue,
        "test_queue".to_string(),
        Some(config),
    );

    let now = Utc::now();
    let retry_1_time = retry_service.calculate_next_retry_time(1);
    let diff_1 = retry_1_time - now;
    assert!(diff_1.num_seconds() >= 60); // Should be at least base interval
    assert!(diff_1.num_seconds() <= 120); // Should be around base * multiplier

    let retry_2_time = retry_service.calculate_next_retry_time(2);
    let diff_2 = retry_2_time - now;
    assert!(diff_2.num_seconds() >= 240); // Should be around base * multiplier^2
    assert!(diff_2.num_seconds() <= 300);

    let retry_10_time = retry_service.calculate_next_retry_time(10);
    let diff_10 = retry_10_time - now;
    assert!(diff_10.num_seconds() <= 3600); // Should not exceed max interval
}

#[tokio::test]
async fn test_retry_config_default() {
    let config = RetryConfig::default();
    assert_eq!(config.base_interval_seconds, 60);
    assert_eq!(config.max_interval_seconds, 3600);
    assert_eq!(config.backoff_multiplier, 2.0);
    assert_eq!(config.jitter_factor, 0.1);
}

#[tokio::test]
async fn test_task_not_found_error() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(999) // task_id = 999 doesn't exist
        .with_retry_count(1)
        .failed()
        .build();

    task_run_repo.create(&task_run).await.unwrap();

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo,
        message_queue,
        "test_queue".to_string(),
        None,
    );

    let result = retry_service.handle_failed_task(1).await;
    assert!(result.is_err());

    if let Err(SchedulerError::TaskNotFound { id }) = result {
        assert_eq!(id, 999);
    } else {
        panic!("Expected TaskNotFound error");
    }
}

#[tokio::test]
async fn test_task_run_not_found_error() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo,
        message_queue,
        "test_queue".to_string(),
        None,
    );

    let result = retry_service.handle_failed_task(999).await;
    assert!(result.is_err());

    if let Err(SchedulerError::TaskRunNotFound { id }) = result {
        assert_eq!(id, 999);
    } else {
        panic!("Expected TaskRunNotFound error");
    }
}
