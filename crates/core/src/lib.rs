//! # Scheduler Core
//!
//! 分布式任务调度系统的核心模块，提供统一的数据模型、服务接口和基础设施。
//!
//! ## 概述
//!
//! 此模块是整个调度系统的基础，定义了：
//! - 核心数据结构（任务、Worker、消息等）
//! - 统一的错误处理机制
//! - 服务接口抽象
//! - 配置管理系统
//! - 日志和监控基础设施
//!
//! ## 架构设计
//!
//! ```
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Scheduler Core                            │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Models  │  Services  │  Traits  │  Config  │  Logging  │
//! │  (数据模型) │ (服务实现)  │ (接口定义) │ (配置管理) │ (日志系统) │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## 核心组件
//!
//! ### 数据模型
//! - [`Task`] - 任务定义和元数据
//! - [`TaskRun`] - 任务执行实例
//! - [`WorkerInfo`] - Worker节点信息
//! - [`Message`] - 消息队列通信格式
//!
//! ### 服务接口
//! - [`TaskControlService`] - 任务控制服务
//! - [`WorkerServiceTrait`] - Worker管理服务
//! - [`MessageQueue`] - 消息队列接口
//! - [`TaskExecutor`] - 任务执行器接口
//!
//! ### 基础设施
//! - [`ServiceLocator`] - 服务定位器
//! - [`CircuitBreaker`] - 熔断器
//! - [`StructuredLogger`] - 结构化日志
//!
//! ## 使用示例
//!
//! ### 基本任务创建
//!
//! ```rust
//! use scheduler_core::prelude::*;
//! use scheduler_core::models::{Task, TaskStatus, TaskPriority};
//! use chrono::{DateTime, Utc};
//!
//! // 创建任务
//! let task = Task {
//!     id: 1,
//!     name: "data_processing".to_string(),
//!     description: "处理每日数据".to_string(),
//!     task_type: "batch".to_string(),
//!     executor: "python".to_string(),
//!     command: "process_data.py".to_string(),
//!     arguments: Some(vec!["--input".to_string(), "data.csv".to_string()]),
//!     schedule: Some("0 2 * * *".to_string()), // 每天凌晨2点执行
//!     priority: TaskPriority::Normal,
//!     status: TaskStatus::Active,
//!     retry_count: 3,
//!     timeout_seconds: 3600,
//!     created_at: Utc::now(),
//!     updated_at: Utc::now(),
//!     last_run_at: None,
//!     next_run_at: None,
//!     dependencies: None,
//!     tags: Some(vec!["data".to_string(), "daily".to_string()]),
//!     metadata: None,
//! };
//! ```
//!
//! ### 服务使用
//!
//! ```rust
//! use scheduler_core::prelude::*;
//! use std::sync::Arc;
//!
//! #[async_trait::async_trait]
//! impl TaskControlService for MyTaskService {
//!     async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
//!         // 实现任务触发逻辑
//!         // 1. 验证任务状态
//!         // 2. 创建任务运行实例
//!         // 3. 发送到消息队列
//!         // 4. 返回运行实例
//!         Ok(task_run)
//!     }
//! }
//! ```
//!
//! ### 错误处理
//!
//! ```rust
//! use scheduler_core::{SchedulerResult, SchedulerError};
//!
//! async fn process_task(task_id: i64) -> SchedulerResult<()> {
//!     let task = task_repository.get_by_id(task_id).await
//!         .map_err(|e| SchedulerError::DatabaseError(format!("查询任务失败: {}", e)))?;
//!     
//!     if task.status != TaskStatus::Active {
//!         return Err(SchedulerError::ValidationError("任务未激活".to_string()));
//!     }
//!     
//!     // 处理任务...
//!     Ok(())
//! }
//! ```
//!
//! ## 配置管理
//!
//! ```rust
//! use scheduler_core::config::{AppConfig, DatabaseConfig, MessageQueueConfig};
//!
//! // 从配置文件加载
//! let config = AppConfig::from_file("config.toml").await?;
//!
//! // 或使用构建器创建
//! let config = AppConfig::builder()
//!     .database(DatabaseConfig::postgres("localhost", 5432, "scheduler"))
//!     .message_queue(MessageQueueConfig::rabbitmq("localhost", 5672))
//!     .build()?;
//! ```
//!
//! ## 日志和监控
//!
//! ```rust
//! use scheduler_core::logging::{StructuredLogger, LogLevel};
//!
//! // 初始化日志
//! let logger = StructuredLogger::new(LogLevel::Info)
//!     .with_service_name("scheduler-core")
//!     .enable_json_format(true)
//!     .init()?;
//!
//! // 记录结构化日志
//! info!(task_id = %task_id, worker_id = %worker_id, "任务分配成功");
//! error!(error = %e, "任务执行失败");
//! ```
//!
//! ## 模块组织
//!
//! ```
//! scheduler_core/
//! ├── models/          # 数据模型定义
//! │   ├── mod.rs
//! │   ├── task.rs
//! │   ├── task_run.rs
//! │   └── worker.rs
//! ├── traits/          # 服务接口定义
//! │   ├── mod.rs
//! │   ├── task_executor.rs
//! │   └── message_queue.rs
//! ├── services/        # 服务实现
//! │   ├── mod.rs
//! │   └── task_services.rs
//! ├── config/          # 配置管理
//! │   ├── mod.rs
//! │   └── app_config.rs
//! ├── logging/         # 日志系统
//! │   ├── mod.rs
//! │   └── structured_logger.rs
//! └── lib.rs           # 公开API
//! ```
//!
//! ## 设计原则
//!
//! ### SOLID 原则
//! - **单一职责原则**: 每个模块职责明确
//! - **开闭原则**: 通过trait扩展而非修改
//! - **里氏替换原则**: 所有trait实现可互换
//! - **接口隔离原则**: 接口细粒度，职责单一
//! - **依赖倒置原则**: 依赖抽象而非具体实现
//!
//! ### 性能考虑
//! - 使用 `Arc<T>` 共享数据，避免克隆
//! - 异步操作使用 `tokio` 运行时
//! - 数据库查询优化和连接池
//! - 消息队列批量操作
//!
//! ### 错误处理
//! - 统一的 `SchedulerError` 错误类型
//! - 详细的错误上下文信息
//! - 错误传播和恢复机制
//!
//! ## 线程安全
//!
//! 所有核心数据结构都实现了 `Send + Sync` trait，可以在多线程环境中安全使用。
//! 共享状态使用适当的同步原语保护：
//! - `Arc<Mutex<T>>` - 互斥访问
//! - `Arc<RwLock<T>>` - 读写分离
//! - `Arc<AtomicU64>` - 原子操作
//!
//! ## 扩展性
//!
//! 系统通过trait机制提供良好的扩展性：
//! - 可以添加新的任务执行器类型
//! - 可以支持不同的消息队列实现
//! - 可以自定义数据存储后端
//! - 可以扩展监控和日志功能
//!
//! ## 测试
//!
//! 模块提供完整的单元测试和集成测试：
//!
//! ```bash
//! # 运行所有测试
//! cargo test
//!
//! # 运行特定模块测试
//! cargo test models::tests
//!
//! # 运行集成测试
//! cargo test --test integration_tests
//! ```
//!
//! ## 版本兼容性
//!
//! - 主要版本：破坏性API变更
//! - 次要版本：向后兼容的功能添加
//! - 补丁版本：bug修复和内部改进
//!
//! ## 相关模块
//!
//! - [`scheduler_api`] - REST API服务
//! - [`scheduler_dispatcher`] - 任务调度器
//! - [`scheduler_worker`] - Worker节点
//! - [`scheduler_infrastructure`] - 基础设施实现
//!
//! [`scheduler_api`]: ../api/index.html
//! [`scheduler_dispatcher`]: ../dispatcher/index.html
//! [`scheduler_worker`]: ../worker/index.html
//! [`scheduler_infrastructure`]: ../infrastructure/index.html

pub mod circuit_breaker;
pub mod config;
pub mod container;
pub mod error_handling;
pub mod errors;
pub mod executor_registry;
pub mod logging;
pub mod models;
pub mod service_interfaces;
pub mod services;
pub mod traits;

// Core types and utilities
pub use models::{Message, Task, TaskResult, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
pub use errors::SchedulerError;
pub use container::ServiceLocator;
pub type SchedulerResult<T> = std::result::Result<T, SchedulerError>;

// Core modules with focused re-exports
pub mod prelude {
    pub use crate::circuit_breaker::*;
    pub use crate::config::*;
    pub use crate::container::*;
    pub use crate::error_handling::*;
    pub use crate::executor_registry::*;
    pub use crate::logging::*;
}

// Service interfaces organized by domain  
pub mod api {
    pub use crate::services::{
        TaskControlService, WorkerHealthService, AlertManagementService,
        ConfigurationService, HealthCheckService, PerformanceMonitoringService,
    };
}

// Core traits for extensibility
pub mod ext {
    pub use crate::traits::{
        ExecutorRegistry, ExecutorStatus, MessageQueue, ResourceLimits, 
        TaskExecutionContextTrait, TaskExecutor, WorkerServiceTrait,
    };
}

// Backward compatibility re-exports
pub use crate::ext::*;
