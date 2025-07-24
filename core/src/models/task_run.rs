use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 任务执行实例
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

/// 任务运行状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "task_run_status", rename_all = "UPPERCASE")]
pub enum TaskRunStatus {
    Pending,
    Dispatched,
    Running,
    Completed,
    Failed,
    Timeout,
}

/// 任务执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub output: Option<String>,
    pub error_message: Option<String>,
    pub exit_code: Option<i32>,
    pub execution_time_ms: u64,
}

/// 任务状态更新
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
    /// 创建新的任务运行实例
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

    /// 检查任务是否正在运行
    pub fn is_running(&self) -> bool {
        matches!(self.status, TaskRunStatus::Running | TaskRunStatus::Dispatched)
    }

    /// 检查任务是否已完成（成功或失败）
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            TaskRunStatus::Completed | TaskRunStatus::Failed | TaskRunStatus::Timeout
        )
    }

    /// 检查任务是否成功完成
    pub fn is_successful(&self) -> bool {
        matches!(self.status, TaskRunStatus::Completed)
    }

    /// 更新任务状态
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

    /// 获取执行时长（毫秒）
    pub fn execution_duration_ms(&self) -> Option<i64> {
        if let (Some(started), Some(completed)) = (self.started_at, self.completed_at) {
            Some((completed - started).num_milliseconds())
        } else {
            None
        }
    }
}