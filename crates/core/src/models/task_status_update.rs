use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use scheduler_domain::entities::TaskRunStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusUpdate {
    pub task_run_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: String,
    pub result: Option<String>,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}