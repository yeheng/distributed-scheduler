use thiserror::Error;

/// 调度器错误类型定义
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
}

/// 统一的Result类型
pub type Result<T> = std::result::Result<T, SchedulerError>;
