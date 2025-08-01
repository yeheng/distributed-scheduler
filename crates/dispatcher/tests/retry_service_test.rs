use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use tokio::sync::Mutex;

use scheduler_core::{
    models::{Message, Task, TaskRun, TaskRunStatus, TaskStatus, WorkerInfo, WorkerStatus},
    traits::{MessageQueue, TaskRepository, TaskRunRepository, WorkerRepository},
    SchedulerError, SchedulerResult,
};

use scheduler_dispatcher::retry_service::{RetryConfig, RetryService, TaskRetryService};

// Mock implementations for testing

#[derive(Debug, Clone)]
pub struct MockTaskRepository {
    tasks: Arc<Mutex<HashMap<i64, Task>>>,
}

impl MockTaskRepository {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_task(&self, task: Task) {
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task.id, task);
    }
}

#[async_trait]
impl TaskRepository for MockTaskRepository {
    async fn create(&self, task: &Task) -> SchedulerResult<Task> {
        let mut tasks = self.tasks.lock().await;
        let mut new_task = task.clone();
        new_task.id = (tasks.len() + 1) as i64;
        tasks.insert(new_task.id, new_task.clone());
        Ok(new_task)
    }

    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
        let tasks = self.tasks.lock().await;
        Ok(tasks.get(&id).cloned())
    }

    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>> {
        let tasks = self.tasks.lock().await;
        Ok(tasks.values().find(|t| t.name == name).cloned())
    }

    async fn update(&self, task: &Task) -> SchedulerResult<()> {
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task.id, task.clone());
        Ok(())
    }

    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        let mut tasks = self.tasks.lock().await;
        tasks.remove(&id);
        Ok(())
    }

    async fn list(
        &self,
        _filter: &scheduler_core::models::TaskFilter,
    ) -> SchedulerResult<Vec<Task>> {
        let tasks = self.tasks.lock().await;
        Ok(tasks.values().cloned().collect())
    }

    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
        let tasks = self.tasks.lock().await;
        Ok(tasks
            .values()
            .filter(|t| t.status == TaskStatus::Active)
            .cloned()
            .collect())
    }

    async fn get_schedulable_tasks(
        &self,
        _current_time: DateTime<Utc>,
    ) -> SchedulerResult<Vec<Task>> {
        Ok(vec![])
    }

    async fn check_dependencies(&self, _task_id: i64) -> SchedulerResult<bool> {
        Ok(true)
    }

    async fn get_dependencies(&self, _task_id: i64) -> SchedulerResult<Vec<Task>> {
        Ok(vec![])
    }

    async fn batch_update_status(
        &self,
        _task_ids: &[i64],
        _status: TaskStatus,
    ) -> SchedulerResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MockTaskRunRepository {
    task_runs: Arc<Mutex<HashMap<i64, TaskRun>>>,
    next_id: Arc<Mutex<i64>>,
}

impl MockTaskRunRepository {
    pub fn new() -> Self {
        Self {
            task_runs: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    pub async fn add_task_run(&self, task_run: TaskRun) {
        let mut task_runs = self.task_runs.lock().await;
        task_runs.insert(task_run.id, task_run);
    }
}

#[async_trait]
impl TaskRunRepository for MockTaskRunRepository {
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun> {
        let mut task_runs = self.task_runs.lock().await;
        let mut next_id = self.next_id.lock().await;

        let mut new_task_run = task_run.clone();
        new_task_run.id = *next_id;
        *next_id += 1;

        task_runs.insert(new_task_run.id, new_task_run.clone());
        Ok(new_task_run)
    }

    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>> {
        let task_runs = self.task_runs.lock().await;
        Ok(task_runs.get(&id).cloned())
    }

    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().await;
        task_runs.insert(task_run.id, task_run.clone());
        Ok(())
    }

    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().await;
        task_runs.remove(&id);
        Ok(())
    }

    async fn get_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().await;
        Ok(task_runs
            .values()
            .filter(|tr| tr.task_id == task_id)
            .cloned()
            .collect())
    }

    async fn get_by_worker_id(&self, worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().await;
        Ok(task_runs
            .values()
            .filter(|tr| tr.worker_id.as_ref().map_or(false, |w| w == worker_id))
            .cloned()
            .collect())
    }

    async fn get_by_status(&self, status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().await;
        Ok(task_runs
            .values()
            .filter(|tr| tr.status == status)
            .cloned()
            .collect())
    }

    async fn get_pending_runs(&self, _limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>> {
        self.get_by_status(TaskRunStatus::Pending).await
    }

    async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>> {
        self.get_by_status(TaskRunStatus::Running).await
    }

    async fn get_timeout_runs(&self, _timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>> {
        Ok(vec![])
    }

    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        worker_id: Option<&str>,
    ) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().await;
        if let Some(task_run) = task_runs.get_mut(&id) {
            task_run.status = status;
            if let Some(worker_id) = worker_id {
                task_run.worker_id = Some(worker_id.to_string());
            }
        }
        Ok(())
    }

    async fn update_result(
        &self,
        id: i64,
        result: Option<&str>,
        error_message: Option<&str>,
    ) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().await;
        if let Some(task_run) = task_runs.get_mut(&id) {
            task_run.result = result.map(|s| s.to_string());
            task_run.error_message = error_message.map(|s| s.to_string());
        }
        Ok(())
    }

    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().await;
        let mut runs: Vec<_> = task_runs
            .values()
            .filter(|tr| tr.task_id == task_id)
            .cloned()
            .collect();
        runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        runs.truncate(limit as usize);
        Ok(runs)
    }

    async fn get_execution_stats(
        &self,
        _task_id: i64,
        _days: i32,
    ) -> SchedulerResult<scheduler_core::traits::TaskExecutionStats> {
        Ok(scheduler_core::traits::TaskExecutionStats {
            task_id: _task_id,
            total_runs: 0,
            successful_runs: 0,
            failed_runs: 0,
            timeout_runs: 0,
            average_execution_time_ms: None,
            success_rate: 0.0,
            last_execution: None,
        })
    }

    async fn cleanup_old_runs(&self, _days: i32) -> SchedulerResult<u64> {
        Ok(0)
    }

    async fn batch_update_status(
        &self,
        _run_ids: &[i64],
        _status: TaskRunStatus,
    ) -> SchedulerResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MockWorkerRepository {
    workers: Arc<Mutex<HashMap<String, WorkerInfo>>>,
}

impl MockWorkerRepository {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_worker(&self, worker: WorkerInfo) {
        let mut workers = self.workers.lock().await;
        workers.insert(worker.id.clone(), worker);
    }
}

#[async_trait]
impl WorkerRepository for MockWorkerRepository {
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().await;
        workers.insert(worker.id.clone(), worker.clone());
        Ok(())
    }

    async fn unregister(&self, worker_id: &str) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().await;
        workers.remove(worker_id);
        Ok(())
    }

    async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
        let workers = self.workers.lock().await;
        Ok(workers.get(worker_id).cloned())
    }

    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().await;
        workers.insert(worker.id.clone(), worker.clone());
        Ok(())
    }

    async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        let workers = self.workers.lock().await;
        Ok(workers.values().cloned().collect())
    }

    async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        let workers = self.workers.lock().await;
        Ok(workers
            .values()
            .filter(|w| w.status == WorkerStatus::Alive)
            .cloned()
            .collect())
    }

    async fn get_workers_by_task_type(&self, task_type: &str) -> SchedulerResult<Vec<WorkerInfo>> {
        let workers = self.workers.lock().await;
        Ok(workers
            .values()
            .filter(|w| w.supported_task_types.contains(&task_type.to_string()))
            .cloned()
            .collect())
    }

    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: DateTime<Utc>,
        current_task_count: i32,
    ) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().await;
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.last_heartbeat = heartbeat_time;
            worker.current_task_count = current_task_count;
        }
        Ok(())
    }

    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().await;
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.status = status;
        }
        Ok(())
    }

    async fn get_timeout_workers(&self, _timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>> {
        Ok(vec![])
    }

    async fn cleanup_offline_workers(&self, _timeout_seconds: i64) -> SchedulerResult<u64> {
        Ok(0)
    }

    async fn get_worker_load_stats(
        &self,
    ) -> SchedulerResult<Vec<scheduler_core::traits::WorkerLoadStats>> {
        Ok(vec![])
    }

    async fn batch_update_status(
        &self,
        _worker_ids: &[String],
        _status: WorkerStatus,
    ) -> SchedulerResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MockMessageQueue {
    messages: Arc<Mutex<Vec<Message>>>,
}

impl MockMessageQueue {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn get_messages(&self) -> Vec<Message> {
        let messages = self.messages.lock().await;
        messages.clone()
    }

    pub async fn clear_messages(&self) {
        let mut messages = self.messages.lock().await;
        messages.clear();
    }
}

#[async_trait]
impl MessageQueue for MockMessageQueue {
    async fn publish_message(&self, _queue_name: &str, message: &Message) -> SchedulerResult<()> {
        let mut messages = self.messages.lock().await;
        messages.push(message.clone());
        Ok(())
    }

    async fn consume_messages(&self, _queue_name: &str) -> SchedulerResult<Vec<Message>> {
        let messages = self.messages.lock().await;
        Ok(messages.clone())
    }

    async fn ack_message(&self, _message_id: &str) -> SchedulerResult<()> {
        Ok(())
    }

    async fn nack_message(&self, _message_id: &str, _requeue: bool) -> SchedulerResult<()> {
        Ok(())
    }

    async fn create_queue(&self, _queue_name: &str, _durable: bool) -> SchedulerResult<()> {
        Ok(())
    }

    async fn delete_queue(&self, _queue_name: &str) -> SchedulerResult<()> {
        Ok(())
    }

    async fn get_queue_size(&self, _queue_name: &str) -> SchedulerResult<u32> {
        let messages = self.messages.lock().await;
        Ok(messages.len() as u32)
    }

    async fn purge_queue(&self, _queue_name: &str) -> SchedulerResult<()> {
        let mut messages = self.messages.lock().await;
        messages.clear();
        Ok(())
    }
}

// Helper functions for creating test data

fn create_test_task(id: i64, max_retries: i32) -> Task {
    Task {
        id,
        name: format!("test_task_{}", id),
        task_type: "shell".to_string(),
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo hello"}),
        timeout_seconds: 300,
        max_retries,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn create_test_task_run(id: i64, task_id: i64, retry_count: i32, status: TaskRunStatus) -> TaskRun {
    let now = Utc::now();
    TaskRun {
        id,
        task_id,
        status,
        worker_id: None,
        retry_count,
        shard_index: None,
        shard_total: None,
        scheduled_at: now,
        started_at: None,
        completed_at: None,
        result: None,
        error_message: None,
        created_at: now,
    }
}

fn create_test_worker(id: &str, status: WorkerStatus) -> WorkerInfo {
    let now = Utc::now();
    WorkerInfo {
        id: id.to_string(),
        hostname: format!("host-{}", id),
        ip_address: "127.0.0.1".parse().unwrap(),
        supported_task_types: vec!["shell".to_string()],
        max_concurrent_tasks: 5,
        current_task_count: 0,
        status,
        last_heartbeat: now,
        registered_at: now,
    }
}

// Test cases

#[tokio::test]
async fn test_handle_failed_task_with_retries_available() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data
    let task = create_test_task(1, 3); // max_retries = 3
    task_repo.add_task(task).await;

    let task_run = create_test_task_run(1, 1, 1, TaskRunStatus::Failed); // retry_count = 1
    task_run_repo.add_task_run(task_run).await;

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue,
        "test_queue".to_string(),
        None,
    );

    // Test retry handling
    let result = retry_service.handle_failed_task(1).await;
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true indicating retry was scheduled

    // Verify retry task was created
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

    // Setup test data
    let task = create_test_task(1, 2); // max_retries = 2
    task_repo.add_task(task).await;

    let task_run = create_test_task_run(1, 1, 2, TaskRunStatus::Failed); // retry_count = 2 (at max)
    task_run_repo.add_task_run(task_run).await;

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue,
        "test_queue".to_string(),
        None,
    );

    // Test retry handling
    let result = retry_service.handle_failed_task(1).await;
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false indicating no retry

    // Verify no retry task was created
    let pending_runs = task_run_repo.get_pending_runs(None).await.unwrap();
    assert_eq!(pending_runs.len(), 0);
}

#[tokio::test]
async fn test_handle_failed_task_inactive_task() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data
    let mut task = create_test_task(1, 3);
    task.status = TaskStatus::Inactive; // Task is inactive
    task_repo.add_task(task).await;

    let task_run = create_test_task_run(1, 1, 1, TaskRunStatus::Failed);
    task_run_repo.add_task_run(task_run).await;

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue,
        "test_queue".to_string(),
        None,
    );

    // Test retry handling
    let result = retry_service.handle_failed_task(1).await;
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false for inactive task

    // Verify no retry task was created
    let pending_runs = task_run_repo.get_pending_runs(None).await.unwrap();
    assert_eq!(pending_runs.len(), 0);
}

#[tokio::test]
async fn test_handle_timeout_task() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data
    let task = create_test_task(1, 3);
    task_repo.add_task(task).await;

    let task_run = create_test_task_run(1, 1, 0, TaskRunStatus::Timeout);
    task_run_repo.add_task_run(task_run).await;

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue,
        "test_queue".to_string(),
        None,
    );

    // Test timeout handling
    let result = retry_service.handle_timeout_task(1).await;
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true indicating retry was scheduled

    // Verify retry task was created
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

    // Setup test data
    let task = create_test_task(1, 3);
    task_repo.add_task(task).await;

    let worker = create_test_worker("worker-1", WorkerStatus::Alive);
    worker_repo.add_worker(worker).await;

    // Create running task on the worker
    let mut task_run = create_test_task_run(1, 1, 0, TaskRunStatus::Running);
    task_run.worker_id = Some("worker-1".to_string());
    task_run_repo.add_task_run(task_run).await;

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue.clone(),
        "test_queue".to_string(),
        None,
    );

    // Test worker failure handling
    let result = retry_service.handle_worker_failure("worker-1").await;
    assert!(result.is_ok());

    let reassigned_tasks = result.unwrap();
    assert_eq!(reassigned_tasks.len(), 1);
    assert_eq!(reassigned_tasks[0].retry_count, 1);

    // Verify original task was marked as failed
    let original_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(original_task.status, TaskRunStatus::Failed);

    // Verify message was sent to queue
    let messages = message_queue.get_messages().await;
    assert_eq!(messages.len(), 1);
}

#[tokio::test]
async fn test_scan_retry_tasks() {
    let task_repo = Arc::new(MockTaskRepository::new());
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data
    let task = create_test_task(1, 3);
    task_repo.add_task(task).await;

    // Create a retry task that's ready to execute
    let past_time = Utc::now() - Duration::minutes(5);
    let mut retry_task_run = create_test_task_run(2, 1, 1, TaskRunStatus::Pending);
    retry_task_run.scheduled_at = past_time;
    task_run_repo.add_task_run(retry_task_run).await;

    // Create a retry task that's not ready yet
    let future_time = Utc::now() + Duration::minutes(5);
    let mut future_retry_task_run = create_test_task_run(3, 1, 1, TaskRunStatus::Pending);
    future_retry_task_run.scheduled_at = future_time;
    task_run_repo.add_task_run(future_retry_task_run).await;

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo.clone(),
        message_queue.clone(),
        "test_queue".to_string(),
        None,
    );

    // Test scanning for retry tasks
    let result = retry_service.scan_retry_tasks().await;
    assert!(result.is_ok());

    let retry_tasks = result.unwrap();
    assert_eq!(retry_tasks.len(), 1); // Only the ready task should be returned

    // Verify message was sent to queue
    let messages = message_queue.get_messages().await;
    assert_eq!(messages.len(), 1);

    // Verify task status was updated to dispatched
    let dispatched_task = task_run_repo.get_by_id(2).await.unwrap().unwrap();
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

    // Test first retry (retry_count = 1)
    let retry_1_time = retry_service.calculate_next_retry_time(1);
    let diff_1 = retry_1_time - now;
    assert!(diff_1.num_seconds() >= 60); // Should be at least base interval
    assert!(diff_1.num_seconds() <= 120); // Should be around base * multiplier

    // Test second retry (retry_count = 2)
    let retry_2_time = retry_service.calculate_next_retry_time(2);
    let diff_2 = retry_2_time - now;
    assert!(diff_2.num_seconds() >= 240); // Should be around base * multiplier^2
    assert!(diff_2.num_seconds() <= 300);

    // Test that later retries don't exceed max interval
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

    // Create task run without corresponding task
    let task_run = create_test_task_run(1, 999, 1, TaskRunStatus::Failed); // task_id = 999 doesn't exist
    task_run_repo.add_task_run(task_run).await;

    let retry_service = TaskRetryService::new(
        task_repo,
        task_run_repo,
        message_queue,
        "test_queue".to_string(),
        None,
    );

    // Test should return error for non-existent task
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

    // Test should return error for non-existent task run
    let result = retry_service.handle_failed_task(999).await;
    assert!(result.is_err());

    if let Err(SchedulerError::TaskRunNotFound { id }) = result {
        assert_eq!(id, 999);
    } else {
        panic!("Expected TaskRunNotFound error");
    }
}
