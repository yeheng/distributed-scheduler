use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// 活跃状态 - 任务可以被正常调度执行
    Active,

    /// 暂停状态 - 任务暂时停止调度，可以恢复
    Paused,

    /// 禁用状态 - 任务完全停止，需要重新配置
    Disabled,
}

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