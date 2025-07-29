use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
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
