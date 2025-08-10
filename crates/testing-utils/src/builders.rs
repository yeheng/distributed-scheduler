//! Test data builders for creating test entities
//!
//! This module provides builder patterns for creating test data with
//! sensible defaults and easy customization.

use chrono::{DateTime, Utc};
use scheduler_domain::entities::{
    Message, MessageType, ShardConfig, Task, TaskExecutionMessage, TaskRun, TaskRunStatus,
    TaskStatus, WorkerInfo, WorkerStatus,
};

/// Builder for creating test Task entities
pub struct TaskBuilder {
    task: Task,
}

impl TaskBuilder {
    pub fn new() -> Self {
        Self {
            task: Task {
                id: 1,
                name: "test_task".to_string(),
                task_type: "shell".to_string(),
                schedule: "0 0 * * *".to_string(),
                parameters: serde_json::json!({}),
                timeout_seconds: 300,
                max_retries: 3,
                status: TaskStatus::Active,
                dependencies: vec![],
                shard_config: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        }
    }

    pub fn with_id(mut self, id: i64) -> Self {
        self.task.id = id;
        self
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.task.name = name.to_string();
        self
    }

    pub fn with_task_type(mut self, task_type: &str) -> Self {
        self.task.task_type = task_type.to_string();
        self
    }

    pub fn with_schedule(mut self, schedule: &str) -> Self {
        self.task.schedule = schedule.to_string();
        self
    }

    pub fn with_parameters(mut self, parameters: serde_json::Value) -> Self {
        self.task.parameters = parameters;
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: i32) -> Self {
        self.task.timeout_seconds = timeout_seconds;
        self
    }

    pub fn with_max_retries(mut self, max_retries: i32) -> Self {
        self.task.max_retries = max_retries;
        self
    }

    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.task.status = status;
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<i64>) -> Self {
        self.task.dependencies = dependencies;
        self
    }

    pub fn with_shard_config(mut self, shard_config: ShardConfig) -> Self {
        self.task.shard_config = Some(shard_config);
        self
    }

    pub fn inactive(mut self) -> Self {
        self.task.status = TaskStatus::Inactive;
        self
    }

    pub fn build(self) -> Task {
        self.task
    }
}

impl Default for TaskBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating test TaskRun entities
pub struct TaskRunBuilder {
    task_run: TaskRun,
}

impl TaskRunBuilder {
    pub fn new() -> Self {
        Self {
            task_run: TaskRun {
                id: 1,
                task_id: 1,
                status: TaskRunStatus::Pending,
                worker_id: None,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
                scheduled_at: Utc::now(),
                started_at: None,
                completed_at: None,
                result: None,
                error_message: None,
                created_at: Utc::now(),
            },
        }
    }

    pub fn with_id(mut self, id: i64) -> Self {
        self.task_run.id = id;
        self
    }

    pub fn with_task_id(mut self, task_id: i64) -> Self {
        self.task_run.task_id = task_id;
        self
    }

    pub fn with_status(mut self, status: TaskRunStatus) -> Self {
        self.task_run.status = status;
        self
    }

    pub fn with_worker_id(mut self, worker_id: &str) -> Self {
        self.task_run.worker_id = Some(worker_id.to_string());
        self
    }

    pub fn with_retry_count(mut self, retry_count: i32) -> Self {
        self.task_run.retry_count = retry_count;
        self
    }

    pub fn with_shard(mut self, shard_index: i32, shard_total: i32) -> Self {
        self.task_run.shard_index = Some(shard_index);
        self.task_run.shard_total = Some(shard_total);
        self
    }

    pub fn with_scheduled_at(mut self, scheduled_at: DateTime<Utc>) -> Self {
        self.task_run.scheduled_at = scheduled_at;
        self
    }

    pub fn with_started_at(mut self, started_at: DateTime<Utc>) -> Self {
        self.task_run.started_at = Some(started_at);
        self
    }

    pub fn with_completed_at(mut self, completed_at: DateTime<Utc>) -> Self {
        self.task_run.completed_at = Some(completed_at);
        self
    }

    pub fn with_result(mut self, result: &str) -> Self {
        self.task_run.result = Some(result.to_string());
        self
    }

    pub fn with_error_message(mut self, error_message: &str) -> Self {
        self.task_run.error_message = Some(error_message.to_string());
        self
    }

    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.task_run.created_at = created_at;
        self
    }

    pub fn running(mut self) -> Self {
        self.task_run.status = TaskRunStatus::Running;
        self.task_run.started_at = Some(Utc::now());
        self
    }

    pub fn completed(mut self) -> Self {
        let now = Utc::now();
        self.task_run.status = TaskRunStatus::Completed;
        self.task_run.started_at = Some(now);
        self.task_run.completed_at = Some(now);
        self
    }

    pub fn failed(mut self) -> Self {
        let now = Utc::now();
        self.task_run.status = TaskRunStatus::Failed;
        self.task_run.started_at = Some(now);
        self.task_run.completed_at = Some(now);
        self.task_run.error_message = Some("Test error".to_string());
        self
    }

    pub fn build(self) -> TaskRun {
        self.task_run
    }
}

impl Default for TaskRunBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating test WorkerInfo entities
pub struct WorkerInfoBuilder {
    worker: WorkerInfo,
}

impl WorkerInfoBuilder {
    pub fn new() -> Self {
        Self {
            worker: WorkerInfo {
                id: "test-worker-1".to_string(),
                hostname: "test-host".to_string(),
                ip_address: "127.0.0.1".to_string(),
                supported_task_types: vec!["shell".to_string()],
                max_concurrent_tasks: 5,
                current_task_count: 0,
                status: WorkerStatus::Alive,
                last_heartbeat: Utc::now(),
                registered_at: Utc::now(),
            },
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.worker.id = id.to_string();
        self
    }

    pub fn with_hostname(mut self, hostname: &str) -> Self {
        self.worker.hostname = hostname.to_string();
        self
    }

    pub fn with_ip_address(mut self, ip_address: &str) -> Self {
        self.worker.ip_address = ip_address.to_string();
        self
    }

    pub fn with_supported_task_types(mut self, task_types: Vec<&str>) -> Self {
        self.worker.supported_task_types = task_types.into_iter().map(String::from).collect();
        self
    }

    pub fn with_max_concurrent_tasks(mut self, max_concurrent_tasks: i32) -> Self {
        self.worker.max_concurrent_tasks = max_concurrent_tasks;
        self
    }

    pub fn with_current_task_count(mut self, current_task_count: i32) -> Self {
        self.worker.current_task_count = current_task_count;
        self
    }

    pub fn with_status(mut self, status: WorkerStatus) -> Self {
        self.worker.status = status;
        self
    }

    pub fn with_last_heartbeat(mut self, last_heartbeat: DateTime<Utc>) -> Self {
        self.worker.last_heartbeat = last_heartbeat;
        self
    }

    pub fn down(mut self) -> Self {
        self.worker.status = WorkerStatus::Down;
        self
    }

    pub fn busy(mut self) -> Self {
        self.worker.current_task_count = self.worker.max_concurrent_tasks;
        self
    }

    pub fn build(self) -> WorkerInfo {
        self.worker
    }
}

impl Default for WorkerInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating test Message entities
pub struct MessageBuilder {
    message: Message,
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {
            message: Message {
                id: "test-message-1".to_string(),
                timestamp: Utc::now(),
                message_type: MessageType::TaskExecution(TaskExecutionMessage {
                    task_run_id: 1,
                    task_id: 1,
                    task_name: "test_task".to_string(),
                    task_type: "shell".to_string(),
                    parameters: serde_json::json!({}),
                    timeout_seconds: 300,
                    retry_count: 0,
                    shard_index: None,
                    shard_total: None,
                }),
                payload: serde_json::json!({}),
                retry_count: 0,
                correlation_id: None,
            },
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.message.id = id.to_string();
        self
    }

    pub fn with_message_type(mut self, message_type: MessageType) -> Self {
        self.message.message_type = message_type;
        self
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.message.payload = payload;
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: &str) -> Self {
        self.message.correlation_id = Some(correlation_id.to_string());
        self
    }

    pub fn with_retry_count(mut self, retry_count: i32) -> Self {
        self.message.retry_count = retry_count;
        self
    }

    pub fn task_execution(mut self, task_run_id: i64, task_id: i64, task_name: &str) -> Self {
        let execution_msg = TaskExecutionMessage {
            task_run_id,
            task_id,
            task_name: task_name.to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        self.message.message_type = MessageType::TaskExecution(execution_msg.clone());
        self.message.payload = serde_json::to_value(execution_msg).unwrap();
        self
    }

    pub fn build(self) -> Message {
        self.message
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}
