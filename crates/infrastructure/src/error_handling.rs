//! Enhanced error handling for repository operations with rich context
//! 
//! This module provides context-rich error helpers for all repository operations,
//! including detailed entity information, operation context, and structured logging.

use chrono::{DateTime, Utc};
use scheduler_domain::entities::{TaskRunStatus, WorkerStatus};
use scheduler_errors::SchedulerError;
use sqlx::Error as SqlxError;
use tracing::{error, warn, info, instrument};
use std::fmt;

/// Operation context for repository operations
#[derive(Debug, Clone)]
pub enum RepositoryOperation {
    Create,
    Read,
    Update,
    Delete,
    Query,
    BatchUpdate,
    BatchRead,
    Validate,
    Execute,
}

impl fmt::Display for RepositoryOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepositoryOperation::Create => write!(f, "创建"),
            RepositoryOperation::Read => write!(f, "查询"),
            RepositoryOperation::Update => write!(f, "更新"),
            RepositoryOperation::Delete => write!(f, "删除"),
            RepositoryOperation::Query => write!(f, "查询"),
            RepositoryOperation::BatchUpdate => write!(f, "批量更新"),
            RepositoryOperation::BatchRead => write!(f, "批量查询"),
            RepositoryOperation::Validate => write!(f, "验证"),
            RepositoryOperation::Execute => write!(f, "执行"),
        }
    }
}

/// Context information for task repository operations
#[derive(Debug, Clone)]
pub struct TaskOperationContext {
    pub operation: RepositoryOperation,
    pub task_id: Option<i64>,
    pub task_name: Option<String>,
    pub task_type: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub additional_info: Option<String>,
}

impl TaskOperationContext {
    pub fn new(operation: RepositoryOperation) -> Self {
        Self {
            operation,
            task_id: None,
            task_name: None,
            task_type: None,
            timestamp: Utc::now(),
            additional_info: None,
        }
    }

    pub fn with_task_id(mut self, task_id: i64) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn with_task_name(mut self, task_name: String) -> Self {
        self.task_name = Some(task_name);
        self
    }

    pub fn with_task_type(mut self, task_type: String) -> Self {
        self.task_type = Some(task_type);
        self
    }

    pub fn with_additional_info(mut self, info: String) -> Self {
        self.additional_info = Some(info);
        self
    }

    pub fn entity_description(&self) -> String {
        match (&self.task_id, &self.task_name) {
            (Some(id), Some(name)) => format!("任务 '{}' (ID: {})", name, id),
            (Some(id), None) => format!("任务 (ID: {})", id),
            (None, Some(name)) => format!("任务 '{}'", name),
            (None, None) => "任务".to_string(),
        }
    }
}

/// Context information for task run repository operations
#[derive(Debug, Clone)]
pub struct TaskRunOperationContext {
    pub operation: RepositoryOperation,
    pub run_id: Option<i64>,
    pub task_id: Option<i64>,
    pub worker_id: Option<String>,
    pub status: Option<TaskRunStatus>,
    pub timestamp: DateTime<Utc>,
    pub additional_info: Option<String>,
}

impl TaskRunOperationContext {
    pub fn new(operation: RepositoryOperation) -> Self {
        Self {
            operation,
            run_id: None,
            task_id: None,
            worker_id: None,
            status: None,
            timestamp: Utc::now(),
            additional_info: None,
        }
    }

    pub fn with_run_id(mut self, run_id: i64) -> Self {
        self.run_id = Some(run_id);
        self
    }

    pub fn with_task_id(mut self, task_id: i64) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn with_worker_id(mut self, worker_id: String) -> Self {
        self.worker_id = Some(worker_id);
        self
    }

    pub fn with_status(mut self, status: TaskRunStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_additional_info(mut self, info: String) -> Self {
        self.additional_info = Some(info);
        self
    }

    pub fn entity_description(&self) -> String {
        match (&self.run_id, &self.task_id) {
            (Some(run_id), Some(task_id)) => format!("任务执行实例 (ID: {}, 任务ID: {})", run_id, task_id),
            (Some(run_id), None) => format!("任务执行实例 (ID: {})", run_id),
            (None, Some(task_id)) => format!("任务执行实例 (任务ID: {})", task_id),
            (None, None) => "任务执行实例".to_string(),
        }
    }
}

/// Context information for worker repository operations
#[derive(Debug, Clone)]
pub struct WorkerOperationContext {
    pub operation: RepositoryOperation,
    pub worker_id: Option<String>,
    pub hostname: Option<String>,
    pub status: Option<WorkerStatus>,
    pub timestamp: DateTime<Utc>,
    pub additional_info: Option<String>,
}

impl WorkerOperationContext {
    pub fn new(operation: RepositoryOperation) -> Self {
        Self {
            operation,
            worker_id: None,
            hostname: None,
            status: None,
            timestamp: Utc::now(),
            additional_info: None,
        }
    }

    pub fn with_worker_id(mut self, worker_id: String) -> Self {
        self.worker_id = Some(worker_id);
        self
    }

    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.hostname = Some(hostname);
        self
    }

    pub fn with_status(mut self, status: WorkerStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_additional_info(mut self, info: String) -> Self {
        self.additional_info = Some(info);
        self
    }

    pub fn entity_description(&self) -> String {
        match (&self.worker_id, &self.hostname) {
            (Some(id), Some(hostname)) => format!("Worker '{}' ({})", id, hostname),
            (Some(id), None) => format!("Worker '{}'", id),
            (None, Some(hostname)) => format!("Worker ({})", hostname),
            (None, None) => "Worker".to_string(),
        }
    }
}

/// Context information for message queue operations
#[derive(Debug, Clone)]
pub struct MessageQueueOperationContext {
    pub operation: RepositoryOperation,
    pub queue_name: Option<String>,
    pub message_id: Option<String>,
    pub message_type: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub additional_info: Option<String>,
}

impl MessageQueueOperationContext {
    pub fn new(operation: RepositoryOperation) -> Self {
        Self {
            operation,
            queue_name: None,
            message_id: None,
            message_type: None,
            timestamp: Utc::now(),
            additional_info: None,
        }
    }

    pub fn with_queue_name(mut self, queue_name: String) -> Self {
        self.queue_name = Some(queue_name);
        self
    }

    pub fn with_message_id(mut self, message_id: String) -> Self {
        self.message_id = Some(message_id);
        self
    }

    pub fn with_message_type(mut self, message_type: String) -> Self {
        self.message_type = Some(message_type);
        self
    }

    pub fn with_additional_info(mut self, info: String) -> Self {
        self.additional_info = Some(info);
        self
    }

    pub fn entity_description(&self) -> String {
        match (&self.queue_name, &self.message_id) {
            (Some(queue), Some(msg_id)) => format!("消息队列 '{}' 中的消息 '{}'", queue, msg_id),
            (Some(queue), None) => format!("消息队列 '{}'", queue),
            (None, Some(msg_id)) => format!("消息 '{}'", msg_id),
            (None, None) => "消息队列".to_string(),
        }
    }
}

/// Enhanced error helpers for repository operations
pub struct RepositoryErrorHelpers;

impl RepositoryErrorHelpers {
    /// Create a database error with task context for batch operations
    #[instrument(skip_all)]
    pub fn database_error(context: TaskOperationContext, error: SqlxError) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = format!("{}{}时发生数据库错误: {}", operation_desc, entity_desc, error);
        error!(error = %error, "{}", error_msg);
        
        SchedulerError::database_error(error_msg)
    }

    /// Create a database error with task context
    #[instrument(skip_all, fields(
        operation = %context.operation,
        task_id = ?context.task_id,
        task_name = ?context.task_name,
        task_type = ?context.task_type,
        timestamp = %context.timestamp,
    ))]
    pub fn task_database_error(context: TaskOperationContext, error: SqlxError) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = match &error {
            SqlxError::Database(ref db_error) => {
                if let Some(constraint) = db_error.constraint() {
                    match constraint {
                        "tasks_name_key" => {
                            let msg = format!("{}{}时发生唯一约束冲突: 任务名称 '{}' 已存在", 
                                operation_desc, entity_desc, 
                                context.task_name.as_deref().unwrap_or("未知"));
                            error!(error = %error, constraint = constraint, "{}", msg);
                            return SchedulerError::database_error(msg);
                        }
                        "tasks_pkey" => {
                            let msg = format!("{}{}时发生主键冲突: 任务ID {} 已存在", 
                                operation_desc, entity_desc, 
                                context.task_id.unwrap_or(0));
                            error!(error = %error, constraint = constraint, "{}", msg);
                            return SchedulerError::database_error(msg);
                        }
                        _ => {
                            format!("{}{}时发生数据库约束冲突: {}", 
                                operation_desc, entity_desc, constraint)
                        }
                    }
                } else {
                    format!("{}{}时发生数据库错误: {}", operation_desc, entity_desc, db_error)
                }
            }
            SqlxError::PoolClosed => {
                format!("{}{}时数据库连接池已关闭", operation_desc, entity_desc)
            }
            SqlxError::PoolTimedOut => {
                format!("{}{}时数据库连接池超时", operation_desc, entity_desc)
            }
            SqlxError::Io(ref io_error) => {
                format!("{}{}时发生I/O错误: {}", operation_desc, entity_desc, io_error)
            }
            _ => {
                format!("{}{}时发生未知数据库错误: {}", operation_desc, entity_desc, error)
            }
        };

        error!(error = %error, "{}", error_msg);
        SchedulerError::database_error(error_msg)
    }

    /// Create a database error with task run context
    #[instrument(skip_all, fields(
        operation = %context.operation,
        run_id = ?context.run_id,
        task_id = ?context.task_id,
        worker_id = ?context.worker_id,
        status = ?context.status,
        timestamp = %context.timestamp,
    ))]
    pub fn task_run_database_error(context: TaskRunOperationContext, error: SqlxError) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = match &error {
            SqlxError::Database(ref db_error) => {
                if let Some(constraint) = db_error.constraint() {
                    match constraint {
                        "task_runs_pkey" => {
                            let msg = format!("{}{}时发生主键冲突: 执行ID {} 已存在", 
                                operation_desc, entity_desc, 
                                context.run_id.unwrap_or(0));
                            error!(error = %error, constraint = constraint, "{}", msg);
                            return SchedulerError::database_error(msg);
                        }
                        "task_runs_task_id_fkey" => {
                            let msg = format!("{}{}时发生外键约束冲突: 关联的任务ID {} 不存在", 
                                operation_desc, entity_desc, 
                                context.task_id.unwrap_or(0));
                            error!(error = %error, constraint = constraint, "{}", msg);
                            return SchedulerError::database_error(msg);
                        }
                        "task_runs_worker_id_fkey" => {
                            let msg = format!("{}{}时发生外键约束冲突: 关联的Worker '{}' 不存在", 
                                operation_desc, entity_desc, 
                                context.worker_id.as_deref().unwrap_or("未知"));
                            error!(error = %error, constraint = constraint, "{}", msg);
                            return SchedulerError::database_error(msg);
                        }
                        _ => {
                            format!("{}{}时发生数据库约束冲突: {}", 
                                operation_desc, entity_desc, constraint)
                        }
                    }
                } else {
                    format!("{}{}时发生数据库错误: {}", operation_desc, entity_desc, db_error)
                }
            }
            SqlxError::PoolClosed => {
                format!("{}{}时数据库连接池已关闭", operation_desc, entity_desc)
            }
            SqlxError::PoolTimedOut => {
                format!("{}{}时数据库连接池超时", operation_desc, entity_desc)
            }
            SqlxError::Io(ref io_error) => {
                format!("{}{}时发生I/O错误: {}", operation_desc, entity_desc, io_error)
            }
            _ => {
                format!("{}{}时发生未知数据库错误: {}", operation_desc, entity_desc, error)
            }
        };

        error!(error = %error, "{}", error_msg);
        SchedulerError::database_error(error_msg)
    }

    /// Create a database error with worker context
    #[instrument(skip_all, fields(
        operation = %context.operation,
        worker_id = ?context.worker_id,
        hostname = ?context.hostname,
        status = ?context.status,
        timestamp = %context.timestamp,
    ))]
    pub fn worker_database_error(context: WorkerOperationContext, error: SqlxError) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = match &error {
            SqlxError::Database(ref db_error) => {
                if let Some(constraint) = db_error.constraint() {
                    match constraint {
                        "workers_pkey" => {
                            let msg = format!("{}{}时发生主键冲突: Worker ID '{}' 已存在", 
                                operation_desc, entity_desc, 
                                context.worker_id.as_deref().unwrap_or("未知"));
                            error!(error = %error, constraint = constraint, "{}", msg);
                            return SchedulerError::database_error(msg);
                        }
                        "workers_hostname_key" => {
                            let msg = format!("{}{}时发生唯一约束冲突: 主机名 '{}' 已被使用", 
                                operation_desc, entity_desc, 
                                context.hostname.as_deref().unwrap_or("未知"));
                            error!(error = %error, constraint = constraint, "{}", msg);
                            return SchedulerError::database_error(msg);
                        }
                        _ => {
                            format!("{}{}时发生数据库约束冲突: {}", 
                                operation_desc, entity_desc, constraint)
                        }
                    }
                } else {
                    format!("{}{}时发生数据库错误: {}", operation_desc, entity_desc, db_error)
                }
            }
            SqlxError::PoolClosed => {
                format!("{}{}时数据库连接池已关闭", operation_desc, entity_desc)
            }
            SqlxError::PoolTimedOut => {
                format!("{}{}时数据库连接池超时", operation_desc, entity_desc)
            }
            SqlxError::Io(ref io_error) => {
                format!("{}{}时发生I/O错误: {}", operation_desc, entity_desc, io_error)
            }
            _ => {
                format!("{}{}时发生未知数据库错误: {}", operation_desc, entity_desc, error)
            }
        };

        error!(error = %error, "{}", error_msg);
        SchedulerError::database_error(error_msg)
    }

    /// Create a message queue error with context
    #[instrument(skip_all, fields(
        operation = %context.operation,
        queue_name = ?context.queue_name,
        message_id = ?context.message_id,
        message_type = ?context.message_type,
        timestamp = %context.timestamp,
    ))]
    pub fn message_queue_error(context: MessageQueueOperationContext, error: impl fmt::Display) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = format!("{}{}时发生消息队列错误: {}", operation_desc, entity_desc, error);
        
        error!(error = %error, "{}", error_msg);
        SchedulerError::MessageQueue(error_msg)
    }

    /// Create a serialization error with task context
    #[instrument(skip_all, fields(
        operation = %context.operation,
        task_id = ?context.task_id,
        task_name = ?context.task_name,
        timestamp = %context.timestamp,
    ))]
    pub fn task_serialization_error(context: TaskOperationContext, error: impl fmt::Display) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = format!("{}{}时序列化失败: {}", operation_desc, entity_desc, error);
        
        error!(error = %error, "{}", error_msg);
        SchedulerError::Serialization(error_msg)
    }

    /// Create a validation error with context
    #[instrument(skip_all, fields(
        operation = %context.operation,
        timestamp = %context.timestamp,
    ))]
    pub fn validation_error(context: TaskOperationContext, message: String) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = format!("{}{}时验证失败: {}", operation_desc, entity_desc, message);
        
        error!("{}", error_msg);
        SchedulerError::ValidationError(error_msg)
    }

    /// Log successful repository operation for task operations
    #[instrument(skip_all, fields(
        operation = %context.operation,
        entity_desc = %entity_desc,
        timestamp = %context.timestamp,
    ))]
    pub fn log_operation_success(
        context: TaskOperationContext, 
        entity_desc: &str, 
        additional_info: Option<&str>
    ) {
        let operation_desc = context.operation.to_string();
        let base_msg = format!("{}{}成功", operation_desc, entity_desc);
        
        if let Some(info) = additional_info {
            info!("{}: {}", base_msg, info);
        } else {
            info!("{}", base_msg);
        }
    }

    /// Log successful repository operation for task run operations
    #[instrument(skip_all, fields(
        operation = %context.operation,
        entity_desc = %entity_desc,
        timestamp = %context.timestamp,
    ))]
    pub fn log_operation_success_task_run(
        context: TaskRunOperationContext, 
        entity_desc: &str, 
        additional_info: Option<&str>
    ) {
        let operation_desc = context.operation.to_string();
        let base_msg = format!("{}{}成功", operation_desc, entity_desc);
        
        if let Some(info) = additional_info {
            info!("{}: {}", base_msg, info);
        } else {
            info!("{}", base_msg);
        }
    }

    /// Log successful repository operation for worker operations
    #[instrument(skip_all, fields(
        operation = %context.operation,
        entity_desc = %entity_desc,
        timestamp = %context.timestamp,
    ))]
    pub fn log_operation_success_worker(
        context: WorkerOperationContext, 
        entity_desc: &str, 
        additional_info: Option<&str>
    ) {
        let operation_desc = context.operation.to_string();
        let base_msg = format!("{}{}成功", operation_desc, entity_desc);
        
        if let Some(info) = additional_info {
            info!("{}: {}", base_msg, info);
        } else {
            info!("{}", base_msg);
        }
    }

    /// Log successful repository operation for message queue operations
    #[instrument(skip_all, fields(
        operation = %context.operation,
        entity_desc = %entity_desc,
        timestamp = %context.timestamp,
    ))]
    pub fn log_operation_success_message_queue(
        context: MessageQueueOperationContext, 
        entity_desc: &str, 
        additional_info: Option<&str>
    ) {
        let operation_desc = context.operation.to_string();
        let base_msg = format!("{}{}成功", operation_desc, entity_desc);
        
        if let Some(info) = additional_info {
            info!("{}: {}", base_msg, info);
        } else {
            info!("{}", base_msg);
        }
    }

    /// Log warning for task operations
    #[instrument(skip_all, fields(
        operation = %context.operation,
        entity_desc = %entity_desc,
        timestamp = %context.timestamp,
    ))]
    pub fn log_operation_warning(
        context: TaskOperationContext, 
        entity_desc: &str, 
        warning: &str
    ) {
        let operation_desc = context.operation.to_string();
        
        warn!("{}{}时警告: {}", operation_desc, entity_desc, warning);
    }

    /// Log warning for message queue operations
    #[instrument(skip_all, fields(
        operation = %context.operation,
        entity_desc = %entity_desc,
        timestamp = %context.timestamp,
    ))]
    pub fn log_operation_warning_message_queue(
        context: MessageQueueOperationContext, 
        entity_desc: &str, 
        warning: &str
    ) {
        let operation_desc = context.operation.to_string();
        
        warn!("{}{}时警告: {}", operation_desc, entity_desc, warning);
    }

    /// Create a task not found error with context
    pub fn task_not_found(context: TaskOperationContext) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = format!("{}{}时未找到: {} 不存在", operation_desc, entity_desc, entity_desc);
        
        error!("{}", error_msg);
        SchedulerError::TaskNotFound { id: context.task_id.unwrap_or(0) }
    }

    /// Create a task run not found error with context
    pub fn task_run_not_found(context: TaskRunOperationContext) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = format!("{}{}时未找到: {} 不存在", operation_desc, entity_desc, entity_desc);
        
        error!("{}", error_msg);
        SchedulerError::TaskRunNotFound { id: context.run_id.unwrap_or(0) }
    }

    /// Create a worker not found error with context
    pub fn worker_not_found(context: WorkerOperationContext) -> SchedulerError {
        let entity_desc = context.entity_description();
        let operation_desc = context.operation.to_string();
        
        let error_msg = format!("{}{}时未找到: {} 不存在", operation_desc, entity_desc, entity_desc);
        
        error!("{}", error_msg);
        SchedulerError::WorkerNotFound { id: context.worker_id.unwrap_or_default() }
    }
}

/// Macro for creating task operation context easily
#[macro_export]
macro_rules! task_context {
    ($operation:expr) => {
        $crate::error_handling::TaskOperationContext::new($operation)
    };
    ($operation:expr, task_id = $task_id:expr) => {
        $crate::error_handling::TaskOperationContext::new($operation).with_task_id($task_id)
    };
    ($operation:expr, task_name = $task_name:expr) => {
        $crate::error_handling::TaskOperationContext::new($operation).with_task_name($task_name.to_string())
    };
    ($operation:expr, task_id = $task_id:expr, task_name = $task_name:expr) => {
        $crate::error_handling::TaskOperationContext::new($operation)
            .with_task_id($task_id)
            .with_task_name($task_name.to_string())
    };
}

/// Macro for creating task run operation context easily
#[macro_export]
macro_rules! task_run_context {
    ($operation:expr) => {
        $crate::error_handling::TaskRunOperationContext::new($operation)
    };
    ($operation:expr, run_id = $run_id:expr) => {
        $crate::error_handling::TaskRunOperationContext::new($operation).with_run_id($run_id)
    };
    ($operation:expr, task_id = $task_id:expr) => {
        $crate::error_handling::TaskRunOperationContext::new($operation).with_task_id($task_id)
    };
    ($operation:expr, run_id = $run_id:expr, task_id = $task_id:expr) => {
        $crate::error_handling::TaskRunOperationContext::new($operation)
            .with_run_id($run_id)
            .with_task_id($task_id)
    };
}

/// Macro for creating worker operation context easily
#[macro_export]
macro_rules! worker_context {
    ($operation:expr) => {
        $crate::error_handling::WorkerOperationContext::new($operation)
    };
    ($operation:expr, worker_id = $worker_id:expr) => {
        $crate::error_handling::WorkerOperationContext::new($operation).with_worker_id($worker_id.to_string())
    };
    ($operation:expr, hostname = $hostname:expr) => {
        $crate::error_handling::WorkerOperationContext::new($operation).with_hostname($hostname.to_string())
    };
}

/// Macro for creating message queue operation context easily
#[macro_export]
macro_rules! message_queue_context {
    ($operation:expr) => {
        $crate::error_handling::MessageQueueOperationContext::new($operation)
    };
    ($operation:expr, queue = $queue:expr) => {
        $crate::error_handling::MessageQueueOperationContext::new($operation).with_queue_name($queue.to_string())
    };
    ($operation:expr, queue = $queue:expr, message_id = $message_id:expr) => {
        $crate::error_handling::MessageQueueOperationContext::new($operation)
            .with_queue_name($queue.to_string())
            .with_message_id($message_id.to_string())
    };
}