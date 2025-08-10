use std::sync::Arc;
use chrono::{Duration, Utc};

use scheduler_domain::{TaskRunStatus, WorkerStatus};
use scheduler_domain::repositories::{TaskRunRepository, WorkerRepository};
use scheduler_testing_utils::{
    MockTaskRunRepository, MockWorkerRepository, MockMessageQueue,
    TaskRunBuilder, WorkerInfoBuilder
};

use scheduler_dispatcher::recovery_service::{
    RecoveryService, SystemRecoveryService,
};
#[tokio::test]
async fn test_recover_running_tasks_with_alive_worker() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());
    let now = Utc::now();
    
    let worker = WorkerInfoBuilder::new()
        .with_id("worker-1")
        .with_status(WorkerStatus::Alive)
        .with_last_heartbeat(now - Duration::seconds(30))
        .build();
    worker_repo.register(&worker).await.unwrap();

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_status(TaskRunStatus::Running)
        .with_worker_id("worker-1")
        .build();
    task_run_repo.create(&task_run).await.unwrap();

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 0); // Task should remain running since worker is alive
}

#[tokio::test]
async fn test_recover_running_tasks_with_timeout_worker() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());
    let now = Utc::now();
    
    let worker = WorkerInfoBuilder::new()
        .with_id("worker-1")
        .with_status(WorkerStatus::Alive)
        .with_last_heartbeat(now - Duration::seconds(120)) // 120 seconds ago
        .build();
    worker_repo.register(&worker).await.unwrap();

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_status(TaskRunStatus::Running)
        .with_worker_id("worker-1")
        .build();
    task_run_repo.create(&task_run).await.unwrap();

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 1); // Task should be marked as failed
    let updated_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskRunStatus::Failed);
    assert!(updated_task.error_message.is_some());
}

#[tokio::test]
async fn test_recover_running_tasks_with_down_worker() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());
    let now = Utc::now();
    
    let worker = WorkerInfoBuilder::new()
        .with_id("worker-1")
        .with_status(WorkerStatus::Down)
        .with_last_heartbeat(now - Duration::seconds(60))
        .build();
    worker_repo.register(&worker).await.unwrap();

    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_status(TaskRunStatus::Running)
        .with_worker_id("worker-1")
        .build();
    task_run_repo.create(&task_run).await.unwrap();

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 1); // Task should be marked as failed
    let updated_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskRunStatus::Failed);
    assert!(updated_task.error_message.is_some());
}

#[tokio::test]
async fn test_recover_running_tasks_with_missing_worker() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());
    
    let task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_status(TaskRunStatus::Running)
        .with_worker_id("non-existent-worker")
        .build();
    task_run_repo.create(&task_run).await.unwrap();

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 1); // Task should be marked as failed
    let updated_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskRunStatus::Failed);
    assert!(updated_task.error_message.is_some());
}

#[tokio::test]
async fn test_recover_dispatched_tasks() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());
    let old_time = Utc::now() - Duration::seconds(400); // 400 seconds ago
    
    let old_task_run = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_status(TaskRunStatus::Dispatched)
        .with_scheduled_at(old_time)
        .build();
    task_run_repo.create(&old_task_run).await.unwrap();
    
    let recent_task_run = TaskRunBuilder::new()
        .with_id(2)
        .with_task_id(1)
        .with_status(TaskRunStatus::Dispatched)
        .build();
    task_run_repo.create(&recent_task_run).await.unwrap();

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 1); // Only old task should be recovered
    let updated_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskRunStatus::Pending);
    let recent_task = task_run_repo.get_by_id(2).await.unwrap().unwrap();
    assert_eq!(recent_task.status, TaskRunStatus::Dispatched);
}

#[tokio::test]
async fn test_recover_worker_states() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());
    let now = Utc::now();
    
    let alive_worker = WorkerInfoBuilder::new()
        .with_id("worker-1")
        .with_status(WorkerStatus::Alive)
        .with_last_heartbeat(now - Duration::seconds(30))
        .build();
    worker_repo.register(&alive_worker).await.unwrap();
    
    let timeout_worker = WorkerInfoBuilder::new()
        .with_id("worker-2")
        .with_status(WorkerStatus::Alive)
        .with_last_heartbeat(now - Duration::seconds(120))
        .build();
    worker_repo.register(&timeout_worker).await.unwrap();
    
    let down_worker = WorkerInfoBuilder::new()
        .with_id("worker-3")
        .with_status(WorkerStatus::Down)
        .with_last_heartbeat(now - Duration::seconds(200))
        .build();
    worker_repo.register(&down_worker).await.unwrap();

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo.clone(), message_queue, None);
    let result = recovery_service.recover_worker_states().await;
    assert!(result.is_ok());

    let failed_workers = result.unwrap();
    assert_eq!(failed_workers.len(), 1); // Only worker-2 should be marked as failed
    assert_eq!(failed_workers[0], "worker-2");
    let updated_worker = worker_repo.get_by_id("worker-2").await.unwrap().unwrap();
    assert_eq!(updated_worker.status, WorkerStatus::Down);
    let alive_worker = worker_repo.get_by_id("worker-1").await.unwrap().unwrap();
    assert_eq!(alive_worker.status, WorkerStatus::Alive);
}

#[tokio::test]
async fn test_recover_system_state() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());
    let now = Utc::now();
    
    let timeout_worker = WorkerInfoBuilder::new()
        .with_id("worker-1")
        .with_status(WorkerStatus::Alive)
        .with_last_heartbeat(now - Duration::seconds(120))
        .build();
    worker_repo.register(&timeout_worker).await.unwrap();
    
    let running_task = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_status(TaskRunStatus::Running)
        .with_worker_id("worker-1")
        .build();
    task_run_repo.create(&running_task).await.unwrap();
    
    let old_time = Utc::now() - Duration::seconds(400);
    let dispatched_task = TaskRunBuilder::new()
        .with_id(2)
        .with_task_id(1)
        .with_status(TaskRunStatus::Dispatched)
        .with_scheduled_at(old_time)
        .build();
    task_run_repo.create(&dispatched_task).await.unwrap();

    let recovery_service = SystemRecoveryService::new(
        task_run_repo.clone(),
        worker_repo.clone(),
        message_queue,
        None,
    );
    let result = recovery_service.recover_system_state().await;
    assert!(result.is_ok());

    let report = result.unwrap();
    assert_eq!(report.recovered_tasks.len(), 2); // Both tasks should be recovered
    assert_eq!(report.failed_workers.len(), 1); // One worker should be marked as failed
    assert!(report.errors.is_empty());
}

#[tokio::test]
async fn test_database_reconnection_success() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo, message_queue, None);
    let result = recovery_service.reconnect_database().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_message_queue_reconnection_success() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo, message_queue, None);
    let result = recovery_service.reconnect_message_queue().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_system_health() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());
    let now = Utc::now();
    
    let worker = WorkerInfoBuilder::new()
        .with_id("worker-1")
        .with_status(WorkerStatus::Alive)
        .with_last_heartbeat(now)
        .build();
    worker_repo.register(&worker).await.unwrap();

    let pending_task = TaskRunBuilder::new()
        .with_id(1)
        .with_task_id(1)
        .with_status(TaskRunStatus::Pending)
        .build();
    task_run_repo.create(&pending_task).await.unwrap();

    let running_task = TaskRunBuilder::new()
        .with_id(2)
        .with_task_id(1)
        .with_status(TaskRunStatus::Running)
        .with_worker_id("worker-1")
        .build();
    task_run_repo.create(&running_task).await.unwrap();

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo, message_queue, None);
    let result = recovery_service.check_system_health().await;
    assert!(result.is_ok());

    let health_status = result.unwrap();
    assert!(health_status.database_healthy);
    assert!(health_status.message_queue_healthy);
    assert_eq!(health_status.active_workers, 1);
    assert_eq!(health_status.pending_tasks, 1);
    assert_eq!(health_status.running_tasks, 1);
}
