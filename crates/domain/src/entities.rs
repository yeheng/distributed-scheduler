use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub task_type: String, // "shell", "http", etc.
    pub schedule: String,  // cron 表达式
    pub parameters: serde_json::Value,
    pub timeout_seconds: i32,
    pub max_retries: i32,
    pub status: TaskStatus,     // ACTIVE, INACTIVE
    pub dependencies: Vec<i64>, // 依赖的任务 ID
    pub shard_config: Option<ShardConfig>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "INACTIVE")]
    Inactive,
}
impl sqlx::Type<sqlx::Postgres> for TaskStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("VARCHAR")
    }
}

impl sqlx::Type<sqlx::Sqlite> for TaskStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <str as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for TaskStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "ACTIVE" => Ok(TaskStatus::Active),
            "INACTIVE" => Ok(TaskStatus::Inactive),
            _ => Err(format!("Invalid task status: {s}").into()),
        }
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for TaskStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        match s {
            "ACTIVE" => Ok(TaskStatus::Active),
            "INACTIVE" => Ok(TaskStatus::Inactive),
            _ => Err(format!("Invalid task status: {s}").into()),
        }
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for TaskStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            TaskStatus::Active => "ACTIVE",
            TaskStatus::Inactive => "INACTIVE",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(s, buf)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for TaskStatus {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            TaskStatus::Active => "ACTIVE",
            TaskStatus::Inactive => "INACTIVE",
        };
        <&str as sqlx::Encode<sqlx::Sqlite>>::encode(s, buf)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    pub enabled: bool,
    pub shard_count: i32,
    pub shard_key: String,
}

#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub task_type: Option<String>,
    pub name_pattern: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl Task {
    pub fn new(
        name: String,
        task_type: String,
        schedule: String,
        parameters: serde_json::Value,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0, // 将由数据库生成
            name,
            task_type,
            schedule,
            parameters,
            timeout_seconds: 300, // 默认5分钟超时
            max_retries: 0,       // 默认不重试
            status: TaskStatus::Active,
            dependencies: Vec::new(),
            shard_config: None,
            created_at: now,
            updated_at: now,
        }
    }
    pub fn is_active(&self) -> bool {
        matches!(self.status, TaskStatus::Active)
    }
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }
    pub fn entity_description(&self) -> String {
        format!(
            "任务 '{}' (ID: {}, 类型: {})",
            self.name, self.id, self.task_type
        )
    }
}

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
    pub fn entity_description(&self) -> String {
        match &self.worker_id {
            Some(worker_id) => format!(
                "任务执行实例 (ID: {}, 任务ID: {}, Worker: {})",
                self.id, self.task_id, worker_id
            ),
            None => format!("任务执行实例 (ID: {}, 任务ID: {})", self.id, self.task_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub id: String,
    pub hostname: String,
    pub ip_address: String,
    pub supported_task_types: Vec<String>,
    pub max_concurrent_tasks: i32,
    pub current_task_count: i32,
    pub status: WorkerStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub registered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WorkerStatus {
    #[serde(rename = "ALIVE")]
    Alive,
    #[serde(rename = "DOWN")]
    Down,
}
impl sqlx::Type<sqlx::Postgres> for WorkerStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("VARCHAR")
    }
}

impl sqlx::Type<sqlx::Sqlite> for WorkerStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <&str as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for WorkerStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "ALIVE" => Ok(WorkerStatus::Alive),
            "DOWN" => Ok(WorkerStatus::Down),
            _ => Err(format!("Invalid worker status: {s}").into()),
        }
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for WorkerStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        match s {
            "ALIVE" => Ok(WorkerStatus::Alive),
            "DOWN" => Ok(WorkerStatus::Down),
            _ => Err(format!("Invalid worker status: {s}").into()),
        }
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for WorkerStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            WorkerStatus::Alive => "ALIVE",
            WorkerStatus::Down => "DOWN",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(s, buf)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for WorkerStatus {
    fn encode_by_ref(
        &self,
        args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            WorkerStatus::Alive => "ALIVE",
            WorkerStatus::Down => "DOWN",
        };
        <&str as sqlx::Encode<sqlx::Sqlite>>::encode(s, args)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRegistration {
    pub worker_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub supported_task_types: Vec<String>,
    pub max_concurrent_tasks: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHeartbeat {
    pub worker_id: String,
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

impl WorkerInfo {
    pub fn new(registration: WorkerRegistration) -> Self {
        let now = Utc::now();
        Self {
            id: registration.worker_id,
            hostname: registration.hostname,
            ip_address: registration.ip_address,
            supported_task_types: registration.supported_task_types,
            max_concurrent_tasks: registration.max_concurrent_tasks,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: now,
            registered_at: now,
        }
    }
    pub fn is_alive(&self) -> bool {
        matches!(self.status, WorkerStatus::Alive)
    }
    pub fn can_accept_task(&self, task_type: &str) -> bool {
        self.is_alive()
            && self.current_task_count < self.max_concurrent_tasks
            && self.supported_task_types.contains(&task_type.to_string())
    }
    pub fn load_percentage(&self) -> f64 {
        if self.max_concurrent_tasks == 0 {
            0.0
        } else {
            (self.current_task_count as f64 / self.max_concurrent_tasks as f64) * 100.0
        }
    }
    pub fn update_heartbeat(&mut self, heartbeat: WorkerHeartbeat) {
        self.current_task_count = heartbeat.current_task_count;
        self.last_heartbeat = heartbeat.timestamp;
        self.status = WorkerStatus::Alive;
    }
    pub fn is_heartbeat_expired(&self, timeout_seconds: i64) -> bool {
        let now = Utc::now();
        (now - self.last_heartbeat).num_seconds() > timeout_seconds
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub message_type: MessageType,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub retry_count: i32,
    pub correlation_id: Option<String>,
    /// Distributed tracing headers for context propagation
    pub trace_headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageType {
    TaskExecution(TaskExecutionMessage),
    StatusUpdate(StatusUpdateMessage),
    WorkerHeartbeat(WorkerHeartbeatMessage),
    TaskControl(TaskControlMessage),
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdateMessage {
    pub task_run_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: String,
    pub result: Option<TaskResult>,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHeartbeatMessage {
    pub worker_id: String,
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskControlMessage {
    pub task_run_id: i64,
    pub action: TaskControlAction,
    pub requester: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TaskControlAction {
    Pause,
    Resume,
    Cancel,
    Restart,
}

impl Message {
    /// 创建任务执行消息（可能失败版本）
    pub fn try_task_execution(message: TaskExecutionMessage) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(&message)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::TaskExecution(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
            trace_headers: None,
        })
    }

    /// 创建任务执行消息（兼容性版本，但会记录更详细的错误）
    pub fn task_execution(message: TaskExecutionMessage) -> Self {
        match Self::try_task_execution(message.clone()) {
            Ok(msg) => msg,
            Err(e) => {
                // 记录详细的错误信息而不是吞没错误
                let task_name = message.task_name.clone();
                let task_id = message.task_id;
                tracing::error!(
                    "Critical: Failed to serialize TaskExecutionMessage for task {}: {}. This may cause message loss!",
                    task_name, e
                );
                // 创建一个错误消息而不是空值
                Self {
                    id: Uuid::new_v4().to_string(),
                    message_type: MessageType::TaskExecution(message),
                    payload: serde_json::json!({
                        "error": "Serialization failed",
                        "message": e.to_string(),
                        "task_name": task_name,
                        "task_id": task_id
                    }),
                    timestamp: Utc::now(),
                    retry_count: 0,
                    correlation_id: None,
                    trace_headers: None,
                }
            }
        }
    }

    /// 创建状态更新消息（可能失败版本）
    pub fn try_status_update(message: StatusUpdateMessage) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(&message)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::StatusUpdate(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
            trace_headers: None,
        })
    }

    /// 创建状态更新消息（兼容性版本，但会记录更详细的错误）
    pub fn status_update(message: StatusUpdateMessage) -> Self {
        match Self::try_status_update(message.clone()) {
            Ok(msg) => msg,
            Err(e) => {
                // 记录详细的错误信息而不是吞没错误
                let task_run_id = message.task_run_id;
                tracing::error!(
                    "Critical: Failed to serialize StatusUpdateMessage for task_run {}: {}. This may cause message loss!",
                    task_run_id, e
                );
                // 创建一个错误消息而不是空值
                Self {
                    id: Uuid::new_v4().to_string(),
                    message_type: MessageType::StatusUpdate(message),
                    payload: serde_json::json!({
                        "error": "Serialization failed",
                        "message": e.to_string(),
                        "task_run_id": task_run_id
                    }),
                    timestamp: Utc::now(),
                    retry_count: 0,
                    correlation_id: None,
                    trace_headers: None,
                }
            }
        }
    }

    /// 创建工作节点心跳消息（可能失败版本）
    pub fn try_worker_heartbeat(
        message: WorkerHeartbeatMessage,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(&message)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::WorkerHeartbeat(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
            trace_headers: None,
        })
    }

    /// 创建工作节点心跳消息（兼容性版本，但会记录更详细的错误）
    pub fn worker_heartbeat(message: WorkerHeartbeatMessage) -> Self {
        match Self::try_worker_heartbeat(message.clone()) {
            Ok(msg) => msg,
            Err(e) => {
                let worker_id = message.worker_id.clone();
                tracing::error!(
                    "Critical: Failed to serialize WorkerHeartbeatMessage for worker {}: {}. This may cause heartbeat loss!",
                    worker_id, e
                );
                Self {
                    id: Uuid::new_v4().to_string(),
                    message_type: MessageType::WorkerHeartbeat(message),
                    payload: serde_json::json!({
                        "error": "Serialization failed",
                        "message": e.to_string(),
                        "worker_id": worker_id
                    }),
                    timestamp: Utc::now(),
                    retry_count: 0,
                    correlation_id: None,
                    trace_headers: None,
                }
            }
        }
    }

    /// 创建任务控制消息（可能失败版本）
    pub fn try_task_control(message: TaskControlMessage) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(&message)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::TaskControl(message),
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id: None,
            trace_headers: None,
        })
    }

    /// 创建任务控制消息（兼容性版本，但会记录更详细的错误）
    pub fn task_control(message: TaskControlMessage) -> Self {
        match Self::try_task_control(message.clone()) {
            Ok(msg) => msg,
            Err(e) => {
                let task_run_id = message.task_run_id;
                let action = format!("{:?}", message.action);
                tracing::error!(
                    "Critical: Failed to serialize TaskControlMessage for task_run {}: {}. This may cause control message loss!",
                    task_run_id, e
                );
                Self {
                    id: Uuid::new_v4().to_string(),
                    message_type: MessageType::TaskControl(message),
                    payload: serde_json::json!({
                        "error": "Serialization failed",
                        "message": e.to_string(),
                        "task_run_id": task_run_id,
                        "action": action
                    }),
                    timestamp: Utc::now(),
                    retry_count: 0,
                    correlation_id: None,
                    trace_headers: None,
                }
            }
        }
    }
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// 设置分布式追踪头部信息
    pub fn with_trace_headers(
        mut self,
        headers: std::collections::HashMap<String, String>,
    ) -> Self {
        self.trace_headers = Some(headers);
        self
    }

    /// 获取分布式追踪头部信息
    pub fn get_trace_headers(&self) -> Option<&std::collections::HashMap<String, String>> {
        self.trace_headers.as_ref()
    }

    /// 注入当前tracing span的上下文到消息中
    /// 注意：此方法需要在有observability依赖的crate中使用具体实现
    pub fn inject_current_trace_context(self) -> Self {
        // This is a placeholder - actual implementation should be done
        // in crates that have observability dependencies
        self
    }

    pub fn is_retry_exhausted(&self, max_retries: i32) -> bool {
        self.retry_count >= max_retries
    }
    pub fn serialize(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    pub fn deserialize(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
    pub fn serialize_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    pub fn deserialize_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
    pub fn message_type_str(&self) -> &'static str {
        match &self.message_type {
            MessageType::TaskExecution(_) => "task_execution",
            MessageType::StatusUpdate(_) => "status_update",
            MessageType::WorkerHeartbeat(_) => "worker_heartbeat",
            MessageType::TaskControl(_) => "task_control",
        }
    }
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
