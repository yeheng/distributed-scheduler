//! Mock implementations for all repository and service traits
//!
//! This module provides in-memory mock implementations that can be used
//! for unit testing without requiring actual database connections or
//! external services.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_domain::entities::{
    Message, Task, TaskFilter, TaskRun, TaskRunStatus, TaskStatus, WorkerInfo, WorkerStatus,
};

use scheduler_domain::repositories::{TaskExecutionStats, WorkerLoadStats};
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository, WorkerRepository};
use scheduler_domain::MessageQueue;
use scheduler_errors::SchedulerResult;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mock implementation of TaskRepository for testing
#[derive(Debug, Clone)]
pub struct MockTaskRepository {
    tasks: Arc<Mutex<HashMap<i64, Task>>>,
    next_id: Arc<Mutex<i64>>,
}

impl MockTaskRepository {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    pub fn with_tasks(tasks: Vec<Task>) -> Self {
        let mut task_map = HashMap::new();
        let mut max_id = 0;

        for task in tasks {
            if task.id > max_id {
                max_id = task.id;
            }
            task_map.insert(task.id, task);
        }

        Self {
            tasks: Arc::new(Mutex::new(task_map)),
            next_id: Arc::new(Mutex::new(max_id + 1)),
        }
    }

    pub fn clear(&self) {
        self.tasks.lock().unwrap().clear();
        *self.next_id.lock().unwrap() = 1;
    }

    pub fn count(&self) -> usize {
        self.tasks.lock().unwrap().len()
    }

    pub fn get_all_tasks(&self) -> Vec<Task> {
        self.tasks.lock().unwrap().values().cloned().collect()
    }
}

impl Default for MockTaskRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskRepository for MockTaskRepository {
    async fn create(&self, task: &Task) -> SchedulerResult<Task> {
        let mut tasks = self.tasks.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();

        let mut new_task = task.clone();
        new_task.id = *next_id;
        *next_id += 1;

        tasks.insert(new_task.id, new_task.clone());
        Ok(new_task)
    }

    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
        let tasks = self.tasks.lock().unwrap();
        Ok(tasks.get(&id).cloned())
    }

    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>> {
        let tasks = self.tasks.lock().unwrap();
        Ok(tasks.values().find(|t| t.name == name).cloned())
    }

    async fn update(&self, task: &Task) -> SchedulerResult<()> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task.id, task.clone());
        Ok(())
    }

    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.remove(&id);
        Ok(())
    }

    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
        let tasks = self.tasks.lock().unwrap();
        let mut filtered_tasks: Vec<Task> = tasks.values().cloned().collect();

        // Apply filters
        if let Some(status) = filter.status {
            filtered_tasks.retain(|t| t.status == status);
        }

        if let Some(task_type) = &filter.task_type {
            filtered_tasks.retain(|t| t.task_type == *task_type);
        }

        if let Some(name_pattern) = &filter.name_pattern {
            filtered_tasks.retain(|t| t.name.contains(name_pattern));
        }

        // Apply limit and offset
        if let Some(offset) = filter.offset {
            filtered_tasks = filtered_tasks.into_iter().skip(offset as usize).collect();
        }

        if let Some(limit) = filter.limit {
            filtered_tasks.truncate(limit as usize);
        }

        Ok(filtered_tasks)
    }

    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
        let tasks = self.tasks.lock().unwrap();
        Ok(tasks
            .values()
            .filter(|t| t.status == TaskStatus::Active)
            .cloned()
            .collect())
    }

    async fn get_schedulable_tasks(&self, _now: DateTime<Utc>) -> SchedulerResult<Vec<Task>> {
        // For testing, just return active tasks
        self.get_active_tasks().await
    }

    async fn check_dependencies(&self, _task_id: i64) -> SchedulerResult<bool> {
        // For testing, assume all dependencies are satisfied
        Ok(true)
    }

    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        let tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get(&task_id) {
            let mut dependencies = Vec::new();
            for &dep_id in &task.dependencies {
                if let Some(dep_task) = tasks.get(&dep_id) {
                    dependencies.push(dep_task.clone());
                }
            }
            Ok(dependencies)
        } else {
            Ok(vec![])
        }
    }

    async fn batch_update_status(
        &self,
        task_ids: &[i64],
        status: TaskStatus,
    ) -> SchedulerResult<()> {
        let mut tasks = self.tasks.lock().unwrap();
        for &task_id in task_ids {
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = status;
            }
        }
        Ok(())
    }
}

/// Mock implementation of TaskRunRepository for testing
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

    pub fn with_task_runs(task_runs: Vec<TaskRun>) -> Self {
        let mut task_run_map = HashMap::new();
        let mut max_id = 0;

        for task_run in task_runs {
            if task_run.id > max_id {
                max_id = task_run.id;
            }
            task_run_map.insert(task_run.id, task_run);
        }

        Self {
            task_runs: Arc::new(Mutex::new(task_run_map)),
            next_id: Arc::new(Mutex::new(max_id + 1)),
        }
    }

    pub fn clear(&self) {
        self.task_runs.lock().unwrap().clear();
        *self.next_id.lock().unwrap() = 1;
    }

    pub fn count(&self) -> usize {
        self.task_runs.lock().unwrap().len()
    }
}

impl Default for MockTaskRunRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskRunRepository for MockTaskRunRepository {
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun> {
        let mut task_runs = self.task_runs.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();

        let mut new_task_run = task_run.clone();
        new_task_run.id = *next_id;
        *next_id += 1;

        task_runs.insert(new_task_run.id, new_task_run.clone());
        Ok(new_task_run)
    }

    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>> {
        let task_runs = self.task_runs.lock().unwrap();
        Ok(task_runs.get(&id).cloned())
    }

    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().unwrap();
        task_runs.insert(task_run.id, task_run.clone());
        Ok(())
    }

    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().unwrap();
        task_runs.remove(&id);
        Ok(())
    }

    async fn get_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().unwrap();
        Ok(task_runs
            .values()
            .filter(|tr| tr.task_id == task_id)
            .cloned()
            .collect())
    }

    async fn get_by_worker_id(&self, worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().unwrap();
        Ok(task_runs
            .values()
            .filter(|tr| tr.worker_id.as_deref() == Some(worker_id))
            .cloned()
            .collect())
    }

    async fn get_by_status(&self, status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().unwrap();
        Ok(task_runs
            .values()
            .filter(|tr| tr.status == status)
            .cloned()
            .collect())
    }

    async fn get_pending_runs(&self, limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().unwrap();
        let mut pending_runs: Vec<TaskRun> = task_runs
            .values()
            .filter(|tr| tr.status == TaskRunStatus::Pending)
            .cloned()
            .collect();

        if let Some(limit) = limit {
            pending_runs.truncate(limit as usize);
        }

        Ok(pending_runs)
    }

    async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().unwrap();
        Ok(task_runs
            .values()
            .filter(|tr| tr.status == TaskRunStatus::Running)
            .cloned()
            .collect())
    }

    async fn get_timeout_runs(&self, _timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().unwrap();
        Ok(task_runs
            .values()
            .filter(|tr| tr.status == TaskRunStatus::Timeout)
            .cloned()
            .collect())
    }

    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        _reason: Option<&str>,
    ) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().unwrap();
        if let Some(task_run) = task_runs.get_mut(&id) {
            task_run.status = status;
        }
        Ok(())
    }

    async fn update_result(
        &self,
        id: i64,
        result: Option<&str>,
        error: Option<&str>,
    ) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().unwrap();
        if let Some(task_run) = task_runs.get_mut(&id) {
            task_run.result = result.map(String::from);
            task_run.error_message = error.map(String::from);
        }
        Ok(())
    }

    async fn cleanup_old_runs(&self, _keep_days: i32) -> SchedulerResult<u64> {
        // For testing, just return 0
        Ok(0)
    }

    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> SchedulerResult<Vec<TaskRun>> {
        let task_runs = self.task_runs.lock().unwrap();
        let mut recent_runs: Vec<TaskRun> = task_runs
            .values()
            .filter(|tr| tr.task_id == task_id)
            .cloned()
            .collect();

        recent_runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        recent_runs.truncate(limit as usize);

        Ok(recent_runs)
    }

    async fn get_execution_stats(
        &self,
        task_id: i64,
        _days: i32,
    ) -> SchedulerResult<TaskExecutionStats> {
        let task_runs = self.task_runs.lock().unwrap();
        let runs: Vec<&TaskRun> = task_runs
            .values()
            .filter(|tr| tr.task_id == task_id)
            .collect();

        let total_runs = runs.len() as i64;
        let successful_runs = runs
            .iter()
            .filter(|tr| tr.status == TaskRunStatus::Completed)
            .count() as i64;
        let failed_runs = runs
            .iter()
            .filter(|tr| tr.status == TaskRunStatus::Failed)
            .count() as i64;
        let timeout_runs = runs
            .iter()
            .filter(|tr| tr.status == TaskRunStatus::Timeout)
            .count() as i64;

        let success_rate = if total_runs > 0 {
            (successful_runs as f64 / total_runs as f64) * 100.0
        } else {
            0.0
        };

        let last_execution = runs
            .iter()
            .max_by_key(|tr| tr.created_at)
            .map(|tr| tr.created_at);

        Ok(TaskExecutionStats {
            task_id,
            total_runs,
            successful_runs,
            failed_runs,
            timeout_runs,
            average_execution_time_ms: None,
            success_rate,
            last_execution,
        })
    }

    async fn batch_update_status(&self, ids: &[i64], status: TaskRunStatus) -> SchedulerResult<()> {
        let mut task_runs = self.task_runs.lock().unwrap();
        for &id in ids {
            if let Some(task_run) = task_runs.get_mut(&id) {
                task_run.status = status;
            }
        }
        Ok(())
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Mock implementation of WorkerRepository for testing
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

    pub fn with_workers(workers: Vec<WorkerInfo>) -> Self {
        let mut worker_map = HashMap::new();
        for worker in workers {
            worker_map.insert(worker.id.clone(), worker);
        }

        Self {
            workers: Arc::new(Mutex::new(worker_map)),
        }
    }

    pub fn clear(&self) {
        self.workers.lock().unwrap().clear();
    }

    pub fn count(&self) -> usize {
        self.workers.lock().unwrap().len()
    }
}

impl Default for MockWorkerRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkerRepository for MockWorkerRepository {
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().unwrap();
        workers.insert(worker.id.clone(), worker.clone());
        Ok(())
    }

    async fn unregister(&self, worker_id: &str) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().unwrap();
        workers.remove(worker_id);
        Ok(())
    }

    async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
        let workers = self.workers.lock().unwrap();
        Ok(workers.get(worker_id).cloned())
    }

    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().unwrap();
        workers.insert(worker.id.clone(), worker.clone());
        Ok(())
    }

    async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        let workers = self.workers.lock().unwrap();
        Ok(workers.values().cloned().collect())
    }

    async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        let workers = self.workers.lock().unwrap();
        Ok(workers
            .values()
            .filter(|w| w.status == WorkerStatus::Alive)
            .cloned()
            .collect())
    }

    async fn get_workers_by_task_type(&self, task_type: &str) -> SchedulerResult<Vec<WorkerInfo>> {
        let workers = self.workers.lock().unwrap();
        Ok(workers
            .values()
            .filter(|w| w.supported_task_types.contains(&task_type.to_string()))
            .cloned()
            .collect())
    }

    async fn update_heartbeat(
        &self,
        worker_id: &str,
        timestamp: DateTime<Utc>,
        _load: i32,
    ) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().unwrap();
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.last_heartbeat = timestamp;
        }
        Ok(())
    }

    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().unwrap();
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.status = status;
        }
        Ok(())
    }

    async fn get_timeout_workers(&self, _timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>> {
        // For testing, return empty list
        Ok(vec![])
    }

    async fn cleanup_offline_workers(&self, _offline_seconds: i64) -> SchedulerResult<u64> {
        // For testing, return 0
        Ok(0)
    }

    async fn get_worker_load_stats(&self) -> SchedulerResult<Vec<WorkerLoadStats>> {
        let workers = self.workers.lock().unwrap();
        Ok(workers
            .values()
            .map(|w| WorkerLoadStats {
                worker_id: w.id.clone(),
                current_task_count: w.current_task_count,
                max_concurrent_tasks: w.max_concurrent_tasks,
                load_percentage: (w.current_task_count as f64 / w.max_concurrent_tasks as f64)
                    * 100.0,
                total_completed_tasks: 0,
                total_failed_tasks: 0,
                average_task_duration_ms: None,
                last_heartbeat: w.last_heartbeat,
            })
            .collect())
    }

    async fn batch_update_status(
        &self,
        worker_ids: &[String],
        status: WorkerStatus,
    ) -> SchedulerResult<()> {
        let mut workers = self.workers.lock().unwrap();
        for worker_id in worker_ids {
            if let Some(worker) = workers.get_mut(worker_id) {
                worker.status = status;
            }
        }
        Ok(())
    }
}

/// Mock implementation of MessageQueue for testing
#[derive(Debug, Clone)]
pub struct MockMessageQueue {
    queues: Arc<Mutex<HashMap<String, Vec<Message>>>>,
    acked_messages: Arc<Mutex<Vec<String>>>,
    nacked_messages: Arc<Mutex<Vec<String>>>,
}

impl MockMessageQueue {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            acked_messages: Arc::new(Mutex::new(Vec::new())),
            nacked_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_acked_messages(&self) -> Vec<String> {
        self.acked_messages.lock().unwrap().clone()
    }

    pub fn get_nacked_messages(&self) -> Vec<String> {
        self.nacked_messages.lock().unwrap().clone()
    }

    pub fn get_queue_messages(&self, queue: &str) -> Vec<Message> {
        self.queues
            .lock()
            .unwrap()
            .get(queue)
            .cloned()
            .unwrap_or_default()
    }

    pub fn add_message_to_queue(&self, queue: &str, message: Message) {
        let mut queues = self.queues.lock().unwrap();
        queues.entry(queue.to_string()).or_default().push(message);
    }

    pub fn get_all_messages(&self) -> Vec<Message> {
        let queues = self.queues.lock().unwrap();
        queues.values().flatten().cloned().collect()
    }

    pub fn clear(&self) {
        self.queues.lock().unwrap().clear();
        self.acked_messages.lock().unwrap().clear();
        self.nacked_messages.lock().unwrap().clear();
    }
}

impl Default for MockMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageQueue for MockMessageQueue {
    async fn publish_message(&self, queue: &str, message: &Message) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().unwrap();
        queues
            .entry(queue.to_string())
            .or_default()
            .push(message.clone());
        Ok(())
    }

    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>> {
        let mut queues = self.queues.lock().unwrap();
        let messages = queues.remove(queue).unwrap_or_default();
        Ok(messages)
    }

    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()> {
        self.acked_messages
            .lock()
            .unwrap()
            .push(message_id.to_string());
        Ok(())
    }

    async fn nack_message(&self, message_id: &str, _requeue: bool) -> SchedulerResult<()> {
        self.nacked_messages
            .lock()
            .unwrap()
            .push(message_id.to_string());
        Ok(())
    }

    async fn create_queue(&self, queue: &str, _durable: bool) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().unwrap();
        queues.entry(queue.to_string()).or_default();
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().unwrap();
        queues.remove(queue);
        Ok(())
    }

    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32> {
        let queues = self.queues.lock().unwrap();
        let size = queues.get(queue).map(|q| q.len()).unwrap_or(0) as u32;
        Ok(size)
    }

    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().unwrap();
        if let Some(queue_messages) = queues.get_mut(queue) {
            queue_messages.clear();
        }
        Ok(())
    }
}
