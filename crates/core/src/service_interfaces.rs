//! # 服务接口定义模块
//!
//! 定义了调度系统中所有服务的抽象接口，遵循依赖倒置原则，
//! 使得具体实现可以独立变化而不影响接口使用者。
//!
//! ## 模块组织
//!
//! ```text
//! service_interfaces
//! ├── task_services      # 任务管理相关服务
//! │   ├── TaskControlService     # 任务控制服务
//! │   ├── TaskSchedulerService   # 任务调度服务
//! │   └── TaskDispatchService    # 任务分发服务
//! ├── worker_services    # Worker管理相关服务
//! │   ├── WorkerManagementService # Worker管理服务
//! │   └── WorkerHealthService     # Worker健康检查服务
//! ├── system_services    # 系统级服务
//! │   ├── ConfigurationService   # 配置管理服务
//! │   ├── MonitoringService      # 监控服务
//! │   └── AuditService          # 审计服务
//! └── ServiceFactory     # 服务工厂
//! ```
//!
//! ## 设计原则
//!
//! ### SOLID原则应用
//! - **单一职责**: 每个服务接口职责明确
//! - **开闭原则**: 通过接口扩展功能
//! - **里氏替换**: 所有实现可互换使用
//! - **接口隔离**: 接口细粒度，避免臃肿
//! - **依赖倒置**: 依赖抽象而非具体实现
//!
//! ### 异步设计
//! - 所有服务方法都是异步的
//! - 支持高并发和非阻塞操作
//! - 统一的错误处理机制
//!
//! ## 使用示例
//!
//! ### 任务控制服务使用
//!
//! ```rust
//! use scheduler_core::service_interfaces::task_services::TaskControlService;
//! use std::sync::Arc;
//!
//! async fn trigger_task_example(
//!     task_service: Arc<dyn TaskControlService>,
//!     task_id: i64
//! ) -> SchedulerResult<()> {
//!     // 触发任务执行
//!     let task_run = task_service.trigger_task(task_id).await?;
//!     println!("任务运行实例创建: {}", task_run.id);
//!     
//!     // 检查是否有运行中的实例
//!     let has_running = task_service.has_running_instances(task_id).await?;
//!     if has_running {
//!         println!("任务有运行中的实例");
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Worker管理服务使用
//!
//! ```rust
//! use scheduler_core::service_interfaces::worker_services::WorkerManagementService;
//! use scheduler_domain::entities::{WorkerInfo, WorkerStatus};
//!
//! async fn manage_worker_example(
//!     worker_service: Arc<dyn WorkerManagementService>
//! ) -> SchedulerResult<()> {
//!     // 获取活跃的Worker列表
//!     let workers = worker_service.get_active_workers().await?;
//!     println!("活跃Worker数量: {}", workers.len());
//!     
//!     // 选择最佳Worker执行Python任务
//!     if let Some(worker_id) = worker_service.select_best_worker("python").await? {
//!         println!("选择Worker: {}", worker_id);
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### 服务工厂使用
//!
//! ```rust
//! use scheduler_core::service_interfaces::ServiceFactory;
//!
//! async fn create_services_example(
//!     factory: Arc<dyn ServiceFactory>
//! ) -> SchedulerResult<()> {
//!     // 创建任务控制服务
//!     let task_service = factory.create_task_control_service().await?;
//!     
//!     // 创建Worker管理服务
//!     let worker_service = factory.create_worker_management_service().await?;
//!     
//!     // 创建监控服务
//!     let monitoring_service = factory.create_monitoring_service().await?;
//!     
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{
    models::{Task, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus},
    SchedulerResult,
};

/// 任务管理服务模块
///
/// 包含所有与任务生命周期管理、调度和分发相关的服务接口。
/// 这些服务负责任务的创建、执行、监控和状态管理。
pub mod task_services {
    use super::*;

    /// 任务控制服务接口
    ///
    /// 提供任务生命周期的核心控制功能，包括手动触发、暂停、恢复、重启和中止等操作。
    /// 这是任务管理的核心接口，被API层和调度器使用。
    ///
    /// # 设计理念
    ///
    /// - **状态管理**: 完整的任务状态转换控制
    /// - **实例控制**: 精确控制任务运行实例
    /// - **批量操作**: 支持批量取消和管理
    /// - **历史查询**: 提供执行历史查询功能
    ///
    /// # 实现要求
    ///
    /// - 必须是线程安全的 (`Send + Sync`)
    /// - 所有操作都是异步的
    /// - 提供详细的错误信息
    /// - 支持并发操作
    ///
    /// # 使用场景
    ///
    /// - Web API接口实现
    /// - 管理界面后端服务
    /// - 自动化运维脚本
    /// - 监控和告警系统
    ///
    /// # 示例实现
    ///
    /// ```rust
    /// use async_trait::async_trait;
    /// use scheduler_core::service_interfaces::task_services::TaskControlService;
    /// use scheduler_core::{SchedulerResult, models::{TaskRun, Task}};
    ///
    /// pub struct MyTaskControlService {
    ///     // 依赖的仓储和其他服务
    /// }
    ///
    /// #[async_trait]
    /// impl TaskControlService for MyTaskControlService {
    ///     async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
    ///         // 1. 验证任务存在且可执行
    ///         // 2. 检查依赖条件
    ///         // 3. 创建任务运行实例
    ///         // 4. 发送到执行队列
    ///         // 5. 返回运行实例
    ///         todo!()
    ///     }
    ///     
    ///     // 其他方法实现...
    /// }
    /// ```
    #[async_trait]
    pub trait TaskControlService: Send + Sync {
        /// 手动触发任务执行
        ///
        /// 立即创建并启动指定任务的新运行实例，忽略正常的调度时间。
        /// 这是最常用的任务控制操作，用于手动执行、测试或紧急处理。
        ///
        /// # 参数
        ///
        /// * `task_id` - 要触发的任务ID
        ///
        /// # 返回值
        ///
        /// 成功时返回新创建的 `TaskRun` 实例，包含运行ID和初始状态。
        ///
        /// # 错误
        ///
        /// * `TaskNotFound` - 任务不存在
        /// * `InvalidTaskParams` - 任务状态不允许执行（如已暂停）
        /// * `CircularDependency` - 任务依赖形成循环
        /// * `MessageQueue` - 发送到执行队列失败
        ///
        /// # 执行逻辑
        ///
        /// 1. 验证任务存在且状态为Active
        /// 2. 检查任务依赖是否满足
        /// 3. 创建新的TaskRun实例
        /// 4. 发送到消息队列等待执行
        /// 5. 更新任务的last_run_at时间
        ///
        /// # 示例
        ///
        /// ```rust
        /// let task_run = task_service.trigger_task(123).await?;
        /// println!("任务运行实例ID: {}", task_run.id);
        /// println!("状态: {:?}", task_run.status);
        /// ```
        async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;

        /// 暂停任务的调度执行
        ///
        /// 将任务状态设置为暂停，阻止调度器创建新的运行实例。
        /// 已经运行中的实例不会受到影响，会继续执行直到完成。
        ///
        /// # 参数
        ///
        /// * `task_id` - 要暂停的任务ID
        ///
        /// # 返回值
        ///
        /// 成功时返回 `Ok(())`。
        ///
        /// # 错误
        ///
        /// * `TaskNotFound` - 任务不存在
        /// * `DatabaseOperation` - 更新任务状态失败
        ///
        /// # 执行逻辑
        ///
        /// 1. 验证任务存在
        /// 2. 将任务状态更新为Paused
        /// 3. 记录暂停时间和操作者
        /// 4. 发送任务状态变更事件
        ///
        /// # 示例
        ///
        /// ```rust
        /// task_service.pause_task(123).await?;
        /// println!("任务已暂停");
        /// ```
        async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;

        /// 恢复已暂停任务的调度执行
        ///
        /// 将暂停状态的任务恢复为活跃状态，允许调度器重新创建运行实例。
        /// 恢复后任务将按照原有的调度规则继续执行。
        ///
        /// # 参数
        ///
        /// * `task_id` - 要恢复的任务ID
        ///
        /// # 返回值
        ///
        /// 成功时返回 `Ok(())`。
        ///
        /// # 错误
        ///
        /// * `TaskNotFound` - 任务不存在
        /// * `InvalidTaskParams` - 任务状态不是暂停状态
        /// * `DatabaseOperation` - 更新任务状态失败
        ///
        /// # 执行逻辑
        ///
        /// 1. 验证任务存在且状态为Paused
        /// 2. 将任务状态更新为Active
        /// 3. 重新计算下次执行时间
        /// 4. 发送任务状态变更事件
        ///
        /// # 示例
        ///
        /// ```rust
        /// task_service.resume_task(123).await?;
        /// println!("任务已恢复");
        /// ```
        async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;

        /// 重启指定的任务运行实例
        ///
        /// 停止当前运行实例并创建新的实例重新执行。
        /// 适用于任务执行异常或需要重新执行的场景。
        ///
        /// # 参数
        ///
        /// * `task_run_id` - 要重启的任务运行实例ID
        ///
        /// # 返回值
        ///
        /// 成功时返回新创建的 `TaskRun` 实例。
        ///
        /// # 错误
        ///
        /// * `TaskRunNotFound` - 任务运行实例不存在
        /// * `InvalidTaskParams` - 实例状态不允许重启
        /// * `MessageQueue` - 发送到执行队列失败
        ///
        /// # 执行逻辑
        ///
        /// 1. 验证任务运行实例存在
        /// 2. 停止当前实例（如果正在运行）
        /// 3. 创建新的运行实例
        /// 4. 复制原实例的参数和配置
        /// 5. 发送到执行队列
        ///
        /// # 示例
        ///
        /// ```rust
        /// let new_run = task_service.restart_task_run(456).await?;
        /// println!("重启后的运行实例ID: {}", new_run.id);
        /// ```
        async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;

        /// 中止指定的任务运行实例
        ///
        /// 强制停止正在运行的任务实例，将其状态设置为已中止。
        /// 这是一个强制操作，会立即终止任务执行。
        ///
        /// # 参数
        ///
        /// * `task_run_id` - 要中止的任务运行实例ID
        ///
        /// # 返回值
        ///
        /// 成功时返回 `Ok(())`。
        ///
        /// # 错误
        ///
        /// * `TaskRunNotFound` - 任务运行实例不存在
        /// * `InvalidTaskParams` - 实例状态不允许中止
        /// * `MessageQueue` - 发送中止信号失败
        ///
        /// # 执行逻辑
        ///
        /// 1. 验证任务运行实例存在
        /// 2. 发送中止信号到执行器
        /// 3. 更新实例状态为Aborted
        /// 4. 记录中止时间和原因
        /// 5. 清理相关资源
        ///
        /// # 示例
        ///
        /// ```rust
        /// task_service.abort_task_run(456).await?;
        /// println!("任务运行实例已中止");
        /// ```
        async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;

        /// 取消指定任务的所有运行实例
        ///
        /// 批量取消指定任务的所有正在运行或等待执行的实例。
        /// 这是一个批量操作，适用于需要完全停止某个任务的场景。
        ///
        /// # 参数
        ///
        /// * `task_id` - 要取消所有实例的任务ID
        ///
        /// # 返回值
        ///
        /// 成功时返回被取消的实例数量。
        ///
        /// # 错误
        ///
        /// * `TaskNotFound` - 任务不存在
        /// * `DatabaseOperation` - 批量更新失败
        /// * `MessageQueue` - 发送取消信号失败
        ///
        /// # 执行逻辑
        ///
        /// 1. 查找任务的所有活跃实例
        /// 2. 批量发送取消信号
        /// 3. 更新实例状态为Cancelled
        /// 4. 清理相关资源
        /// 5. 返回取消的实例数量
        ///
        /// # 示例
        ///
        /// ```rust
        /// let cancelled_count = task_service.cancel_all_task_runs(123).await?;
        /// println!("取消了 {} 个运行实例", cancelled_count);
        /// ```
        async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize>;

        /// 检查任务是否有运行中的实例
        ///
        /// 快速检查指定任务是否有正在运行或等待执行的实例。
        /// 用于防止重复执行或检查任务状态。
        ///
        /// # 参数
        ///
        /// * `task_id` - 要检查的任务ID
        ///
        /// # 返回值
        ///
        /// - `true` - 有运行中的实例
        /// - `false` - 没有运行中的实例
        ///
        /// # 错误
        ///
        /// * `TaskNotFound` - 任务不存在
        /// * `DatabaseOperation` - 查询失败
        ///
        /// # 检查范围
        ///
        /// 包括以下状态的实例：
        /// - Pending（等待执行）
        /// - Running（正在运行）
        /// - Queued（已排队）
        ///
        /// # 示例
        ///
        /// ```rust
        /// if task_service.has_running_instances(123).await? {
        ///     println!("任务有运行中的实例，跳过本次执行");
        /// } else {
        ///     println!("任务空闲，可以执行");
        /// }
        /// ```
        async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool>;

        /// 获取任务的最近执行历史
        ///
        /// 查询指定任务的最近执行记录，按执行时间倒序排列。
        /// 用于监控、调试和历史分析。
        ///
        /// # 参数
        ///
        /// * `task_id` - 要查询的任务ID
        /// * `limit` - 返回记录的最大数量
        ///
        /// # 返回值
        ///
        /// 返回 `TaskRun` 实例的向量，按 `started_at` 时间倒序排列。
        ///
        /// # 错误
        ///
        /// * `TaskNotFound` - 任务不存在
        /// * `DatabaseOperation` - 查询失败
        /// * `InvalidTaskParams` - limit参数无效（如为0）
        ///
        /// # 查询逻辑
        ///
        /// 1. 验证任务存在
        /// 2. 查询任务的运行历史
        /// 3. 按开始时间倒序排序
        /// 4. 限制返回数量
        /// 5. 包含完整的执行信息
        ///
        /// # 示例
        ///
        /// ```rust
        /// let recent_runs = task_service.get_recent_executions(123, 10).await?;
        /// for run in recent_runs {
        ///     println!("运行ID: {}, 状态: {:?}, 开始时间: {:?}", 
        ///              run.id, run.status, run.started_at);
        /// }
        /// ```
        async fn get_recent_executions(
            &self,
            task_id: i64,
            limit: usize,
        ) -> SchedulerResult<Vec<TaskRun>>;
    }

    /// Task scheduler service - Handle task scheduling and execution
    #[async_trait]
    pub trait TaskSchedulerService: Send + Sync {
        /// Start the scheduler
        async fn start(&self) -> SchedulerResult<()>;

        /// Stop the scheduler
        async fn stop(&self) -> SchedulerResult<()>;

        /// Schedule single task
        async fn schedule_task(&self, task: &Task) -> SchedulerResult<()>;

        /// Schedule multiple tasks
        async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()>;

        /// Scan and schedule pending tasks
        async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>>;

        /// Check task dependencies
        async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool>;

        /// Create task run instance
        async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun>;

        /// Dispatch task to queue
        async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()>;

        /// Check if scheduler is running
        async fn is_running(&self) -> bool;

        /// Get scheduler statistics
        async fn get_stats(&self) -> SchedulerResult<SchedulerStats>;

        /// Reload scheduler configuration
        async fn reload_config(&self) -> SchedulerResult<()>;
    }

    /// Task dispatch service - Handle task distribution to workers
    #[async_trait]
    pub trait TaskDispatchService: Send + Sync {
        /// Dispatch task to specific worker
        async fn dispatch_task(&self, task_run: &TaskRun, worker_id: &str) -> SchedulerResult<()>;

        /// Batch dispatch multiple tasks
        async fn dispatch_tasks(&self, dispatches: &[(TaskRun, String)]) -> SchedulerResult<()>;

        /// Handle task status updates
        async fn handle_status_update(
            &self,
            task_run_id: i64,
            status: TaskRunStatus,
            error_message: Option<String>,
        ) -> SchedulerResult<()>;

        /// Redispatch failed tasks
        async fn redispatch_failed_tasks(&self) -> SchedulerResult<usize>;

        /// Get dispatch statistics
        async fn get_dispatch_stats(&self) -> SchedulerResult<DispatchStats>;
    }
}

/// Worker management services - Handle worker lifecycle and operations
pub mod worker_services {
    use super::*;

    /// Worker management service - Handle worker registration and lifecycle
    #[async_trait]
    pub trait WorkerManagementService: Send + Sync {
        /// Register new worker
        async fn register_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()>;

        /// Unregister worker
        async fn unregister_worker(&self, worker_id: &str) -> SchedulerResult<()>;

        /// Update worker status
        async fn update_worker_status(
            &self,
            worker_id: &str,
            status: WorkerStatus,
        ) -> SchedulerResult<()>;

        /// Get list of active workers
        async fn get_active_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;

        /// Get worker details
        async fn get_worker_details(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;

        /// Check worker health
        async fn check_worker_health(&self, worker_id: &str) -> SchedulerResult<bool>;

        /// Get worker load statistics
        async fn get_worker_load_stats(&self) -> SchedulerResult<HashMap<String, WorkerLoadStats>>;

        /// Select best worker for task type
        async fn select_best_worker(&self, task_type: &str) -> SchedulerResult<Option<String>>;

        /// Process worker heartbeat
        async fn process_heartbeat(
            &self,
            worker_id: &str,
            heartbeat_data: &WorkerHeartbeat,
        ) -> SchedulerResult<()>;
    }

    /// Worker health service - Handle worker monitoring and health checks
    #[async_trait]
    pub trait WorkerHealthService: Send + Sync {
        /// Perform health check on worker
        async fn perform_health_check(&self, worker_id: &str)
            -> SchedulerResult<HealthCheckResult>;

        /// Get worker health status
        async fn get_worker_health_status(
            &self,
            worker_id: &str,
        ) -> SchedulerResult<WorkerHealthStatus>;

        /// Update worker health metrics
        async fn update_health_metrics(
            &self,
            worker_id: &str,
            metrics: WorkerHealthMetrics,
        ) -> SchedulerResult<()>;

        /// Get unhealthy workers
        async fn get_unhealthy_workers(&self) -> SchedulerResult<Vec<String>>;

        /// Handle worker failure
        async fn handle_worker_failure(&self, worker_id: &str) -> SchedulerResult<()>;
    }
}

/// System services - Handle system-level operations
pub mod system_services {
    use super::*;

    /// Configuration service - Handle system configuration
    #[async_trait]
    pub trait ConfigurationService: Send + Sync {
        /// Get configuration value
        async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<Value>>;

        /// Set configuration value
        async fn set_config_value(&self, key: &str, value: &Value) -> SchedulerResult<()>;

        /// Delete configuration
        async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;

        /// List all configuration keys
        async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;

        /// Reload configuration
        async fn reload_config(&self) -> SchedulerResult<()>;

        /// Watch for configuration changes
        async fn watch_config(&self, key: &str) -> SchedulerResult<Box<dyn ConfigWatcher>>;
    }

    /// Monitoring service - Handle system monitoring and metrics
    #[async_trait]
    pub trait MonitoringService: Send + Sync {
        /// Record metric
        async fn record_metric(
            &self,
            name: &str,
            value: f64,
            tags: &HashMap<String, String>,
        ) -> SchedulerResult<()>;

        /// Record event
        async fn record_event(&self, event_type: &str, data: &Value) -> SchedulerResult<()>;

        /// Get system health
        async fn get_system_health(&self) -> SchedulerResult<SystemHealth>;

        /// Get performance metrics
        async fn get_performance_metrics(
            &self,
            time_range: TimeRange,
        ) -> SchedulerResult<PerformanceMetrics>;

        /// Set alert rule
        async fn set_alert_rule(&self, rule: &AlertRule) -> SchedulerResult<()>;

        /// Check alerts
        async fn check_alerts(&self) -> SchedulerResult<Vec<Alert>>;
    }

    /// Audit service - Handle audit logging and compliance
    #[async_trait]
    pub trait AuditService: Send + Sync {
        /// Log audit event
        async fn log_event(&self, event: &AuditEvent) -> SchedulerResult<()>;

        /// Query audit events
        async fn query_events(&self, query: &AuditQuery) -> SchedulerResult<Vec<AuditEvent>>;

        /// Get audit statistics
        async fn get_audit_stats(&self, time_range: TimeRange) -> SchedulerResult<AuditStats>;

        /// Export audit events
        async fn export_events(
            &self,
            query: &AuditQuery,
            format: ExportFormat,
        ) -> SchedulerResult<Vec<u8>>;
    }
}

/// Service factory - Create service instances
#[async_trait]
pub trait ServiceFactory: Send + Sync {
    /// Task management services
    async fn create_task_control_service(
        &self,
    ) -> SchedulerResult<Box<dyn task_services::TaskControlService>>;
    async fn create_task_scheduler_service(
        &self,
    ) -> SchedulerResult<Box<dyn task_services::TaskSchedulerService>>;
    async fn create_task_dispatch_service(
        &self,
    ) -> SchedulerResult<Box<dyn task_services::TaskDispatchService>>;

    /// Worker management services
    async fn create_worker_management_service(
        &self,
    ) -> SchedulerResult<Box<dyn worker_services::WorkerManagementService>>;
    async fn create_worker_health_service(
        &self,
    ) -> SchedulerResult<Box<dyn worker_services::WorkerHealthService>>;

    /// System services
    async fn create_configuration_service(
        &self,
    ) -> SchedulerResult<Box<dyn system_services::ConfigurationService>>;
    async fn create_monitoring_service(
        &self,
    ) -> SchedulerResult<Box<dyn system_services::MonitoringService>>;
    async fn create_audit_service(&self)
        -> SchedulerResult<Box<dyn system_services::AuditService>>;
}

// Data structures for services

/// Scheduler statistics
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub total_tasks: i64,
    pub active_tasks: i64,
    pub running_task_runs: i64,
    pub pending_task_runs: i64,
    pub uptime_seconds: u64,
    pub last_schedule_time: Option<DateTime<Utc>>,
}

/// Dispatch statistics
#[derive(Debug, Clone)]
pub struct DispatchStats {
    pub total_dispatched: i64,
    pub successful_dispatched: i64,
    pub failed_dispatched: i64,
    pub redispatched: i64,
    pub avg_dispatch_time_ms: f64,
}

/// Worker load statistics
#[derive(Debug, Clone)]
pub struct WorkerLoadStats {
    pub worker_id: String,
    pub current_task_count: i32,
    pub max_concurrent_tasks: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub last_heartbeat: DateTime<Utc>,
}

/// Worker heartbeat data
#[derive(Debug, Clone)]
pub struct WorkerHeartbeat {
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub is_healthy: bool,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
    pub checked_at: DateTime<Utc>,
}

/// Worker health status
#[derive(Debug, Clone)]
pub struct WorkerHealthStatus {
    pub worker_id: String,
    pub status: HealthStatus,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
    pub metrics: WorkerHealthMetrics,
}

/// Worker health metrics
#[derive(Debug, Clone)]
pub struct WorkerHealthMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_usage_mb: u64,
    pub network_io_mb: u64,
    pub task_success_rate: f64,
    pub avg_task_execution_time_ms: f64,
}

/// Health status
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// System health
#[derive(Debug, Clone)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub components: HashMap<String, ComponentHealth>,
    pub checked_at: DateTime<Utc>,
}

/// Component health
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: DateTime<Utc>,
}

/// Time range
#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub task_throughput: f64,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
    pub resource_usage: ResourceUsage,
}

/// Resource usage
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_usage_mb: u64,
    pub network_io_mb: u64,
}

/// Alert rule
#[derive(Debug, Clone)]
pub struct AlertRule {
    pub id: String,
    pub name: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub duration_seconds: u64,
    pub enabled: bool,
}

/// Alert condition
#[derive(Debug, Clone)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
}

/// Alert
#[derive(Debug, Clone)]
pub struct Alert {
    pub id: String,
    pub rule_id: String,
    pub level: AlertLevel,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
    pub resolved: bool,
}

/// Alert level
#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

/// Audit event
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub id: String,
    pub event_type: String,
    pub user_id: Option<String>,
    pub resource_id: Option<String>,
    pub action: String,
    pub result: AuditResult,
    pub data: Value,
    pub timestamp: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Audit result
#[derive(Debug, Clone)]
pub enum AuditResult {
    Success,
    Failure,
    Error,
}

/// Audit query
#[derive(Debug, Clone)]
pub struct AuditQuery {
    pub time_range: Option<TimeRange>,
    pub event_types: Vec<String>,
    pub user_ids: Vec<String>,
    pub resource_ids: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Audit statistics
#[derive(Debug, Clone)]
pub struct AuditStats {
    pub total_events: i64,
    pub success_events: i64,
    pub failure_events: i64,
    pub error_events: i64,
    pub events_by_type: HashMap<String, i64>,
    pub events_by_user: HashMap<String, i64>,
}

/// Export format
#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Excel,
    Pdf,
}

/// Configuration watcher
#[async_trait]
pub trait ConfigWatcher: Send + Sync {
    async fn wait_for_change(&mut self) -> SchedulerResult<ConfigChange>;
    async fn stop(&mut self) -> SchedulerResult<()>;
}

/// Configuration change
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub key: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub changed_at: DateTime<Utc>,
}
