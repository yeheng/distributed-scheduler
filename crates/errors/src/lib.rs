use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),
    #[error("数据库操作错误: {0}")]
    DatabaseOperation(String),
    #[error("任务未找到: {id}")]
    TaskNotFound { id: i64 },
    #[error("任务运行实例未找到: {id}")]
    TaskRunNotFound { id: i64 },
    #[error("Worker未找到: {id}")]
    WorkerNotFound { id: String },
    #[error("无效的CRON表达式: {expr} - {message}")]
    InvalidCron { expr: String, message: String },
    #[error("任务执行超时")]
    ExecutionTimeout,
    #[error("检测到循环依赖")]
    CircularDependency,
    #[error("无效的任务依赖: 任务 {task_id} 依赖任务 {dependency_id} - {reason}")]
    InvalidDependency {
        task_id: i64,
        dependency_id: i64,
        reason: String,
    },
    #[error("失去领导权")]
    LeadershipLost,
    #[error("消息队列错误: {0}")]
    MessageQueue(String),
    #[error("序列化错误: {0}")]
    Serialization(String),
    #[error("配置错误: {0}")]
    Configuration(String),
    #[error("任务执行错误: {0}")]
    TaskExecution(String),
    #[error("网络错误: {0}")]
    Network(String),
    #[error("内部错误: {0}")]
    Internal(String),
    #[error("无效的任务参数: {0}")]
    InvalidTaskParams(String),
    #[error("数据验证失败: {0}")]
    ValidationError(String),
    #[error("操作超时: {0}")]
    Timeout(String),
    #[error("资源不足: {0}")]
    ResourceExhausted(String),
    #[error("权限不足: {0}")]
    Permission(String),
}

pub type SchedulerResult<T> = Result<T, SchedulerError>;

impl SchedulerError {
    pub fn database_error<S: Into<String>>(msg: S) -> Self {
        Self::DatabaseOperation(msg.into())
    }
    pub fn task_not_found(id: i64) -> Self {
        Self::TaskNotFound { id }
    }
    pub fn worker_not_found<S: Into<String>>(id: S) -> Self {
        Self::WorkerNotFound { id: id.into() }
    }
    pub fn task_run_not_found(id: i64) -> Self {
        Self::TaskRunNotFound { id }
    }
    pub fn invalid_params<S: Into<String>>(msg: S) -> Self {
        Self::InvalidTaskParams(msg.into())
    }
    pub fn config_error<S: Into<String>>(msg: S) -> Self {
        Self::Configuration(msg.into())
    }
    pub fn validation_error<S: Into<String>>(msg: S) -> Self {
        Self::ValidationError(msg.into())
    }
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            SchedulerError::Internal(_)
                | SchedulerError::Configuration(_)
                | SchedulerError::ResourceExhausted(_)
        )
    }
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            SchedulerError::DatabaseOperation(_)
                | SchedulerError::MessageQueue(_)
                | SchedulerError::Network(_)
                | SchedulerError::Timeout(_)
        )
    }
    pub fn user_message(&self) -> &str {
        match self {
            SchedulerError::TaskNotFound { .. } => "请求的任务不存在",
            SchedulerError::WorkerNotFound { .. } => "请求的Worker节点不存在",
            SchedulerError::TaskRunNotFound { .. } => "请求的任务执行记录不存在",
            SchedulerError::InvalidTaskParams(_) => "任务参数配置有误",
            SchedulerError::ValidationError(_) => "输入数据验证失败",
            SchedulerError::Permission(_) => "您没有执行此操作的权限",
            SchedulerError::ResourceExhausted(_) => "系统资源不足，请稍后重试",
            SchedulerError::Timeout(_) => "操作超时，请稍后重试",
            _ => "系统繁忙，请稍后重试",
        }
    }
}

impl From<serde_json::Error> for SchedulerError {
    fn from(err: serde_json::Error) -> Self {
        SchedulerError::Serialization(err.to_string())
    }
}

impl From<anyhow::Error> for SchedulerError {
    fn from(err: anyhow::Error) -> Self {
        SchedulerError::Internal(err.to_string())
    }
}
