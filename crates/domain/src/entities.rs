//! Domain Entities
//!
//! 核心领域实体定义，包含任务、任务运行实例、Worker节点和消息等业务核心概念。
//! 这些实体是系统的核心业务模型，不依赖于外部技术实现。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// 任务相关实体
// ============================================================================

/// 任务定义
///
/// 表示系统中可调度执行的任务单元，包含任务的完整配置信息。
///
/// # 字段说明
///
/// - `id`: 任务的唯一标识符
/// - `name`: 任务的人类可读名称
/// - `task_type`: 任务类型，如 "shell"、"http" 等
/// - `schedule`: cron 表达式，定义任务的执行时间
/// - `parameters`: 任务执行所需的参数，JSON 格式
/// - `timeout_seconds`: 任务执行超时时间（秒）
/// - `max_retries`: 最大重试次数
/// - `status`: 任务状态（ACTIVE/INACTIVE）
/// - `dependencies`: 依赖的任务 ID 列表
/// - `shard_config`: 分片配置，用于任务分片执行
/// - `created_at`: 任务创建时间
/// - `updated_at`: 任务最后更新时间
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

/// 任务状态
///
/// 定义任务的生命周期状态，用于控制任务的调度和执行。
///
/// # 变体说明
///
/// - `Active`: 任务处于活跃状态，可以正常调度和执行
/// - `Inactive`: 任务处于非活跃状态，暂停调度和执行
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "INACTIVE")]
    Inactive,
}

// SQLx 数据库类型支持 - TaskStatus
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

/// 分片配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    pub enabled: bool,
    pub shard_count: i32,
    pub shard_key: String,
}

/// 任务过滤器
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub task_type: Option<String>,
    pub name_pattern: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl Task {
    /// 创建新任务
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

    /// 检查任务是否处于活跃状态
    pub fn is_active(&self) -> bool {
        matches!(self.status, TaskStatus::Active)
    }

    /// 检查任务是否有依赖
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }
}

// ============================================================================
// 任务运行相关实体
// ============================================================================

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

/// 任务执行上下文
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

/// 任务运行状态
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

// SQLx 数据库类型支持 - TaskRunStatus
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
        matches!(
            self.status,
            TaskRunStatus::Running | TaskRunStatus::Dispatched
        )
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

// ============================================================================
// Worker相关实体
// ============================================================================

/// Worker节点信息
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

/// Worker状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WorkerStatus {
    #[serde(rename = "ALIVE")]
    Alive,
    #[serde(rename = "DOWN")]
    Down,
}

// SQLx 数据库类型支持 - WorkerStatus
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

/// Worker注册请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRegistration {
    pub worker_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub supported_task_types: Vec<String>,
    pub max_concurrent_tasks: i32,
}

/// Worker心跳信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHeartbeat {
    pub worker_id: String,
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

impl WorkerInfo {
    /// 创建新的Worker信息
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

    /// 检查Worker是否存活
    pub fn is_alive(&self) -> bool {
        matches!(self.status, WorkerStatus::Alive)
    }

    /// 检查Worker是否可以接受新任务
    pub fn can_accept_task(&self, task_type: &str) -> bool {
        self.is_alive()
            && self.current_task_count < self.max_concurrent_tasks
            && self.supported_task_types.contains(&task_type.to_string())
    }

    /// 获取Worker负载率
    pub fn load_percentage(&self) -> f64 {
        if self.max_concurrent_tasks == 0 {
            0.0
        } else {
            (self.current_task_count as f64 / self.max_concurrent_tasks as f64) * 100.0
        }
    }

    /// 更新心跳信息
    pub fn update_heartbeat(&mut self, heartbeat: WorkerHeartbeat) {
        self.current_task_count = heartbeat.current_task_count;
        self.last_heartbeat = heartbeat.timestamp;
        self.status = WorkerStatus::Alive;
    }

    /// 检查心跳是否超时
    pub fn is_heartbeat_expired(&self, timeout_seconds: i64) -> bool {
        let now = Utc::now();
        (now - self.last_heartbeat).num_seconds() > timeout_seconds
    }
}

// ============================================================================
// 消息相关实体
// ============================================================================

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