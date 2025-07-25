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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn test_message_creation_task_execution() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: Some(1),
            shard_total: Some(3),
        };

        let message = Message::task_execution(task_execution.clone());

        assert!(!message.id.is_empty());
        assert_eq!(message.retry_count, 0);
        assert!(message.correlation_id.is_none());
        assert_eq!(message.message_type_str(), "task_execution");

        if let MessageType::TaskExecution(msg) = &message.message_type {
            assert_eq!(msg.task_run_id, 123);
            assert_eq!(msg.task_id, 456);
            assert_eq!(msg.task_name, "test_task");
            assert_eq!(msg.task_type, "shell");
        } else {
            panic!("Expected TaskExecution message type");
        }
    }

    #[test]
    fn test_message_creation_status_update() {
        let status_update = StatusUpdateMessage {
            task_run_id: 789,
            status: TaskRunStatus::Running,
            worker_id: "worker-001".to_string(),
            result: None,
            error_message: None,
            timestamp: Utc::now(),
        };

        let message = Message::status_update(status_update.clone());

        assert!(!message.id.is_empty());
        assert_eq!(message.message_type_str(), "status_update");

        if let MessageType::StatusUpdate(msg) = &message.message_type {
            assert_eq!(msg.task_run_id, 789);
            assert_eq!(msg.worker_id, "worker-001");
            assert_eq!(msg.status, TaskRunStatus::Running);
        } else {
            panic!("Expected StatusUpdate message type");
        }
    }

    #[test]
    fn test_message_creation_worker_heartbeat() {
        let heartbeat = WorkerHeartbeatMessage {
            worker_id: "worker-002".to_string(),
            current_task_count: 5,
            system_load: Some(0.75),
            memory_usage_mb: Some(1024),
            timestamp: Utc::now(),
        };

        let message = Message::worker_heartbeat(heartbeat.clone());

        assert!(!message.id.is_empty());
        assert_eq!(message.message_type_str(), "worker_heartbeat");

        if let MessageType::WorkerHeartbeat(msg) = &message.message_type {
            assert_eq!(msg.worker_id, "worker-002");
            assert_eq!(msg.current_task_count, 5);
            assert_eq!(msg.system_load, Some(0.75));
            assert_eq!(msg.memory_usage_mb, Some(1024));
        } else {
            panic!("Expected WorkerHeartbeat message type");
        }
    }

    #[test]
    fn test_message_creation_task_control() {
        let control = TaskControlMessage {
            task_run_id: 999,
            action: TaskControlAction::Pause,
            requester: "admin".to_string(),
            timestamp: Utc::now(),
        };

        let message = Message::task_control(control.clone());

        assert!(!message.id.is_empty());
        assert_eq!(message.message_type_str(), "task_control");

        if let MessageType::TaskControl(msg) = &message.message_type {
            assert_eq!(msg.task_run_id, 999);
            assert_eq!(msg.action, TaskControlAction::Pause);
            assert_eq!(msg.requester, "admin");
        } else {
            panic!("Expected TaskControl message type");
        }
    }

    #[test]
    fn test_message_serialization_deserialization() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let original_message = Message::task_execution(task_execution);

        // Test JSON string serialization
        let json_str = original_message
            .serialize()
            .expect("Failed to serialize to JSON");
        let deserialized_message =
            Message::deserialize(&json_str).expect("Failed to deserialize from JSON");

        assert_eq!(original_message.id, deserialized_message.id);
        assert_eq!(
            original_message.retry_count,
            deserialized_message.retry_count
        );
        assert_eq!(
            original_message.message_type_str(),
            deserialized_message.message_type_str()
        );

        // Test bytes serialization
        let bytes = original_message
            .serialize_bytes()
            .expect("Failed to serialize to bytes");
        let deserialized_from_bytes =
            Message::deserialize_bytes(&bytes).expect("Failed to deserialize from bytes");

        assert_eq!(original_message.id, deserialized_from_bytes.id);
        assert_eq!(
            original_message.retry_count,
            deserialized_from_bytes.retry_count
        );
    }

    #[test]
    fn test_message_retry_functionality() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let mut message = Message::task_execution(task_execution);

        // Test initial retry count
        assert_eq!(message.retry_count, 0);
        assert!(!message.is_retry_exhausted(3));

        // Test increment retry
        message.increment_retry();
        assert_eq!(message.retry_count, 1);
        assert!(!message.is_retry_exhausted(3));

        // Test retry exhaustion
        message.increment_retry();
        message.increment_retry();
        assert_eq!(message.retry_count, 3);
        assert!(message.is_retry_exhausted(3));
    }

    #[test]
    fn test_message_correlation_id() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution)
            .with_correlation_id("correlation-123".to_string());

        assert_eq!(message.correlation_id, Some("correlation-123".to_string()));
    }

    #[test]
    fn test_message_routing_keys() {
        // Test TaskExecution routing key
        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };
        let task_message = Message::task_execution(task_execution);
        assert_eq!(task_message.routing_key(), "task.execution.shell");

        // Test StatusUpdate routing key
        let status_update = StatusUpdateMessage {
            task_run_id: 789,
            status: TaskRunStatus::Running,
            worker_id: "worker-001".to_string(),
            result: None,
            error_message: None,
            timestamp: Utc::now(),
        };
        let status_message = Message::status_update(status_update);
        assert_eq!(status_message.routing_key(), "status.update.worker-001");

        // Test WorkerHeartbeat routing key
        let heartbeat = WorkerHeartbeatMessage {
            worker_id: "worker-002".to_string(),
            current_task_count: 5,
            system_load: Some(0.75),
            memory_usage_mb: Some(1024),
            timestamp: Utc::now(),
        };
        let heartbeat_message = Message::worker_heartbeat(heartbeat);
        assert_eq!(
            heartbeat_message.routing_key(),
            "worker.heartbeat.worker-002"
        );

        // Test TaskControl routing key
        let control = TaskControlMessage {
            task_run_id: 999,
            action: TaskControlAction::Pause,
            requester: "admin".to_string(),
            timestamp: Utc::now(),
        };
        let control_message = Message::task_control(control);
        assert_eq!(control_message.routing_key(), "task.control.pause");
    }

    #[test]
    fn test_task_control_actions() {
        let actions = vec![
            TaskControlAction::Pause,
            TaskControlAction::Resume,
            TaskControlAction::Cancel,
            TaskControlAction::Restart,
        ];

        for action in actions {
            let control = TaskControlMessage {
                task_run_id: 999,
                action,
                requester: "admin".to_string(),
                timestamp: Utc::now(),
            };

            let message = Message::task_control(control);

            // Verify the action is properly serialized and accessible
            if let MessageType::TaskControl(msg) = &message.message_type {
                assert_eq!(msg.action, action);
            } else {
                panic!("Expected TaskControl message type");
            }
        }
    }

    #[test]
    fn test_message_payload_consistency() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: Some(1),
            shard_total: Some(3),
        };

        let message = Message::task_execution(task_execution.clone());

        // Verify that payload contains the same data as the message_type
        let expected_payload = serde_json::to_value(&task_execution).unwrap();
        assert_eq!(message.payload, expected_payload);
    }
}
