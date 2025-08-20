use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

use crate::SchedulerResult;
use scheduler_domain::entities::{
    Message, Task, TaskResult, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus,
};

// Core service interfaces that remain in core for backward compatibility
#[async_trait]
pub trait TaskControlService: Send + Sync {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize>;
    async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool>;
    async fn get_recent_executions(
        &self,
        task_id: i64,
        limit: usize,
    ) -> SchedulerResult<Vec<TaskRun>>;
}

#[async_trait]
pub trait TaskSchedulerService: Send + Sync {
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>>;
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool>;
    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun>;
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()>;
    async fn start(&self) -> SchedulerResult<()>;
    async fn stop(&self) -> SchedulerResult<()>;
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<()>;
    async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()>;
    async fn is_running(&self) -> bool;
    async fn get_stats(&self) -> SchedulerResult<SchedulerStats>;
    async fn reload_config(&self) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskDispatchService: Send + Sync {
    async fn dispatch_task(&self, task_run: &TaskRun, worker_id: &str) -> SchedulerResult<()>;
    async fn dispatch_tasks(&self, dispatches: &[(TaskRun, String)]) -> SchedulerResult<()>;
    async fn handle_status_update(
        &self,
        task_run_id: i64,
        status: TaskRunStatus,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;
    async fn redispatch_failed_tasks(&self) -> SchedulerResult<usize>;
    async fn get_dispatch_stats(&self) -> SchedulerResult<DispatchStats>;
}

#[async_trait]
pub trait WorkerManagementService: Send + Sync {
    async fn register_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()>;
    async fn unregister_worker(&self, worker_id: &str) -> SchedulerResult<()>;
    async fn update_worker_status(
        &self,
        worker_id: &str,
        status: WorkerStatus,
    ) -> SchedulerResult<()>;
    async fn get_active_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn get_worker_details(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;
    async fn check_worker_health(&self, worker_id: &str) -> SchedulerResult<bool>;
    async fn get_worker_load_stats(&self) -> SchedulerResult<HashMap<String, WorkerLoadStats>>;
    async fn select_best_worker(&self, task_type: &str) -> SchedulerResult<Option<String>>;
    async fn process_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_data: &WorkerHeartbeat,
    ) -> SchedulerResult<()>;
}

#[async_trait]
pub trait WorkerServiceTrait: Send + Sync {
    async fn start(&self) -> SchedulerResult<()>;
    async fn stop(&self) -> SchedulerResult<()>;
    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()>;
    async fn send_status_update(
        &self,
        update: crate::models::task_status_update::TaskStatusUpdate,
    ) -> SchedulerResult<()>;
    async fn get_current_task_count(&self) -> i32;
    async fn can_accept_task(&self, task_type: &str) -> bool;
    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn get_running_tasks(&self) -> Vec<TaskRun>;
    async fn is_task_running(&self, task_run_id: i64) -> bool;
    async fn send_heartbeat(&self) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskDispatchStrategy: Send + Sync {
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>>;
    fn name(&self) -> &str;
}

#[async_trait]
pub trait StateListenerService: Send + Sync {
    async fn listen_for_updates(&self) -> SchedulerResult<()>;
    async fn process_status_update(
        &self,
        task_run_id: i64,
        status: TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;
}

#[async_trait]
pub trait MessageQueue: Send + Sync {
    async fn publish_message(&self, queue: &str, message: &Message) -> SchedulerResult<()>;
    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<Message>>;
    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()>;
    async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()>;
    async fn create_queue(&self, queue: &str, durable: bool) -> SchedulerResult<()>;
    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()>;
    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32>;
    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()>;
}

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute_task(&self, context: &TaskExecutionContext) -> SchedulerResult<TaskResult>;

    fn supports_task_type(&self, task_type: &str) -> bool;
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> &str;
    fn supported_task_types(&self) -> Vec<String>;
    async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()>;
    async fn is_running(&self, task_run_id: i64) -> SchedulerResult<bool>;
    async fn get_status(&self) -> SchedulerResult<ExecutorStatus>;
    async fn health_check(&self) -> SchedulerResult<bool>;
    async fn warm_up(&self) -> SchedulerResult<()>;
    async fn cleanup(&self) -> SchedulerResult<()>;
}

#[async_trait]
pub trait ExecutorRegistry: Send + Sync {
    async fn register(
        &mut self,
        name: String,
        executor: std::sync::Arc<dyn TaskExecutor>,
    ) -> SchedulerResult<()>;
    async fn get(&self, name: &str) -> Option<std::sync::Arc<dyn TaskExecutor>>;
    async fn list_executors(&self) -> Vec<String>;
    async fn unregister(&mut self, name: &str) -> SchedulerResult<bool>;
    async fn clear(&mut self);
    async fn contains(&self, name: &str) -> bool;
    async fn count(&self) -> usize;
    async fn get_all_status(&self) -> SchedulerResult<HashMap<String, ExecutorStatus>>;
    async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>>;
    async fn get_by_task_type(
        &self,
        task_type: &str,
    ) -> SchedulerResult<Vec<std::sync::Arc<dyn TaskExecutor>>>;
}

// Data structures that remain in core for backward compatibility
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub total_tasks: i64,
    pub active_tasks: i64,
    pub running_task_runs: i64,
    pub pending_task_runs: i64,
    pub uptime_seconds: u64,
    pub last_schedule_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct DispatchStats {
    pub total_dispatched: i64,
    pub successful_dispatched: i64,
    pub failed_dispatched: i64,
    pub redispatched: i64,
    pub avg_dispatch_time_ms: f64,
}

#[derive(Debug, Clone)]
pub struct WorkerLoadStats {
    pub worker_id: String,
    pub current_task_count: i32,
    pub max_concurrent_tasks: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WorkerHeartbeat {
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TaskExecutionContext {
    pub task_run: TaskRun,
    pub task_type: String,
    pub parameters: HashMap<String, Value>,
    pub timeout_seconds: u64,
    pub environment: HashMap<String, String>,
    pub working_directory: Option<String>,
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_percent: Option<f64>,
    pub max_disk_mb: Option<u64>,
    pub max_network_kbps: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ExecutorStatus {
    pub name: String,
    pub version: String,
    pub healthy: bool,
    pub running_tasks: i32,
    pub supported_task_types: Vec<String>,
    pub last_health_check: DateTime<Utc>,
    pub metadata: HashMap<String, Value>,
}

// Mock implementation for testing
#[derive(Debug, Clone)]
pub struct MockMessageQueue {
    queues: Arc<tokio::sync::Mutex<HashMap<String, Vec<scheduler_domain::entities::Message>>>>,
    acked_messages: Arc<tokio::sync::Mutex<Vec<String>>>,
    nacked_messages: Arc<tokio::sync::Mutex<Vec<String>>>,
}

impl Default for MockMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl MockMessageQueue {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            acked_messages: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            nacked_messages: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    pub async fn get_acked_messages(&self) -> Vec<String> {
        self.acked_messages.lock().await.clone()
    }

    pub async fn get_nacked_messages(&self) -> Vec<String> {
        self.nacked_messages.lock().await.clone()
    }

    pub async fn get_queue_messages(
        &self,
        queue: &str,
    ) -> Vec<scheduler_domain::entities::Message> {
        self.queues
            .lock()
            .await
            .get(queue)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn add_message(
        &self,
        message: scheduler_domain::entities::Message,
    ) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().await;
        queues
            .entry("default".to_string())
            .or_default()
            .push(message);
        Ok(())
    }

    pub async fn get_messages(&self) -> Vec<scheduler_domain::entities::Message> {
        let queues = self.queues.lock().await;
        queues.values().flatten().cloned().collect()
    }
}

#[async_trait]
impl MessageQueue for MockMessageQueue {
    async fn publish_message(
        &self,
        queue: &str,
        message: &scheduler_domain::entities::Message,
    ) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().await;
        queues
            .entry(queue.to_string())
            .or_default()
            .push(message.clone());
        Ok(())
    }

    async fn consume_messages(
        &self,
        queue: &str,
    ) -> SchedulerResult<Vec<scheduler_domain::entities::Message>> {
        let mut queues = self.queues.lock().await;
        let messages = queues.remove(queue).unwrap_or_default();
        Ok(messages)
    }

    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()> {
        self.acked_messages
            .lock()
            .await
            .push(message_id.to_string());
        Ok(())
    }

    async fn nack_message(&self, message_id: &str, _requeue: bool) -> SchedulerResult<()> {
        self.nacked_messages
            .lock()
            .await
            .push(message_id.to_string());
        Ok(())
    }

    async fn create_queue(&self, queue: &str, _durable: bool) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().await;
        queues.entry(queue.to_string()).or_default();
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().await;
        queues.remove(queue);
        Ok(())
    }

    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32> {
        let queues = self.queues.lock().await;
        let size = queues.get(queue).map(|q| q.len()).unwrap_or(0) as u32;
        Ok(size)
    }

    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()> {
        let mut queues = self.queues.lock().await;
        if let Some(queue_messages) = queues.get_mut(queue) {
            queue_messages.clear();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
