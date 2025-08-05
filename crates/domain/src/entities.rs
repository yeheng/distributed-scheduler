//! 领域实体
//!
//! 核心业务实体定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 任务领域实体
///
/// 表示调度系统中的核心业务对象 - 任务。包含任务的完整定义、
/// 调度配置、状态信息和执行参数。
///
/// # 领域模型设计
///
/// 任务实体遵循DDD（领域驱动设计）原则，封装了任务的：
/// - **身份标识**: 唯一ID和名称
/// - **业务属性**: 类型、描述、负载数据
/// - **调度配置**: CRON表达式、重试策略、超时设置
/// - **状态管理**: 当前状态、时间戳、执行计划
///
/// # 生命周期状态
///
/// ```text
/// [创建] -> Active -> [调度执行]
///    |       |
///    |       v
///    |    Paused -> [恢复] -> Active
///    |       |
///    v       v
/// Disabled <- [禁用]
/// ```
///
/// # 字段说明
///
/// ## 标识字段
/// - `id`: 数据库主键，全局唯一
/// - `name`: 业务名称，用户可读的任务标识
///
/// ## 业务字段
/// - `description`: 任务功能描述，可选
/// - `task_type`: 任务类型，用于执行器匹配
/// - `payload`: 任务执行参数，JSON格式
///
/// ## 调度字段
/// - `cron_expression`: CRON调度表达式，可选
/// - `next_run`: 下次执行时间，由调度器计算
/// - `retry_count`: 当前重试次数
/// - `max_retries`: 最大重试次数
/// - `timeout_seconds`: 执行超时时间（秒）
///
/// ## 状态字段
/// - `status`: 当前任务状态
/// - `created_at`: 创建时间
/// - `updated_at`: 最后更新时间
///
/// # 使用示例
///
/// ## 创建定时任务
///
/// ```rust
/// use scheduler_domain::entities::{Task, TaskStatus};
/// use chrono::Utc;
/// use serde_json::json;
///
/// let task = Task {
///     id: 0, // 由数据库生成
///     name: "daily_report".to_string(),
///     description: Some("生成每日报表".to_string()),
///     task_type: "python".to_string(),
///     payload: json!({
///         "script": "generate_report.py",
///         "output_dir": "/reports",
///         "format": "pdf"
///     }),
///     cron_expression: Some("0 2 * * *".to_string()), // 每天凌晨2点
///     status: TaskStatus::Active,
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
///     next_run: None, // 由调度器计算
///     retry_count: 0,
///     max_retries: 3,
///     timeout_seconds: Some(3600), // 1小时超时
/// };
/// ```
///
/// ## 创建一次性任务
///
/// ```rust
/// let one_time_task = Task {
///     id: 0,
///     name: "data_migration".to_string(),
///     description: Some("数据迁移任务".to_string()),
///     task_type: "shell".to_string(),
///     payload: json!({
///         "command": "migrate_data.sh",
///         "source": "/old_data",
///         "target": "/new_data"
///     }),
///     cron_expression: None, // 一次性任务
///     status: TaskStatus::Active,
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
///     next_run: Some(Utc::now()), // 立即执行
///     retry_count: 0,
///     max_retries: 1,
///     timeout_seconds: Some(7200), // 2小时超时
/// };
/// ```
///
/// # 业务规则
///
/// ## 状态转换规则
/// - 只有Active状态的任务才能被调度执行
/// - Paused状态的任务可以恢复为Active
/// - Disabled状态的任务不能直接恢复，需要重新配置
///
/// ## 调度规则
/// - 有CRON表达式的任务按时间调度
/// - 无CRON表达式的任务为一次性任务
/// - next_run为空的任务不会被调度
///
/// ## 重试规则
/// - retry_count不能超过max_retries
/// - 重试间隔由调度器配置决定
/// - 超过最大重试次数的任务标记为失败
///
/// # 数据一致性
///
/// - 所有时间字段使用UTC时区
/// - payload必须是有效的JSON
/// - cron_expression必须符合标准CRON格式
/// - 状态变更必须记录updated_at时间
///
/// # 序列化支持
///
/// 实体支持JSON序列化，用于：
/// - API接口数据传输
/// - 消息队列传递
/// - 配置文件存储
/// - 日志记录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    /// 任务唯一标识符
    ///
    /// 数据库主键，全局唯一。新创建的任务ID为0，
    /// 由数据库自动生成实际ID。
    pub id: i64,

    /// 任务名称
    ///
    /// 用户可读的任务标识，应该简洁明了地描述任务功能。
    /// 建议使用下划线分隔的小写字母，如 "daily_report"。
    pub name: String,

    /// 任务描述
    ///
    /// 详细描述任务的功能、用途和注意事项。
    /// 可选字段，但建议填写以提高可维护性。
    pub description: Option<String>,

    /// 任务类型
    ///
    /// 用于执行器匹配的任务类型标识。
    /// 常见类型：python, shell, docker, http, sql等。
    pub task_type: String,

    /// 任务执行参数
    ///
    /// JSON格式的任务参数，包含执行所需的所有配置信息。
    /// 具体结构由任务类型和执行器定义。
    pub payload: serde_json::Value,

    /// CRON调度表达式
    ///
    /// 标准的CRON表达式，定义任务的执行时间规律。
    /// 格式：秒 分 时 日 月 周 [年]
    /// 为None时表示一次性任务。
    pub cron_expression: Option<String>,

    /// 任务当前状态
    ///
    /// 控制任务是否可以被调度执行。
    /// 只有Active状态的任务才会被调度器处理。
    pub status: TaskStatus,

    /// 任务创建时间
    ///
    /// 记录任务首次创建的UTC时间，不可变更。
    pub created_at: DateTime<Utc>,

    /// 任务最后更新时间
    ///
    /// 记录任务最后一次修改的UTC时间，
    /// 每次更新任务信息时都应该更新此字段。
    pub updated_at: DateTime<Utc>,

    /// 下次执行时间
    ///
    /// 由调度器根据CRON表达式计算的下次执行时间。
    /// 为None时表示任务不会被自动调度。
    pub next_run: Option<DateTime<Utc>>,

    /// 当前重试次数
    ///
    /// 记录任务执行失败后的重试次数。
    /// 每次重试后递增，成功执行后重置为0。
    pub retry_count: i32,

    /// 最大重试次数
    ///
    /// 任务执行失败时的最大重试次数。
    /// 超过此次数后任务将被标记为最终失败。
    pub max_retries: i32,

    /// 执行超时时间（秒）
    ///
    /// 任务执行的最大允许时间。
    /// 为None时使用系统默认超时时间。
    /// 超时后任务会被强制终止。
    pub timeout_seconds: Option<i32>,
}

/// 任务状态枚举
///
/// 定义任务在其生命周期中的各种状态，控制任务的调度行为。
/// 状态转换遵循特定的业务规则，确保系统的一致性和可预测性。
///
/// # 状态说明
///
/// ## Active（活跃）
/// - 任务可以被调度器正常调度执行
/// - 新创建的任务默认状态
/// - 可以转换为Paused或Disabled状态
///
/// ## Paused（暂停）
/// - 任务暂时停止调度，但保留配置
/// - 可以恢复为Active状态继续执行
/// - 已运行的实例不受影响，会继续执行
///
/// ## Disabled（禁用）
/// - 任务完全停止，不会被调度执行
/// - 通常用于废弃或有问题的任务
/// - 需要重新配置才能恢复使用
///
/// # 状态转换图
///
/// ```text
///     [创建]
///        |
///        v
///    ┌─────────┐    暂停     ┌─────────┐
///    │ Active  │ ---------> │ Paused  │
///    │         │ <--------- │         │
///    └─────────┘    恢复     └─────────┘
///        |                      |
///        |        禁用          |
///        v                      v
///    ┌─────────┐ <----------┌─────────┐
///    │Disabled │            │Disabled │
///    └─────────┘            └─────────┘
/// ```
///
/// # 使用示例
///
/// ```rust
/// use scheduler_domain::entities::TaskStatus;
///
/// // 检查任务是否可以执行
/// fn can_execute(status: &TaskStatus) -> bool {
///     matches!(status, TaskStatus::Active)
/// }
///
/// // 状态转换
/// let mut status = TaskStatus::Active;
/// status = TaskStatus::Paused; // 暂停任务
/// status = TaskStatus::Active; // 恢复任务
/// status = TaskStatus::Disabled; // 禁用任务
/// ```
///
/// # 业务规则
///
/// - 只有Active状态的任务才会被调度器处理
/// - Paused状态的任务可以随时恢复为Active
/// - Disabled状态的任务需要管理员干预才能恢复
/// - 状态变更应该记录操作日志和时间戳
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// 活跃状态 - 任务可以被正常调度执行
    Active,
    
    /// 暂停状态 - 任务暂时停止调度，可以恢复
    Paused,
    
    /// 禁用状态 - 任务完全停止，需要重新配置
    Disabled,
}

/// 任务执行实例实体
///
/// 表示任务的单次执行记录，包含执行过程的完整信息。
/// 每次任务被触发执行时都会创建一个新的TaskRun实例。
///
/// # 设计理念
///
/// TaskRun遵循"一次执行一个实例"的原则：
/// - **可追溯性**: 记录完整的执行历史
/// - **状态管理**: 精确跟踪执行状态变化
/// - **错误处理**: 详细记录失败原因和重试信息
/// - **结果存储**: 保存执行结果供后续分析
///
/// # 生命周期
///
/// ```text
/// [创建] -> Pending -> Running -> Completed
///    |         |         |         ^
///    |         |         v         |
///    |         |      Failed ------+
///    |         |         |
///    |         v         v
///    +---> Cancelled <---+
/// ```
///
/// # 字段详解
///
/// ## 标识字段
/// - `id`: 执行实例的唯一标识
/// - `task_id`: 关联的任务ID
/// - `worker_id`: 执行此实例的Worker节点ID
///
/// ## 状态字段
/// - `status`: 当前执行状态
/// - `started_at`: 开始执行时间
/// - `completed_at`: 完成时间（成功或失败）
///
/// ## 结果字段
/// - `error_message`: 错误信息（失败时）
/// - `result`: 执行结果数据（成功时）
/// - `retry_count`: 重试次数
///
/// # 使用示例
///
/// ## 创建执行实例
///
/// ```rust
/// use scheduler_domain::entities::{TaskRun, TaskRunStatus};
/// use chrono::Utc;
///
/// let task_run = TaskRun {
///     id: 0, // 由数据库生成
///     task_id: 123,
///     worker_id: "worker-001".to_string(),
///     status: TaskRunStatus::Pending,
///     started_at: Utc::now(),
///     completed_at: None,
///     error_message: None,
///     result: None,
///     retry_count: 0,
/// };
/// ```
///
/// ## 更新执行状态
///
/// ```rust
/// use serde_json::json;
///
/// // 开始执行
/// let mut task_run = task_run;
/// task_run.status = TaskRunStatus::Running;
///
/// // 执行成功
/// task_run.status = TaskRunStatus::Completed;
/// task_run.completed_at = Some(Utc::now());
/// task_run.result = Some(json!({
///     "processed_records": 1000,
///     "output_file": "/reports/daily_2024.pdf"
/// }));
/// ```
///
/// ## 处理执行失败
///
/// ```rust
/// // 执行失败
/// task_run.status = TaskRunStatus::Failed;
/// task_run.completed_at = Some(Utc::now());
/// task_run.error_message = Some("文件权限不足".to_string());
/// task_run.retry_count += 1;
/// ```
///
/// # 业务规则
///
/// ## 状态转换规则
/// - Pending -> Running: 开始执行
/// - Running -> Completed: 执行成功
/// - Running -> Failed: 执行失败
/// - Pending/Running -> Cancelled: 被取消
///
/// ## 时间规则
/// - started_at在创建时设置
/// - completed_at在终态时设置（Completed/Failed/Cancelled）
/// - 执行时间 = completed_at - started_at
///
/// ## 重试规则
/// - retry_count记录当前重试次数
/// - 每次重试创建新的TaskRun实例
/// - 重试次数不能超过任务的max_retries设置
///
/// # 数据完整性
///
/// - task_id必须对应存在的Task
/// - worker_id必须对应注册的Worker
/// - 状态变更必须符合状态机规则
/// - 时间字段使用UTC时区保证一致性
///
/// # 监控和分析
///
/// TaskRun数据用于：
/// - 任务执行历史查询
/// - 性能分析和优化
/// - 错误统计和告警
/// - Worker负载分析
/// - 系统容量规划
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskRun {
    /// 执行实例唯一标识符
    ///
    /// 数据库主键，全局唯一。每次任务执行都会生成新的ID。
    pub id: i64,

    /// 关联的任务ID
    ///
    /// 指向执行此实例的Task实体。用于查询任务的执行历史
    /// 和统计信息。
    pub task_id: i64,

    /// 执行Worker的ID
    ///
    /// 标识执行此任务实例的Worker节点。用于负载分析
    /// 和故障排查。
    pub worker_id: String,

    /// 当前执行状态
    ///
    /// 跟踪任务实例的执行进度，支持状态机转换。
    pub status: TaskRunStatus,

    /// 开始执行时间
    ///
    /// 记录任务实例开始执行的UTC时间。在创建实例时设置，
    /// 用于计算执行时长和性能分析。
    pub started_at: DateTime<Utc>,

    /// 完成时间
    ///
    /// 记录任务实例完成的UTC时间（成功、失败或取消）。
    /// 为None表示任务仍在执行中。
    pub completed_at: Option<DateTime<Utc>>,

    /// 错误信息
    ///
    /// 当任务执行失败时记录详细的错误信息。
    /// 包含异常堆栈、错误代码等调试信息。
    pub error_message: Option<String>,

    /// 执行结果
    ///
    /// 任务成功执行后的结果数据，JSON格式。
    /// 具体结构由任务类型和执行器定义。
    pub result: Option<serde_json::Value>,

    /// 重试次数
    ///
    /// 记录此任务实例是第几次重试执行。
    /// 首次执行为0，每次重试递增1。
    pub retry_count: i32,
}

/// 任务执行状态枚举
///
/// 定义任务执行实例在其生命周期中的各种状态。
/// 状态转换遵循严格的状态机规则，确保执行流程的可预测性。
///
/// # 状态详解
///
/// ## Pending（等待中）
/// - 任务实例已创建，等待Worker执行
/// - 初始状态，由调度器创建
/// - 可以转换为Running或Cancelled
///
/// ## Running（运行中）
/// - 任务正在Worker上执行
/// - 从Pending状态转换而来
/// - 可以转换为Completed、Failed或Cancelled
///
/// ## Completed（已完成）
/// - 任务执行成功完成
/// - 终态，不能再转换为其他状态
/// - 应该包含执行结果数据
///
/// ## Failed（执行失败）
/// - 任务执行过程中发生错误
/// - 终态，但可能触发重试机制
/// - 应该包含详细的错误信息
///
/// ## Cancelled（已取消）
/// - 任务被用户或系统主动取消
/// - 终态，不能再转换为其他状态
/// - 可能在任何非终态时被取消
///
/// # 状态转换图
///
/// ```text
///     [创建]
///        |
///        v
///   ┌─────────┐     开始执行    ┌─────────┐
///   │ Pending │ ------------> │ Running │
///   └─────────┘               └─────────┘
///        |                         |
///        |                         |
///        |          ┌─────────────┬┴─────────────┐
///        |          │             │              │
///        |          v             v              v
///        |    ┌───────────┐ ┌─────────┐ ┌─────────────┐
///        |    │ Completed │ │ Failed  │ │ Cancelled   │
///        |    └───────────┘ └─────────┘ └─────────────┘
///        |                      ^              ^
///        |                      |              |
///        └──────────────────────┴──────────────┘
///                          取消
/// ```
///
/// # 使用示例
///
/// ```rust
/// use scheduler_domain::entities::TaskRunStatus;
///
/// // 检查任务是否处于终态
/// fn is_terminal_state(status: &TaskRunStatus) -> bool {
///     matches!(status, 
///         TaskRunStatus::Completed | 
///         TaskRunStatus::Failed | 
///         TaskRunStatus::Cancelled
///     )
/// }
///
/// // 检查任务是否可以取消
/// fn can_cancel(status: &TaskRunStatus) -> bool {
///     matches!(status, 
///         TaskRunStatus::Pending | 
///         TaskRunStatus::Running
///     )
/// }
///
/// // 状态转换示例
/// let mut status = TaskRunStatus::Pending;
/// status = TaskRunStatus::Running;  // 开始执行
/// status = TaskRunStatus::Completed; // 执行成功
/// ```
///
/// # 业务规则
///
/// ## 状态转换规则
/// - 只能从非终态转换到终态
/// - 终态之间不能相互转换
/// - 每次状态变更都应该记录时间戳
///
/// ## 监控规则
/// - Pending状态超时应该告警
/// - Running状态超时应该强制取消
/// - Failed状态应该触发重试或告警
///
/// ## 统计规则
/// - 用于计算任务成功率
/// - 用于分析执行时长分布
/// - 用于监控系统健康状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskRunStatus {
    /// 等待执行状态 - 任务实例已创建，等待Worker处理
    Pending,
    
    /// 运行中状态 - 任务正在Worker上执行
    Running,
    
    /// 完成状态 - 任务执行成功完成
    Completed,
    
    /// 失败状态 - 任务执行过程中发生错误
    Failed,
    
    /// 取消状态 - 任务被主动取消执行
    Cancelled,
}

/// Worker实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Worker {
    pub id: String,
    pub hostname: String,
    pub ip_address: String,
    pub port: Option<u16>,
    pub status: WorkerStatus,
    pub max_concurrent_tasks: i32,
    pub current_task_count: i32,
    pub supported_task_types: Vec<String>,
    pub last_heartbeat: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Worker状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkerStatus {
    Available,
    Busy,
    Offline,
    Maintenance,
}
