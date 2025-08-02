//! 领域事件
//!
//! 领域事件定义，用于系统间解耦

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 领域事件基础trait
pub trait DomainEvent: Send + Sync {
    fn event_id(&self) -> Uuid;
    fn event_type(&self) -> &str;
    fn occurred_at(&self) -> DateTime<Utc>;
    fn aggregate_id(&self) -> String;
}

/// 任务相关事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskEvent {
    TaskCreated {
        id: Uuid,
        task_id: i64,
        task_name: String,
        occurred_at: DateTime<Utc>,
    },
    TaskScheduled {
        id: Uuid,
        task_id: i64,
        task_run_id: i64,
        worker_id: String,
        occurred_at: DateTime<Utc>,
    },
    TaskCompleted {
        id: Uuid,
        task_id: i64,
        task_run_id: i64,
        success: bool,
        occurred_at: DateTime<Utc>,
    },
    TaskFailed {
        id: Uuid,
        task_id: i64,
        task_run_id: i64,
        error_message: String,
        retry_count: u32,
        occurred_at: DateTime<Utc>,
    },
}

impl DomainEvent for TaskEvent {
    fn event_id(&self) -> Uuid {
        match self {
            TaskEvent::TaskCreated { id, .. } => *id,
            TaskEvent::TaskScheduled { id, .. } => *id,
            TaskEvent::TaskCompleted { id, .. } => *id,
            TaskEvent::TaskFailed { id, .. } => *id,
        }
    }

    fn event_type(&self) -> &str {
        match self {
            TaskEvent::TaskCreated { .. } => "TaskCreated",
            TaskEvent::TaskScheduled { .. } => "TaskScheduled",
            TaskEvent::TaskCompleted { .. } => "TaskCompleted",
            TaskEvent::TaskFailed { .. } => "TaskFailed",
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            TaskEvent::TaskCreated { occurred_at, .. } => *occurred_at,
            TaskEvent::TaskScheduled { occurred_at, .. } => *occurred_at,
            TaskEvent::TaskCompleted { occurred_at, .. } => *occurred_at,
            TaskEvent::TaskFailed { occurred_at, .. } => *occurred_at,
        }
    }

    fn aggregate_id(&self) -> String {
        match self {
            TaskEvent::TaskCreated { task_id, .. } => task_id.to_string(),
            TaskEvent::TaskScheduled { task_id, .. } => task_id.to_string(),
            TaskEvent::TaskCompleted { task_id, .. } => task_id.to_string(),
            TaskEvent::TaskFailed { task_id, .. } => task_id.to_string(),
        }
    }
}

/// Worker相关事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerEvent {
    WorkerRegistered {
        id: Uuid,
        worker_id: String,
        occurred_at: DateTime<Utc>,
    },
    WorkerUnregistered {
        id: Uuid,
        worker_id: String,
        occurred_at: DateTime<Utc>,
    },
    WorkerHealthChanged {
        id: Uuid,
        worker_id: String,
        is_healthy: bool,
        occurred_at: DateTime<Utc>,
    },
}

impl DomainEvent for WorkerEvent {
    fn event_id(&self) -> Uuid {
        match self {
            WorkerEvent::WorkerRegistered { id, .. } => *id,
            WorkerEvent::WorkerUnregistered { id, .. } => *id,
            WorkerEvent::WorkerHealthChanged { id, .. } => *id,
        }
    }

    fn event_type(&self) -> &str {
        match self {
            WorkerEvent::WorkerRegistered { .. } => "WorkerRegistered",
            WorkerEvent::WorkerUnregistered { .. } => "WorkerUnregistered",
            WorkerEvent::WorkerHealthChanged { .. } => "WorkerHealthChanged",
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            WorkerEvent::WorkerRegistered { occurred_at, .. } => *occurred_at,
            WorkerEvent::WorkerUnregistered { occurred_at, .. } => *occurred_at,
            WorkerEvent::WorkerHealthChanged { occurred_at, .. } => *occurred_at,
        }
    }

    fn aggregate_id(&self) -> String {
        match self {
            WorkerEvent::WorkerRegistered { worker_id, .. } => worker_id.clone(),
            WorkerEvent::WorkerUnregistered { worker_id, .. } => worker_id.clone(),
            WorkerEvent::WorkerHealthChanged { worker_id, .. } => worker_id.clone(),
        }
    }
}
