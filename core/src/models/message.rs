use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{TaskResult, TaskRunStatus};

/// 消息队列中的统一消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub message_type: MessageType,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub retry_count: i32,
    pub correlation_id: Option<String>,
}

/// 消息类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageType {
    TaskExecution(TaskExecutionMessage),
    StatusUpdate(StatusUpdateMessage),
    WorkerHeartbeat(WorkerHeartbeatMessage),
    TaskControl(TaskControlMessage),
}

/// 任务执行消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionMessage {
    pub task_run_id: i64,
    pub task_id: i64,
    pub task_name: String,
    pub task_type: String,
    pub parameters: serde_json::Value,
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub shard_index: Option<i32>,
    pub shard_total: Option<i32>,
}

/// 状态更新消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdateMessage {
    pub task_run_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: String,
    pub result: Option<TaskResult>,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Worker心跳消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHeartbeatMessage {
    pub worker_id: String,
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

/// 任务控制消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskControlMessage {
    pub task_run_id: i64,
    pub action: TaskControlAction,
    pub requester: String,
    pub timestamp: DateTime<Utc>,
}

/// 任务控制动作
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TaskControlAction {
    Pause,
    Resume,
    Cancel,
    Restart,
}

impl Message {
    /// 创建任务执行消息
    pub fn task_execution(message: TaskExecutionMessage) -> Self {
        let payload = serde_json::to_value(&message).unwrap_or(serde_json::Value::Null);
        Self {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::TaskExecution(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
        }
    }

    /// 创建状态更新消息
    pub fn status_update(message: StatusUpdateMessage) -> Self {
        let payload = serde_json::to_value(&message).unwrap_or(serde_json::Value::Null);
        Self {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::StatusUpdate(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
        }
    }

    /// 创建Worker心跳消息
    pub fn worker_heartbeat(message: WorkerHeartbeatMessage) -> Self {
        let payload = serde_json::to_value(&message).unwrap_or(serde_json::Value::Null);
        Self {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::WorkerHeartbeat(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
        }
    }

    /// 创建任务控制消息
    pub fn task_control(message: TaskControlMessage) -> Self {
        let payload = serde_json::to_value(&message).unwrap_or(serde_json::Value::Null);
        Self {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::TaskControl(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
        }
    }

    /// 增加重试次数
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// 设置关联ID
    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// 检查是否超过最大重试次数
    pub fn is_retry_exhausted(&self, max_retries: i32) -> bool {
        self.retry_count >= max_retries
    }

    /// 序列化消息为JSON字符串
    pub fn serialize(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 从JSON字符串反序列化消息
    pub fn deserialize(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 序列化消息为字节数组
    pub fn serialize_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// 从字节数组反序列化消息
    pub fn deserialize_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// 获取消息类型的字符串表示
    pub fn message_type_str(&self) -> &'static str {
        match &self.message_type {
            MessageType::TaskExecution(_) => "task_execution",
            MessageType::StatusUpdate(_) => "status_update",
            MessageType::WorkerHeartbeat(_) => "worker_heartbeat",
            MessageType::TaskControl(_) => "task_control",
        }
    }

    /// 获取消息的路由键（用于消息队列路由）
    pub fn routing_key(&self) -> String {
        match &self.message_type {
            MessageType::TaskExecution(msg) => format!("task.execution.{}", msg.task_type),
            MessageType::StatusUpdate(msg) => format!("status.update.{}", msg.worker_id),
            MessageType::WorkerHeartbeat(msg) => format!("worker.heartbeat.{}", msg.worker_id),
            MessageType::TaskControl(msg) => {
                format!("task.control.{:?}", msg.action).to_lowercase()
            }
        }
    }
}
