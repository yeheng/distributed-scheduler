use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub task_type: String,           // "shell", "http", etc.
    pub schedule: String,            // cron 表达式
    pub parameters: serde_json::Value,
    pub timeout_seconds: i32,
    pub max_retries: i32,
    pub status: TaskStatus,          // ACTIVE, INACTIVE
    pub dependencies: Vec<i64>,      // 依赖的任务 ID
    pub shard_config: Option<ShardConfig>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 任务状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "task_status", rename_all = "UPPERCASE")]
pub enum TaskStatus {
    Active,
    Inactive,
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