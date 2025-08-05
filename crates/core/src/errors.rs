use thiserror::Error;

/// 调度器统一错误类型定义
///
/// 定义了调度系统中所有可能出现的错误类型，提供统一的错误处理机制。
/// 每种错误都包含详细的上下文信息，便于问题诊断和处理。
///
/// # 设计原则
///
/// - **统一性**: 所有模块使用相同的错误类型
/// - **详细性**: 每个错误都包含足够的上下文信息
/// - **可恢复性**: 区分可恢复和不可恢复的错误
/// - **用户友好**: 提供清晰的错误描述
///
/// # 错误分类
///
/// ## 数据层错误
/// - [`Database`] - 数据库连接和操作错误
/// - [`DatabaseOperation`] - 特定数据库操作错误
///
/// ## 业务逻辑错误
/// - [`TaskNotFound`] - 任务不存在
/// - [`TaskRunNotFound`] - 任务运行实例不存在
/// - [`WorkerNotFound`] - Worker节点不存在
/// - [`CircularDependency`] - 任务依赖循环
///
/// ## 配置和验证错误
/// - [`InvalidCron`] - CRON表达式格式错误
/// - [`Configuration`] - 配置参数错误
/// - [`InvalidTaskParams`] - 任务参数验证失败
///
/// ## 运行时错误
/// - [`ExecutionTimeout`] - 任务执行超时
/// - [`LeadershipLost`] - 集群领导权丢失
/// - [`TaskExecution`] - 任务执行过程错误
///
/// ## 基础设施错误
/// - [`MessageQueue`] - 消息队列操作错误
/// - [`Network`] - 网络通信错误
/// - [`Serialization`] - 数据序列化错误
///
/// # 使用示例
///
/// ## 基本错误处理
///
/// ```rust
/// use scheduler_core::{SchedulerResult, SchedulerError};
///
/// async fn get_task(id: i64) -> SchedulerResult<Task> {
///     let task = database.find_task(id).await
///         .map_err(|e| SchedulerError::Database(e))?;
///     
///     match task {
///         Some(task) => Ok(task),
///         None => Err(SchedulerError::TaskNotFound { id }),
///     }
/// }
/// ```
///
/// ## 错误链式传播
///
/// ```rust
/// async fn process_task(task_id: i64) -> SchedulerResult<()> {
///     let task = get_task(task_id).await?; // 自动传播错误
///     
///     if task.status != TaskStatus::Active {
///         return Err(SchedulerError::InvalidTaskParams(
///             "任务状态必须为Active".to_string()
///         ));
///     }
///     
///     execute_task(&task).await
///         .map_err(|e| SchedulerError::TaskExecution(e.to_string()))?;
///     
///     Ok(())
/// }
/// ```
///
/// ## 错误恢复策略
///
/// ```rust
/// async fn robust_task_execution(task_id: i64) -> SchedulerResult<()> {
///     match execute_task(task_id).await {
///         Ok(()) => Ok(()),
///         Err(SchedulerError::ExecutionTimeout) => {
///             // 超时错误可以重试
///             warn!("任务执行超时，准备重试");
///             retry_task(task_id).await
///         },
///         Err(SchedulerError::TaskNotFound { id }) => {
///             // 任务不存在是致命错误
///             error!("任务不存在: {}", id);
///             Err(SchedulerError::TaskNotFound { id })
///         },
///         Err(e) => Err(e), // 其他错误直接传播
///     }
/// }
/// ```
///
/// # 错误处理最佳实践
///
/// ## 1. 使用 `?` 操作符传播错误
/// ```rust
/// let result = operation().await?; // 推荐
/// // 而不是 operation().await.unwrap(); // 不推荐
/// ```
///
/// ## 2. 提供有意义的错误上下文
/// ```rust
/// database.save(&task).await
///     .map_err(|e| SchedulerError::DatabaseOperation(
///         format!("保存任务失败 task_id={}: {}", task.id, e)
///     ))?;
/// ```
///
/// ## 3. 区分不同的错误场景
/// ```rust
/// match validate_cron_expression(&schedule) {
///     Ok(_) => {},
///     Err(e) => return Err(SchedulerError::InvalidCron {
///         expr: schedule.clone(),
///         message: e.to_string(),
///     }),
/// }
/// ```
///
/// # 线程安全
///
/// 所有错误类型都实现了 `Send + Sync`，可以在多线程环境中安全传递。
///
/// # 序列化支持
///
/// 错误类型支持序列化，可以通过网络传输或持久化存储：
///
/// ```rust
/// let error = SchedulerError::TaskNotFound { id: 123 };
/// let json = serde_json::to_string(&error)?;
/// ```
#[derive(Debug, Error)]
pub enum SchedulerError {
    /// 数据库连接或操作错误
    ///
    /// 当数据库连接失败、SQL执行错误或事务回滚时触发。
    /// 这是一个可恢复的错误，通常可以通过重试解决。
    ///
    /// # 常见原因
    /// - 数据库连接超时
    /// - SQL语法错误
    /// - 约束违反
    /// - 事务死锁
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    /// 特定数据库操作错误
    ///
    /// 用于包装具体的数据库操作错误，提供更详细的上下文信息。
    /// 通常包含操作类型、表名、参数等详细信息。
    ///
    /// # 使用场景
    /// - 批量操作失败
    /// - 数据迁移错误
    /// - 索引操作失败
    #[error("数据库操作错误: {0}")]
    DatabaseOperation(String),

    /// 指定ID的任务不存在
    ///
    /// 当尝试访问不存在的任务时触发。这通常是客户端错误，
    /// 表示请求的资源不存在。
    ///
    /// # 处理建议
    /// - 检查任务ID是否正确
    /// - 确认任务是否已被删除
    /// - 返回404状态码给客户端
    #[error("任务未找到: {id}")]
    TaskNotFound { id: i64 },

    /// 指定ID的任务运行实例不存在
    ///
    /// 当尝试访问不存在的任务运行实例时触发。
    /// 可能是任务运行已完成并被清理，或ID错误。
    ///
    /// # 处理建议
    /// - 检查任务运行ID是否正确
    /// - 确认任务运行是否已过期清理
    /// - 查询任务运行历史记录
    #[error("任务运行实例未找到: {id}")]
    TaskRunNotFound { id: i64 },

    /// 指定ID的Worker节点不存在
    ///
    /// 当尝试访问不存在的Worker节点时触发。
    /// 可能是Worker已下线或ID错误。
    ///
    /// # 处理建议
    /// - 检查Worker ID是否正确
    /// - 确认Worker是否在线
    /// - 更新Worker注册信息
    #[error("Worker未找到: {id}")]
    WorkerNotFound { id: String },

    /// CRON表达式格式错误
    ///
    /// 当解析CRON表达式失败时触发。包含原始表达式和详细的错误信息，
    /// 便于用户修正表达式格式。
    ///
    /// # 常见错误
    /// - 字段数量不正确
    /// - 字段值超出范围
    /// - 特殊字符使用错误
    ///
    /// # 示例
    /// ```text
    /// 无效的CRON表达式: "0 25 * * *" - 小时字段值超出范围(0-23)
    /// ```
    #[error("无效的CRON表达式: {expr} - {message}")]
    InvalidCron { expr: String, message: String },

    /// 任务执行超时
    ///
    /// 当任务执行时间超过配置的超时限制时触发。
    /// 这是一个可恢复的错误，可以通过调整超时时间或优化任务逻辑解决。
    ///
    /// # 处理策略
    /// - 增加任务超时时间
    /// - 优化任务执行逻辑
    /// - 分解大任务为小任务
    /// - 实施任务重试机制
    #[error("任务执行超时")]
    ExecutionTimeout,

    /// 任务依赖循环检测
    ///
    /// 当任务依赖关系形成循环时触发。这是一个严重的配置错误，
    /// 会导致任务无法正常调度执行。
    ///
    /// # 检测逻辑
    /// - 使用深度优先搜索检测循环
    /// - 在任务创建和更新时验证
    /// - 提供循环路径信息
    ///
    /// # 解决方法
    /// - 重新设计任务依赖关系
    /// - 移除不必要的依赖
    /// - 使用条件依赖替代强依赖
    #[error("检测到循环依赖")]
    CircularDependency,

    /// 集群领导权丢失
    ///
    /// 在分布式环境中，当当前节点失去领导权时触发。
    /// 这通常发生在网络分区或节点故障时。
    ///
    /// # 处理策略
    /// - 停止当前调度操作
    /// - 等待重新选举
    /// - 清理未完成的操作
    /// - 重新同步状态
    #[error("失去领导权")]
    LeadershipLost,

    /// 消息队列操作错误
    ///
    /// 当消息队列连接、发送或接收消息失败时触发。
    /// 包含详细的错误信息，便于诊断网络或配置问题。
    ///
    /// # 常见原因
    /// - 消息队列服务不可用
    /// - 网络连接问题
    /// - 认证失败
    /// - 队列不存在
    #[error("消息队列错误: {0}")]
    MessageQueue(String),

    /// 数据序列化/反序列化错误
    ///
    /// 当JSON、TOML或其他格式的数据序列化失败时触发。
    /// 通常是数据格式不匹配或字段缺失导致。
    ///
    /// # 常见场景
    /// - API请求体格式错误
    /// - 配置文件格式错误
    /// - 消息队列数据格式错误
    #[error("序列化错误: {0}")]
    Serialization(String),

    /// 配置参数错误
    ///
    /// 当系统配置参数无效或缺失时触发。
    /// 包含具体的配置项和错误原因。
    ///
    /// # 验证内容
    /// - 必需配置项是否存在
    /// - 配置值是否在有效范围内
    /// - 配置项之间的依赖关系
    /// - 配置格式是否正确
    #[error("配置错误: {0}")]
    Configuration(String),

    /// 任务执行过程错误
    ///
    /// 当任务在执行过程中发生错误时触发。
    /// 包含任务执行器返回的详细错误信息。
    ///
    /// # 错误来源
    /// - 任务脚本执行失败
    /// - 资源不足
    /// - 权限问题
    /// - 依赖服务不可用
    #[error("任务执行错误: {0}")]
    TaskExecution(String),

    /// 网络通信错误
    ///
    /// 当网络请求失败时触发，包括HTTP请求、TCP连接等。
    /// 提供详细的网络错误信息和重试建议。
    ///
    /// # 常见原因
    /// - 网络连接超时
    /// - DNS解析失败
    /// - 服务端不可达
    /// - SSL/TLS握手失败
    #[error("网络错误: {0}")]
    Network(String),

    /// 系统内部错误
    ///
    /// 当系统内部发生不可预期的错误时触发。
    /// 这通常表示程序逻辑错误或系统资源问题。
    ///
    /// # 处理建议
    /// - 记录详细的错误日志
    /// - 通知系统管理员
    /// - 考虑系统重启
    /// - 检查系统资源使用情况
    #[error("内部错误: {0}")]
    Internal(String),

    /// 任务参数验证失败
    ///
    /// 当任务参数不符合预期格式或约束时触发。
    /// 包含具体的验证失败原因和修正建议。
    ///
    /// # 验证规则
    /// - 参数类型检查
    /// - 参数值范围验证
    /// - 必需参数存在性检查
    /// - 参数格式验证
    #[error("无效的任务参数: {0}")]
    InvalidTaskParams(String),
}
