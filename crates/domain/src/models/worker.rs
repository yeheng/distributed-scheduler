use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkerStatus {
    Available,
    Busy,
    Offline,
    Maintenance,
}
