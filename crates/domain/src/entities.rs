//! 领域实体
//!
//! 核心业务实体定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 任务实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub cron_expression: Option<String>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_run: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub timeout_seconds: Option<i32>,
}

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Active,
    Paused,
    Disabled,
}

/// 任务执行实例
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskRun {
    pub id: i64,
    pub task_id: i64,
    pub worker_id: String,
    pub status: TaskRunStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub result: Option<serde_json::Value>,
    pub retry_count: i32,
}

/// 任务执行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskRunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Worker实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Worker {
    pub id: String,
    pub hostname: String,
    pub ip_address: String,
    pub port: Option<u16>,
    pub status: WorkerStatus,
    pub max_concurrent_tasks: i32,
    pub current_task_count: i32,
    pub supported_task_types: Vec<String>,
    pub last_heartbeat: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Worker状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkerStatus {
    Available,
    Busy,
    Offline,
    Maintenance,
}
