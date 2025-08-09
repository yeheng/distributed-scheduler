//! Domain Error Types
//!
//! 领域层错误类型定义，体现依赖倒置原则，不依赖外部技术实现。

use thiserror::Error;

/// 调度器统一错误类型
///
/// 定义系统中所有可能出现的错误情况，提供统一的错误处理机制。
/// 错误类型按功能模块和严重程度分类，便于错误处理和用户反馈。
#[derive(Error, Debug, Clone)]
pub enum SchedulerError {
    /// 数据库操作错误
    #[error("数据库操作失败: {0}")]
    DatabaseOperation(String),

    /// 任务不存在错误
    #[error("任务不存在: id={id}")]
    TaskNotFound { id: i64 },

    /// Worker不存在错误
    #[error("Worker不存在: id={id}")]
    WorkerNotFound { id: String },

    /// 任务运行实例不存在错误
    #[error("任务执行实例不存在: id={id}")]
    TaskRunNotFound { id: i64 },

    /// 无效的任务参数错误
    #[error("任务参数无效: {0}")]
    InvalidTaskParams(String),

    /// 配置错误
    #[error("配置错误: {0}")]
    Configuration(String),

    /// 序列化/反序列化错误
    #[error("数据序列化错误: {0}")]
    Serialization(String),

    /// 验证错误
    #[error("数据验证失败: {0}")]
    ValidationError(String),

    /// 消息队列错误
    #[error("消息队列操作失败: {0}")]
    MessageQueue(String),

    /// 网络连接错误
    #[error("网络连接失败: {0}")]
    Network(String),

    /// 权限不足错误
    #[error("权限不足: {0}")]
    Permission(String),

    /// 系统内部错误
    #[error("系统内部错误: {0}")]
    Internal(String),

    /// 超时错误
    #[error("操作超时: {0}")]
    Timeout(String),

    /// 资源不足错误
    #[error("资源不足: {0}")]
    ResourceExhausted(String),
}

/// 调度器结果类型
///
/// 系统中所有可能失败的操作都应该返回此类型。
/// 提供统一的错误处理和链式操作支持。
pub type SchedulerResult<T> = Result<T, SchedulerError>;

impl SchedulerError {
    /// 创建数据库操作错误
    pub fn database_error<S: Into<String>>(msg: S) -> Self {
        Self::DatabaseOperation(msg.into())
    }

    /// 创建任务不存在错误
    pub fn task_not_found(id: i64) -> Self {
        Self::TaskNotFound { id }
    }

    /// 创建Worker不存在错误
    pub fn worker_not_found<S: Into<String>>(id: S) -> Self {
        Self::WorkerNotFound { id: id.into() }
    }

    /// 创建任务运行实例不存在错误
    pub fn task_run_not_found(id: i64) -> Self {
        Self::TaskRunNotFound { id }
    }

    /// 创建参数验证错误
    pub fn invalid_params<S: Into<String>>(msg: S) -> Self {
        Self::InvalidTaskParams(msg.into())
    }

    /// 创建配置错误
    pub fn config_error<S: Into<String>>(msg: S) -> Self {
        Self::Configuration(msg.into())
    }

    /// 创建验证错误
    pub fn validation_error<S: Into<String>>(msg: S) -> Self {
        Self::ValidationError(msg.into())
    }

    /// 检查错误是否为致命错误
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            SchedulerError::Internal(_) 
            | SchedulerError::Configuration(_)
            | SchedulerError::ResourceExhausted(_)
        )
    }

    /// 检查错误是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            SchedulerError::DatabaseOperation(_)
            | SchedulerError::MessageQueue(_) 
            | SchedulerError::Network(_)
            | SchedulerError::Timeout(_)
        )
    }

    /// 获取错误的用户友好描述
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

// 实现从常见错误类型的转换
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