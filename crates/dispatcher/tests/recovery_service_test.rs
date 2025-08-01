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

use scheduler_dispatcher::recovery_service::{
    RecoveryConfig, RecoveryService, SystemRecoveryService,
};

// Reuse mock implementations from retry_service_test
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
    should_fail: Arc<Mutex<bool>>,
}

impl MockTaskRunRepository {
    pub fn new() -> Self {
        Self {
            task_runs: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn add_task_run(&self, task_run: TaskRun) {
        let mut task_runs = self.task_runs.lock().await;
        task_runs.insert(task_run.id, task_run);
    }

    pub async fn set_should_fail(&self, should_fail: bool) {
        let mut fail_flag = self.should_fail.lock().await;
        *fail_flag = should_fail;
    }

    async fn check_should_fail(&self) -> SchedulerResult<()> {
        let should_fail = *self.should_fail.lock().await;
        if should_fail {
            return Err(SchedulerError::DatabaseOperation(
                "Mock database failure".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl TaskRunRepository for MockTaskRunRepository {
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun> {
        self.check_should_fail().await?;

        let mut task_runs = self.task_runs.lock().await;
        let mut next_id = self.next_id.lock().await;

        let mut new_task_run = task_run.clone();
        new_task_run.id = *next_id;
        *next_id += 1;

        task_runs.insert(new_task_run.id, new_task_run.clone());
        Ok(new_task_run)
    }

    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>> {
        self.check_should_fail().await?;

        let task_runs = self.task_runs.lock().await;
        Ok(task_runs.get(&id).cloned())
    }

    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        self.check_should_fail().await?;

        let mut task_runs = self.task_runs.lock().await;
        task_runs.insert(task_run.id, task_run.clone());
        Ok(())
    }

    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        self.check_should_fail().await?;

        let mut task_runs = self.task_runs.lock().await;
        task_runs.remove(&id);
        Ok(())
    }

    async fn get_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>> {
        self.check_should_fail().await?;

        let task_runs = self.task_runs.lock().await;
        Ok(task_runs
            .values()
            .filter(|tr| tr.task_id == task_id)
            .cloned()
            .collect())
    }

    async fn get_by_worker_id(&self, worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
        self.check_should_fail().await?;

        let task_runs = self.task_runs.lock().await;
        Ok(task_runs
            .values()
            .filter(|tr| tr.worker_id.as_ref().map_or(false, |w| w == worker_id))
            .cloned()
            .collect())
    }

    async fn get_by_status(&self, status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>> {
        self.check_should_fail().await?;

        let task_runs = self.task_runs.lock().await;
        Ok(task_runs
            .values()
            .filter(|tr| tr.status == status)
            .cloned()
            .collect())
    }

    async fn get_pending_runs(&self, limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>> {
        self.check_should_fail().await?;

        let mut pending_runs = self.get_by_status(TaskRunStatus::Pending).await?;
        if let Some(limit) = limit {
            pending_runs.truncate(limit as usize);
        }
        Ok(pending_runs)
    }

    async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>> {
        self.check_should_fail().await?;

        self.get_by_status(TaskRunStatus::Running).await
    }

    async fn get_timeout_runs(&self, _timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>> {
        self.check_should_fail().await?;

        Ok(vec![])
    }

    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        worker_id: Option<&str>,
    ) -> SchedulerResult<()> {
        self.check_should_fail().await?;

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
        self.check_should_fail().await?;

        let mut task_runs = self.task_runs.lock().await;
        if let Some(task_run) = task_runs.get_mut(&id) {
            task_run.result = result.map(|s| s.to_string());
            task_run.error_message = error_message.map(|s| s.to_string());
        }
        Ok(())
    }

    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> SchedulerResult<Vec<TaskRun>> {
        self.check_should_fail().await?;

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
        self.check_should_fail().await?;

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
        self.check_should_fail().await?;

        Ok(0)
    }

    async fn batch_update_status(
        &self,
        _run_ids: &[i64],
        _status: TaskRunStatus,
    ) -> SchedulerResult<()> {
        self.check_should_fail().await?;

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
    should_fail: Arc<Mutex<bool>>,
}

impl MockMessageQueue {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn set_should_fail(&self, should_fail: bool) {
        let mut fail_flag = self.should_fail.lock().await;
        *fail_flag = should_fail;
    }

    async fn check_should_fail(&self) -> SchedulerResult<()> {
        let should_fail = *self.should_fail.lock().await;
        if should_fail {
            return Err(SchedulerError::MessageQueue(
                "Mock message queue failure".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl MessageQueue for MockMessageQueue {
    async fn publish_message(&self, _queue_name: &str, message: &Message) -> SchedulerResult<()> {
        self.check_should_fail().await?;

        let mut messages = self.messages.lock().await;
        messages.push(message.clone());
        Ok(())
    }

    async fn consume_messages(&self, _queue_name: &str) -> SchedulerResult<Vec<Message>> {
        self.check_should_fail().await?;

        let messages = self.messages.lock().await;
        Ok(messages.clone())
    }

    async fn ack_message(&self, _message_id: &str) -> SchedulerResult<()> {
        self.check_should_fail().await?;
        Ok(())
    }

    async fn nack_message(&self, _message_id: &str, _requeue: bool) -> SchedulerResult<()> {
        self.check_should_fail().await?;
        Ok(())
    }

    async fn create_queue(&self, _queue_name: &str, _durable: bool) -> SchedulerResult<()> {
        self.check_should_fail().await?;
        Ok(())
    }

    async fn delete_queue(&self, _queue_name: &str) -> SchedulerResult<()> {
        self.check_should_fail().await?;
        Ok(())
    }

    async fn get_queue_size(&self, _queue_name: &str) -> SchedulerResult<u32> {
        self.check_should_fail().await?;

        let messages = self.messages.lock().await;
        Ok(messages.len() as u32)
    }

    async fn purge_queue(&self, _queue_name: &str) -> SchedulerResult<()> {
        self.check_should_fail().await?;

        let mut messages = self.messages.lock().await;
        messages.clear();
        Ok(())
    }
}

// Helper functions for creating test data

fn create_test_task_run(
    id: i64,
    task_id: i64,
    status: TaskRunStatus,
    worker_id: Option<String>,
) -> TaskRun {
    let now = Utc::now();
    TaskRun {
        id,
        task_id,
        status,
        worker_id,
        retry_count: 0,
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

fn create_test_worker(id: &str, status: WorkerStatus, last_heartbeat: DateTime<Utc>) -> WorkerInfo {
    let now = Utc::now();
    WorkerInfo {
        id: id.to_string(),
        hostname: format!("host-{}", id),
        ip_address: "127.0.0.1".parse().unwrap(),
        supported_task_types: vec!["shell".to_string()],
        max_concurrent_tasks: 5,
        current_task_count: 0,
        status,
        last_heartbeat,
        registered_at: now,
    }
}

// Test cases

#[tokio::test]
async fn test_recover_running_tasks_with_alive_worker() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data
    let now = Utc::now();
    let worker = create_test_worker("worker-1", WorkerStatus::Alive, now - Duration::seconds(30));
    worker_repo.add_worker(worker).await;

    let task_run = create_test_task_run(1, 1, TaskRunStatus::Running, Some("worker-1".to_string()));
    task_run_repo.add_task_run(task_run).await;

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);

    // Test recovery
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

    // Setup test data - worker with timeout heartbeat
    let now = Utc::now();
    let worker = create_test_worker(
        "worker-1",
        WorkerStatus::Alive,
        now - Duration::seconds(120),
    ); // 120 seconds ago
    worker_repo.add_worker(worker).await;

    let task_run = create_test_task_run(1, 1, TaskRunStatus::Running, Some("worker-1".to_string()));
    task_run_repo.add_task_run(task_run).await;

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);

    // Test recovery
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 1); // Task should be marked as failed

    // Verify task was marked as failed
    let updated_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskRunStatus::Failed);
    assert!(updated_task.error_message.is_some());
}

#[tokio::test]
async fn test_recover_running_tasks_with_down_worker() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data - worker marked as down
    let now = Utc::now();
    let worker = create_test_worker("worker-1", WorkerStatus::Down, now - Duration::seconds(60));
    worker_repo.add_worker(worker).await;

    let task_run = create_test_task_run(1, 1, TaskRunStatus::Running, Some("worker-1".to_string()));
    task_run_repo.add_task_run(task_run).await;

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);

    // Test recovery
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 1); // Task should be marked as failed

    // Verify task was marked as failed
    let updated_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskRunStatus::Failed);
    assert!(updated_task.error_message.is_some());
}

#[tokio::test]
async fn test_recover_running_tasks_with_missing_worker() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data - task with non-existent worker
    let task_run = create_test_task_run(
        1,
        1,
        TaskRunStatus::Running,
        Some("non-existent-worker".to_string()),
    );
    task_run_repo.add_task_run(task_run).await;

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);

    // Test recovery
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 1); // Task should be marked as failed

    // Verify task was marked as failed
    let updated_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskRunStatus::Failed);
    assert!(updated_task.error_message.is_some());
}

#[tokio::test]
async fn test_recover_dispatched_tasks() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data - old dispatched task
    let old_time = Utc::now() - Duration::seconds(400); // 400 seconds ago
    let mut task_run = create_test_task_run(1, 1, TaskRunStatus::Dispatched, None);
    task_run.created_at = old_time;
    task_run_repo.add_task_run(task_run).await;

    // Recent dispatched task
    let recent_task_run = create_test_task_run(2, 1, TaskRunStatus::Dispatched, None);
    task_run_repo.add_task_run(recent_task_run).await;

    let recovery_service =
        SystemRecoveryService::new(task_run_repo.clone(), worker_repo, message_queue, None);

    // Test recovery
    let result = recovery_service.recover_interrupted_tasks().await;
    assert!(result.is_ok());

    let recovered_tasks = result.unwrap();
    assert_eq!(recovered_tasks.len(), 1); // Only old task should be recovered

    // Verify old task was reset to pending
    let updated_task = task_run_repo.get_by_id(1).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskRunStatus::Pending);

    // Verify recent task remains dispatched
    let recent_task = task_run_repo.get_by_id(2).await.unwrap().unwrap();
    assert_eq!(recent_task.status, TaskRunStatus::Dispatched);
}

#[tokio::test]
async fn test_recover_worker_states() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data
    let now = Utc::now();

    // Alive worker with recent heartbeat
    let alive_worker =
        create_test_worker("worker-1", WorkerStatus::Alive, now - Duration::seconds(30));
    worker_repo.add_worker(alive_worker).await;

    // Alive worker with timeout heartbeat
    let timeout_worker = create_test_worker(
        "worker-2",
        WorkerStatus::Alive,
        now - Duration::seconds(120),
    );
    worker_repo.add_worker(timeout_worker).await;

    // Already down worker
    let down_worker =
        create_test_worker("worker-3", WorkerStatus::Down, now - Duration::seconds(200));
    worker_repo.add_worker(down_worker).await;

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo.clone(), message_queue, None);

    // Test recovery
    let result = recovery_service.recover_worker_states().await;
    assert!(result.is_ok());

    let failed_workers = result.unwrap();
    assert_eq!(failed_workers.len(), 1); // Only worker-2 should be marked as failed
    assert_eq!(failed_workers[0], "worker-2");

    // Verify worker-2 was marked as down
    let updated_worker = worker_repo.get_by_id("worker-2").await.unwrap().unwrap();
    assert_eq!(updated_worker.status, WorkerStatus::Down);

    // Verify worker-1 remains alive
    let alive_worker = worker_repo.get_by_id("worker-1").await.unwrap().unwrap();
    assert_eq!(alive_worker.status, WorkerStatus::Alive);
}

#[tokio::test]
async fn test_recover_system_state() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data
    let now = Utc::now();

    // Timeout worker
    let timeout_worker = create_test_worker(
        "worker-1",
        WorkerStatus::Alive,
        now - Duration::seconds(120),
    );
    worker_repo.add_worker(timeout_worker).await;

    // Running task on timeout worker
    let running_task =
        create_test_task_run(1, 1, TaskRunStatus::Running, Some("worker-1".to_string()));
    task_run_repo.add_task_run(running_task).await;

    // Old dispatched task
    let old_time = Utc::now() - Duration::seconds(400);
    let mut dispatched_task = create_test_task_run(2, 1, TaskRunStatus::Dispatched, None);
    dispatched_task.created_at = old_time;
    task_run_repo.add_task_run(dispatched_task).await;

    let recovery_service = SystemRecoveryService::new(
        task_run_repo.clone(),
        worker_repo.clone(),
        message_queue,
        None,
    );

    // Test system recovery
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

    // Test successful reconnection
    let result = recovery_service.reconnect_database().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_database_reconnection_failure() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Set database to fail
    task_run_repo.set_should_fail(true).await;

    let config = RecoveryConfig {
        db_max_retry_attempts: 2,
        db_retry_interval_seconds: 1,
        ..Default::default()
    };

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo, message_queue, Some(config));

    // Test failed reconnection
    let result = recovery_service.reconnect_database().await;
    assert!(result.is_err());

    if let Err(SchedulerError::DatabaseOperation(msg)) = result {
        assert!(msg.contains("数据库重连失败"));
    } else {
        panic!("Expected DatabaseOperation error");
    }
}

#[tokio::test]
async fn test_message_queue_reconnection_success() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo, message_queue, None);

    // Test successful reconnection
    let result = recovery_service.reconnect_message_queue().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_message_queue_reconnection_failure() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Set message queue to fail
    message_queue.set_should_fail(true).await;

    let config = RecoveryConfig {
        mq_max_retry_attempts: 2,
        mq_retry_interval_seconds: 1,
        ..Default::default()
    };

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo, message_queue, Some(config));

    // Test failed reconnection
    let result = recovery_service.reconnect_message_queue().await;
    assert!(result.is_err());

    if let Err(SchedulerError::MessageQueue(msg)) = result {
        assert!(msg.contains("消息队列重连失败"));
    } else {
        panic!("Expected MessageQueue error");
    }
}

#[tokio::test]
async fn test_check_system_health() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Setup test data
    let now = Utc::now();
    let worker = create_test_worker("worker-1", WorkerStatus::Alive, now);
    worker_repo.add_worker(worker).await;

    let pending_task = create_test_task_run(1, 1, TaskRunStatus::Pending, None);
    task_run_repo.add_task_run(pending_task).await;

    let running_task =
        create_test_task_run(2, 1, TaskRunStatus::Running, Some("worker-1".to_string()));
    task_run_repo.add_task_run(running_task).await;

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo, message_queue, None);

    // Test health check
    let result = recovery_service.check_system_health().await;
    assert!(result.is_ok());

    let health_status = result.unwrap();
    assert!(health_status.database_healthy);
    assert!(health_status.message_queue_healthy);
    assert_eq!(health_status.active_workers, 1);
    assert_eq!(health_status.pending_tasks, 1);
    assert_eq!(health_status.running_tasks, 1);
}

#[tokio::test]
async fn test_check_system_health_with_failures() {
    let task_run_repo = Arc::new(MockTaskRunRepository::new());
    let worker_repo = Arc::new(MockWorkerRepository::new());
    let message_queue = Arc::new(MockMessageQueue::new());

    // Set components to fail
    task_run_repo.set_should_fail(true).await;
    message_queue.set_should_fail(true).await;

    let recovery_service =
        SystemRecoveryService::new(task_run_repo, worker_repo, message_queue, None);

    // Test health check with failures
    let result = recovery_service.check_system_health().await;
    assert!(result.is_ok());

    let health_status = result.unwrap();
    assert!(!health_status.database_healthy);
    assert!(!health_status.message_queue_healthy);
    assert_eq!(health_status.active_workers, 0); // Should still work since worker_repo is not failing
    assert_eq!(health_status.pending_tasks, 0);
    assert_eq!(health_status.running_tasks, 0);
}
