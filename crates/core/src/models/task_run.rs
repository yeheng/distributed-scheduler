use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRun {
    pub id: i64,
    pub task_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: Option<String>,
    pub retry_count: i32,
    pub shard_index: Option<i32>,
    pub shard_total: Option<i32>,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TaskExecutionContext {
    pub task_run_id: i64,
    pub task_id: i64,
    pub task_name: String,
    pub task_type: String,
    pub parameters: serde_json::Value,
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub worker_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TaskRunStatus {
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "DISPATCHED")]
    Dispatched,
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "COMPLETED")]
    Completed,
    #[serde(rename = "FAILED")]
    Failed,
    #[serde(rename = "TIMEOUT")]
    Timeout,
}

impl sqlx::Type<sqlx::Postgres> for TaskRunStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("VARCHAR")
    }
}

impl sqlx::Type<sqlx::Sqlite> for TaskRunStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <str as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for TaskRunStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        match s {
            "PENDING" => Ok(TaskRunStatus::Pending),
            "DISPATCHED" => Ok(TaskRunStatus::Dispatched),
            "RUNNING" => Ok(TaskRunStatus::Running),
            "COMPLETED" => Ok(TaskRunStatus::Completed),
            "FAILED" => Ok(TaskRunStatus::Failed),
            "TIMEOUT" => Ok(TaskRunStatus::Timeout),
            _ => Err(format!("Invalid task run status: {s}").into()),
        }
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for TaskRunStatus {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            TaskRunStatus::Pending => "PENDING",
            TaskRunStatus::Dispatched => "DISPATCHED",
            TaskRunStatus::Running => "RUNNING",
            TaskRunStatus::Completed => "COMPLETED",
            TaskRunStatus::Failed => "FAILED",
            TaskRunStatus::Timeout => "TIMEOUT",
        };
        <&str as sqlx::Encode<sqlx::Sqlite>>::encode(s, buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for TaskRunStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "PENDING" => Ok(TaskRunStatus::Pending),
            "DISPATCHED" => Ok(TaskRunStatus::Dispatched),
            "RUNNING" => Ok(TaskRunStatus::Running),
            "COMPLETED" => Ok(TaskRunStatus::Completed),
            "FAILED" => Ok(TaskRunStatus::Failed),
            "TIMEOUT" => Ok(TaskRunStatus::Timeout),
            _ => Err(format!("Invalid task run status: {s}").into()),
        }
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for TaskRunStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            TaskRunStatus::Pending => "PENDING",
            TaskRunStatus::Dispatched => "DISPATCHED",
            TaskRunStatus::Running => "RUNNING",
            TaskRunStatus::Completed => "COMPLETED",
            TaskRunStatus::Failed => "FAILED",
            TaskRunStatus::Timeout => "TIMEOUT",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(s, buf)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub output: Option<String>,
    pub error_message: Option<String>,
    pub exit_code: Option<i32>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusUpdate {
    pub task_run_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: String,
    pub result: Option<String>,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl TaskRun {
    pub fn new(task_id: i64, scheduled_at: DateTime<Utc>) -> Self {
        let now = Utc::now();
        Self {
            id: 0, // 将由数据库生成
            task_id,
            status: TaskRunStatus::Pending,
            worker_id: None,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at,
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: now,
        }
    }
    pub fn is_running(&self) -> bool {
        matches!(
            self.status,
            TaskRunStatus::Running | TaskRunStatus::Dispatched
        )
    }
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            TaskRunStatus::Completed | TaskRunStatus::Failed | TaskRunStatus::Timeout
        )
    }
    pub fn is_successful(&self) -> bool {
        matches!(self.status, TaskRunStatus::Completed)
    }
    pub fn update_status(&mut self, status: TaskRunStatus) {
        self.status = status;
        match status {
            TaskRunStatus::Running => {
                if self.started_at.is_none() {
                    self.started_at = Some(Utc::now());
                }
            }
            TaskRunStatus::Completed | TaskRunStatus::Failed | TaskRunStatus::Timeout => {
                if self.completed_at.is_none() {
                    self.completed_at = Some(Utc::now());
                }
            }
            _ => {}
        }
    }
    pub fn execution_duration_ms(&self) -> Option<i64> {
        if let (Some(started), Some(completed)) = (self.started_at, self.completed_at) {
            Some((completed - started).num_milliseconds())
        } else {
            None
        }
    }
}
