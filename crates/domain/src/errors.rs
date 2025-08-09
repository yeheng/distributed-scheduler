
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum SchedulerError {
    #[error("数据库操作失败: {0}")]
    DatabaseOperation(String),
    #[error("任务不存在: id={id}")]
    TaskNotFound { id: i64 },
    #[error("Worker不存在: id={id}")]
    WorkerNotFound { id: String },
    #[error("任务执行实例不存在: id={id}")]
    TaskRunNotFound { id: i64 },
    #[error("任务参数无效: {0}")]
    InvalidTaskParams(String),
    #[error("配置错误: {0}")]
    Configuration(String),
    #[error("数据序列化错误: {0}")]
    Serialization(String),
    #[error("数据验证失败: {0}")]
    ValidationError(String),
    #[error("消息队列操作失败: {0}")]
    MessageQueue(String),
    #[error("网络连接失败: {0}")]
    Network(String),
    #[error("权限不足: {0}")]
    Permission(String),
    #[error("系统内部错误: {0}")]
    Internal(String),
    #[error("操作超时: {0}")]
    Timeout(String),
    #[error("资源不足: {0}")]
    ResourceExhausted(String),
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
impl From<sqlx::Error> for SchedulerError {
    fn from(err: sqlx::Error) -> Self {
        SchedulerError::DatabaseOperation(err.to_string())
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