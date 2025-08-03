use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
///
/// # 使用示例
///
/// ```rust
/// use scheduler_core::models::{Task, TaskStatus};
/// use chrono::Utc;
/// use serde_json::json;
///
/// let task = Task {
///     id: 1,
///     name: "数据备份".to_string(),
///     task_type: "shell".to_string(),
///     schedule: "0 2 * * *".to_string(), // 每天凌晨2点
///     parameters: json!({"command": "backup.sh"}),
///     timeout_seconds: 3600,
///     max_retries: 3,
///     status: TaskStatus::Active,
///     dependencies: vec![],
///     shard_config: None,
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
/// };
/// ```
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
///
/// # 使用示例
///
/// ```rust
/// use scheduler_core::models::TaskStatus;
///
/// let status = TaskStatus::Active;
/// match status {
///     TaskStatus::Active => println!("任务活跃，可以执行"),
///     TaskStatus::Inactive => println!("任务暂停，无法执行"),
/// }
/// ```
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
