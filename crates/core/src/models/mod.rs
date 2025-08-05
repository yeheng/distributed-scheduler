//! # 数据模型
//!
//! 定义分布式任务调度系统的核心数据结构，包括任务、Worker、消息队列等。
//!
//! ## 概述
//!
//! 此模块提供系统中的所有核心数据模型，采用结构化设计确保数据一致性和类型安全。
//! 所有模型都实现了序列化和反序列化，支持数据库存储和网络传输。
//!
//! ## 核心模型
//!
//! ### Task - 任务定义
//! 表示一个可执行的任务单元，包含任务的基本信息、执行配置、调度策略等。
//!
//! ### TaskRun - 任务运行实例
//! 表示任务的一次具体执行，跟踪执行状态、结果和性能指标。
//!
//! ### WorkerInfo - Worker节点信息
//! 表示系统中的Worker节点，包含节点能力、状态和负载信息。
//!
//! ### Message - 消息队列通信
//! 表示系统间通信的消息格式，支持任务分发、状态更新、心跳等场景。
//!
//! ## 设计原则
//!
//! ### 数据一致性
//! - 所有时间字段使用 `DateTime<Utc>` 确保时区一致性
//! - 状态字段使用枚举类型，避免无效状态
//! - 必填字段和可选字段明确区分
//!
//! ### 序列化支持
//! - 实现 `serde::Serialize` 和 `serde::Deserialize`
//! - 支持 JSON 和 TOML 格式
//! - 数据库字段映射优化
//!
//! ### 验证和约束
//! - 字段长度限制
//! - 枚举值有效性检查
//! - 关联数据完整性验证
//!
//! ## 使用示例
//!
//! ```rust
//! use scheduler_core::models::*;
//! use chrono::Utc;
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
//!     schedule: Some("0 2 * * *".to_string()),
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
//!
//! // 创建Worker
//! let worker = WorkerInfo {
//!     id: "worker-001".to_string(),
//!     hostname: "server-01".to_string(),
//!     ip_address: "192.168.1.100".to_string(),
//!     port: 8080,
//!     task_types: vec!["python".to_string(), "shell".to_string()],
//!     max_concurrent_tasks: 5,
//!     current_task_count: 2,
//!     status: WorkerStatus::Active,
//!     last_heartbeat: Utc::now(),
//!     registered_at: Utc::now(),
//!     metadata: None,
//! };
//!
//! // 创建消息
//! let message = Message {
//!     id: uuid::Uuid::new_v4().to_string(),
//!     message_type: MessageType::TaskExecution,
//!     correlation_id: Some("task-123".to_string()),
//!     payload: serde_json::json!({"task_id": 1, "action": "start"}),
//!     timestamp: Utc::now(),
//!     retry_count: 0,
//!     expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
//! };
//! ```
//!
//! ## 状态管理
//!
//! ### 任务状态流转
//! ```text
//! Created → Active → Paused → Completed
//!            ↓        ↓
//!          Failed   Cancelled
//! ```
//!
//! ### Worker状态流转
//! ```text
//! Registering → Active → Busy → Idle → Draining → Unregistered
//!                       ↓       ↓
//!                     Failed   Offline
//! ```
//!
//! ## 数据库映射
//!
//! 所有模型都设计为与数据库表结构对应：
//!
//! ### tasks 表
//! - `id` - 主键
//! - `name` - 任务名称
//! - `status` - 任务状态
//! - `schedule` - 调度表达式
//! - `created_at` - 创建时间
//! - `updated_at` - 更新时间
//!
//! ### task_runs 表
//! - `id` - 主键
//! - `task_id` - 关联任务ID
//! - `status` - 运行状态
//! - `started_at` - 开始时间
//! - `completed_at` - 完成时间
//!
//! ### workers 表
//! - `id` - Worker标识
//! - `hostname` - 主机名
//! - `status` - Worker状态
//! - `last_heartbeat` - 最后心跳时间
//!
//! ## 性能考虑
//!
//! ### 内存优化
//! - 使用 `String` 而非 `&str` 避免生命周期问题
//! - 可选字段使用 `Option<T>` 减少内存占用
//! - 大字段（如metadata）使用JSON字符串存储
//!
//! ### 查询优化
//! - 为常用查询字段建立索引
//! - 批量操作减少数据库往返
//! - 使用连接池管理数据库连接
//!
//! ## 扩展性
//!
//! ### 自定义字段
//! 所有模型都包含 `metadata` 字段，支持存储自定义属性：
//!
//! ```rust
//! let task = Task {
//!     // ... 其他字段
//!     metadata: Some(serde_json::json!({
//!         "custom_field": "custom_value",
//!         "priority_score": 95
//!     })),
//! };
//! ```
//!
//! ### 标签系统
//! 支持通过标签对任务进行分类和筛选：
//!
//! ```rust
//! let task = Task {
//!     // ... 其他字段
//!     tags: Some(vec!["critical".to_string(), "production".to_string()]),
//! };
//! ```
//!
//! ## 版本兼容性
//!
//! 模型设计考虑了向后兼容性：
//! - 新增字段使用 `Option<T>` 类型
//! - 删除字段标记为废弃但不立即移除
//! - 重大变更通过版本号管理
//!
//! ## 相关模块
//!
//! - [`crate::traits`] - 服务接口定义
//! - [`crate::services`] - 服务实现
//! - [`crate::config`] - 配置模型
//!
//! ## 测试
//!
//! 模块提供完整的单元测试：
//!
//! ```rust
//! #[tokio::test]
//! async fn test_task_creation() {
//!     let task = create_test_task();
//!     assert_eq!(task.status, TaskStatus::Active);
//!     assert!(task.tags.is_some());
//! }
//! ```

pub mod message;
pub mod task;
pub mod task_run;
pub mod worker;

pub use message::*;
pub use task::*;
pub use task_run::*;
pub use worker::*;
