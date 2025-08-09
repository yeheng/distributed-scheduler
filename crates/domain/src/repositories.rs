//! # 领域仓储抽象模块
//!
//! 定义数据访问层的抽象接口，遵循DDD（领域驱动设计）和依赖倒置原则。
//! 仓储模式将领域逻辑与数据存储技术解耦，使得业务逻辑不依赖于具体的数据库实现。
//!
//! ## 设计原则
//!
//! ### 依赖倒置原则
//! - 高层模块（领域服务）不依赖低层模块（数据库实现）
//! - 两者都依赖于抽象（仓储接口）
//! - 抽象不依赖于细节，细节依赖于抽象
//!
//! ### 仓储模式优势
//! - **可测试性**: 可以使用Mock实现进行单元测试
//! - **可替换性**: 可以轻松切换不同的数据存储技术
//! - **关注点分离**: 领域逻辑与数据访问逻辑分离
//! - **一致性**: 统一的数据访问接口
//!
//! ## 模块结构
//!
//! ```text
//! repositories
//! ├── TaskRepository        # 任务数据仓储
//! ├── TaskRunRepository     # 任务执行实例仓储
//! └── WorkerRepository      # Worker节点仓储
//! ```
//!
//! ## 使用示例
//!
//! ### 在领域服务中使用仓储
//!
//! ```rust
//! use scheduler_domain::repositories::TaskRepository;
//! use std::sync::Arc;
//!
//! pub struct TaskService {
//!     task_repo: Arc<dyn TaskRepository>,
//! }
//!
//! impl TaskService {
//!     pub fn new(task_repo: Arc<dyn TaskRepository>) -> Self {
//!         Self { task_repo }
//!     }
//!
//!     pub async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
//!         let all_tasks = self.task_repo.find_all().await?;
//!         Ok(all_tasks.into_iter()
//!             .filter(|task| task.status == TaskStatus::Active)
//!             .collect())
//!     }
//! }
//! ```
//!
//! ### Mock实现用于测试
//!
//! ```rust
//! use mockall::mock;
//!
//! mock! {
//!     pub TaskRepo {}
//!
//!     #[async_trait]
//!     impl TaskRepository for TaskRepo {
//!         async fn create(&self, task: &Task) -> SchedulerResult<Task>;
//!         async fn find_by_id(&self, id: i64) -> SchedulerResult<Option<Task>>;
//!         // ... 其他方法
//!     }
//! }
//!
//! #[tokio::test]
//! async fn test_task_service() {
//!     let mut mock_repo = MockTaskRepo::new();
//!     mock_repo.expect_find_all()
//!         .returning(|| Ok(vec![/* 测试数据 */]));
//!
//!     let service = TaskService::new(Arc::new(mock_repo));
//!     let tasks = service.get_active_tasks().await.unwrap();
//!     // 断言测试结果
//! }
//! ```

use async_trait::async_trait;
use crate::errors::SchedulerResult;
use crate::entities::{Task, TaskRun, WorkerInfo};

// 移除 models 的引用，因为我们现在使用 entities 模块

/// 任务仓储抽象接口
///
/// 定义任务实体的数据访问操作，包括基本的CRUD操作和业务查询方法。
/// 所有任务相关的数据访问都应该通过此接口进行，以保证数据访问的一致性。
///
/// # 设计理念
///
/// - **完整性**: 提供完整的数据访问操作
/// - **业务导向**: 包含业务相关的查询方法
/// - **性能考虑**: 支持批量操作和优化查询
/// - **事务支持**: 支持事务性操作
///
/// # 实现要求
///
/// - 必须是线程安全的 (`Send + Sync`)
/// - 所有操作都是异步的
/// - 提供详细的错误信息
/// - 支持数据库事务
/// - 保证数据一致性
///
/// # 方法分类
///
/// ## 基础CRUD操作
/// - `create`: 创建新任务
/// - `find_by_id`: 根据ID查找任务
/// - `find_all`: 查找所有任务
/// - `update`: 更新任务信息
/// - `delete`: 删除任务
///
/// ## 业务查询方法
/// - `find_ready_for_execution`: 查找准备执行的任务
///
/// # 使用示例
///
/// ```rust
/// use scheduler_domain::repositories::TaskRepository;
/// use scheduler_domain::entities::{Task, TaskStatus};
/// use std::sync::Arc;
///
/// async fn manage_tasks(repo: Arc<dyn TaskRepository>) -> SchedulerResult<()> {
///     // 创建新任务
///     let mut task = Task::new("daily_backup", "shell");
///     task = repo.create(&task).await?;
///     
///     // 查找任务
///     if let Some(found_task) = repo.find_by_id(task.id).await? {
///         println!("找到任务: {}", found_task.name);
///     }
///     
///     // 更新任务状态
///     task.status = TaskStatus::Paused;
///     repo.update(&task).await?;
///     
///     // 查找准备执行的任务
///     let ready_tasks = repo.find_ready_for_execution().await?;
///     println!("准备执行的任务数量: {}", ready_tasks.len());
///     
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// 创建新任务
    ///
    /// 将新的任务实体持久化到数据存储中。任务ID会由数据库自动生成，
    /// 创建时间和更新时间会设置为当前时间。
    ///
    /// # 参数
    ///
    /// * `task` - 要创建的任务实体，ID字段会被忽略
    ///
    /// # 返回值
    ///
    /// 成功时返回包含数据库生成ID的任务实体。
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库操作失败
    /// * `InvalidTaskParams` - 任务参数验证失败
    /// * `Configuration` - 数据库连接配置错误
    ///
    /// # 业务规则
    ///
    /// - 任务名称不能为空
    /// - 任务类型必须是支持的类型
    /// - CRON表达式必须有效（如果提供）
    /// - payload必须是有效的JSON
    ///
    /// # 示例
    ///
    /// ```rust
    /// let task = Task {
    ///     id: 0, // 会被数据库生成的ID覆盖
    ///     name: "backup_database".to_string(),
    ///     task_type: "shell".to_string(),
    ///     // ... 其他字段
    /// };
    ///
    /// let created_task = repo.create(&task).await?;
    /// assert!(created_task.id > 0);
    /// ```
    async fn create(&self, task: &Task) -> SchedulerResult<Task>;

    /// 根据ID查找任务
    ///
    /// 通过任务的唯一标识符查找对应的任务实体。
    /// 这是最常用的查询方法，用于获取特定任务的详细信息。
    ///
    /// # 参数
    ///
    /// * `id` - 任务的唯一标识符
    ///
    /// # 返回值
    ///
    /// - `Some(Task)` - 找到对应的任务
    /// - `None` - 任务不存在
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库查询失败
    /// * `Serialization` - 数据反序列化失败
    ///
    /// # 性能考虑
    ///
    /// - 通过主键查询，性能最优
    /// - 建议缓存频繁访问的任务
    /// - 支持数据库连接池复用
    ///
    /// # 示例
    ///
    /// ```rust
    /// match repo.find_by_id(123).await? {
    ///     Some(task) => println!("找到任务: {}", task.name),
    ///     None => println!("任务不存在"),
    /// }
    /// ```
    async fn find_by_id(&self, id: i64) -> SchedulerResult<Option<Task>>;

    /// 查找所有任务
    ///
    /// 获取系统中所有的任务实体。此方法应该谨慎使用，
    /// 特别是在任务数量较多的系统中。
    ///
    /// # 返回值
    ///
    /// 返回包含所有任务的向量，按创建时间排序。
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库查询失败
    /// * `Serialization` - 数据反序列化失败
    ///
    /// # 性能警告
    ///
    /// - 大量任务时可能导致内存占用过高
    /// - 建议使用分页查询替代
    /// - 考虑添加过滤条件减少数据量
    ///
    /// # 使用建议
    ///
    /// - 仅在任务数量较少时使用
    /// - 考虑实现分页版本的查询方法
    /// - 可以添加状态过滤参数
    ///
    /// # 示例
    ///
    /// ```rust
    /// let all_tasks = repo.find_all().await?;
    /// println!("系统中共有 {} 个任务", all_tasks.len());
    ///
    /// // 建议添加过滤逻辑
    /// let active_tasks: Vec<_> = all_tasks.into_iter()
    ///     .filter(|task| task.status == TaskStatus::Active)
    ///     .collect();
    /// ```
    async fn find_all(&self) -> SchedulerResult<Vec<Task>>;

    /// 更新任务信息
    ///
    /// 更新现有任务的信息，包括配置、状态、调度规则等。
    /// 更新操作会自动设置updated_at字段为当前时间。
    ///
    /// # 参数
    ///
    /// * `task` - 包含更新信息的任务实体，必须包含有效的ID
    ///
    /// # 返回值
    ///
    /// 成功时返回更新后的任务实体。
    ///
    /// # 错误
    ///
    /// * `TaskNotFound` - 指定ID的任务不存在
    /// * `DatabaseOperation` - 数据库更新失败
    /// * `InvalidTaskParams` - 更新参数验证失败
    ///
    /// # 业务规则
    ///
    /// - 任务ID必须存在且有效
    /// - 不能更新为无效的状态
    /// - CRON表达式变更会重新计算下次执行时间
    /// - 状态变更会触发相应的业务逻辑
    ///
    /// # 并发控制
    ///
    /// - 支持乐观锁控制并发更新
    /// - 建议检查updated_at字段避免覆盖
    /// - 关键状态变更应该使用事务
    ///
    /// # 示例
    ///
    /// ```rust
    /// let mut task = repo.find_by_id(123).await?.unwrap();
    /// task.status = TaskStatus::Paused;
    /// task.description = Some("临时暂停维护".to_string());
    ///
    /// let updated_task = repo.update(&task).await?;
    /// assert_eq!(updated_task.status, TaskStatus::Paused);
    /// ```
    async fn update(&self, task: &Task) -> SchedulerResult<Task>;

    /// 删除任务
    ///
    /// 从系统中永久删除指定的任务。这是一个危险操作，
    /// 会同时删除任务的所有执行历史记录。
    ///
    /// # 参数
    ///
    /// * `id` - 要删除的任务ID
    ///
    /// # 返回值
    ///
    /// - `true` - 任务已成功删除
    /// - `false` - 任务不存在，无需删除
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库删除失败
    /// * `InvalidTaskParams` - 任务状态不允许删除
    ///
    /// # 业务规则
    ///
    /// - 有运行中实例的任务不能删除
    /// - 删除操作应该记录审计日志
    /// - 考虑软删除而非物理删除
    /// - 删除前应该确认用户权限
    ///
    /// # 级联删除
    ///
    /// 删除任务时会级联删除：
    /// - 所有相关的TaskRun记录
    /// - 任务的调度配置
    /// - 相关的监控和告警规则
    ///
    /// # 安全考虑
    ///
    /// ```rust
    /// // 建议的安全删除流程
    /// async fn safe_delete_task(repo: &dyn TaskRepository, id: i64) -> SchedulerResult<bool> {
    ///     // 1. 检查任务是否存在
    ///     let task = repo.find_by_id(id).await?
    ///         .ok_or(SchedulerError::TaskNotFound { id })?;
    ///     
    ///     // 2. 检查是否有运行中的实例
    ///     if has_running_instances(&task).await? {
    ///         return Err(SchedulerError::InvalidTaskParams(
    ///             "任务有运行中的实例，无法删除".to_string()
    ///         ));
    ///     }
    ///     
    ///     // 3. 执行删除
    ///     repo.delete(id).await
    /// }
    /// ```
    async fn delete(&self, id: i64) -> SchedulerResult<bool>;

    /// 查找准备执行的任务
    ///
    /// 查询所有满足执行条件的任务，供调度器使用。
    /// 这是调度系统的核心查询方法，决定了哪些任务会被执行。
    ///
    /// # 返回值
    ///
    /// 返回满足执行条件的任务列表，按优先级和执行时间排序。
    ///
    /// # 执行条件
    ///
    /// 任务必须同时满足以下条件：
    /// - 状态为Active
    /// - next_run时间已到或为空（一次性任务）
    /// - 没有超过最大重试次数
    /// - 依赖的任务已完成（如果有依赖）
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库查询失败
    /// * `Configuration` - 调度配置错误
    ///
    /// # 性能优化
    ///
    /// - 使用数据库索引优化查询
    /// - 限制返回结果数量避免过载
    /// - 缓存查询结果减少数据库压力
    ///
    /// # 调度逻辑
    ///
    /// ```sql
    /// SELECT * FROM tasks
    /// WHERE status = 'Active'
    ///   AND (next_run IS NULL OR next_run <= NOW())
    ///   AND retry_count < max_retries
    /// ORDER BY priority DESC, next_run ASC
    /// LIMIT 100;
    /// ```
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 调度器使用示例
    /// let ready_tasks = repo.find_ready_for_execution().await?;
    /// for task in ready_tasks {
    ///     println!("准备执行任务: {} (下次执行: {:?})",
    ///              task.name, task.next_run);
    ///     
    ///     // 创建执行实例并发送到队列
    ///     let task_run = create_task_run(&task).await?;
    ///     dispatch_to_queue(&task_run).await?;
    /// }
    /// ```
    async fn find_ready_for_execution(&self) -> SchedulerResult<Vec<Task>>;
}

/// 任务执行实例仓储抽象接口
///
/// 定义任务执行实例的数据访问操作，管理任务的执行历史和状态跟踪。
/// 每次任务执行都会创建一个TaskRun实例，记录完整的执行过程。
///
/// # 设计理念
///
/// - **执行追踪**: 完整记录任务执行历史
/// - **状态管理**: 精确跟踪执行状态变化
/// - **性能监控**: 支持执行时间和成功率统计
/// - **故障诊断**: 保存详细的错误信息和调试数据
///
/// # 数据特点
///
/// - **高频写入**: 执行状态频繁更新
/// - **历史查询**: 需要查询执行历史记录
/// - **实时监控**: 支持实时状态查询
/// - **数据量大**: 随时间累积大量执行记录
///
/// # 使用场景
///
/// - 任务执行状态跟踪
/// - 执行历史查询和分析
/// - 性能监控和统计
/// - 故障诊断和调试
/// - 重试机制实现
///
/// # 示例用法
///
/// ```rust
/// use scheduler_domain::repositories::TaskRunRepository;
/// use scheduler_domain::entities::{TaskRun, TaskRunStatus};
/// use std::sync::Arc;
///
/// async fn track_execution(repo: Arc<dyn TaskRunRepository>) -> SchedulerResult<()> {
///     // 创建执行实例
///     let task_run = TaskRun::new(123, "worker-001");
///     let mut run = repo.create(&task_run).await?;
///     
///     // 更新执行状态
///     run.status = TaskRunStatus::Running;
///     repo.update(&run).await?;
///     
///     // 查询任务的执行历史
///     let history = repo.find_by_task_id(123).await?;
///     println!("任务执行历史: {} 条记录", history.len());
///     
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait TaskRunRepository: Send + Sync {
    /// 创建新的任务执行实例
    ///
    /// 在任务开始执行时创建新的执行记录，记录执行的初始状态和参数。
    /// 每次任务触发都会创建一个新的TaskRun实例。
    ///
    /// # 参数
    ///
    /// * `task_run` - 要创建的执行实例，ID字段会被数据库生成
    ///
    /// # 返回值
    ///
    /// 成功时返回包含数据库生成ID的执行实例。
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库操作失败
    /// * `TaskNotFound` - 关联的任务不存在
    /// * `WorkerNotFound` - 指定的Worker不存在
    ///
    /// # 业务规则
    ///
    /// - task_id必须对应存在的任务
    /// - worker_id必须对应注册的Worker
    /// - 初始状态通常为Pending
    /// - started_at设置为当前时间
    ///
    /// # 示例
    ///
    /// ```rust
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
    ///
    /// let created_run = repo.create(&task_run).await?;
    /// assert!(created_run.id > 0);
    /// ```
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun>;

    /// 根据ID查找任务执行实例
    ///
    /// 通过执行实例的唯一标识符查找对应的记录。
    /// 用于获取特定执行实例的详细信息和当前状态。
    ///
    /// # 参数
    ///
    /// * `id` - 执行实例的唯一标识符
    ///
    /// # 返回值
    ///
    /// - `Some(TaskRun)` - 找到对应的执行实例
    /// - `None` - 执行实例不存在
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库查询失败
    /// * `Serialization` - 数据反序列化失败
    ///
    /// # 使用场景
    ///
    /// - 查询执行状态
    /// - 获取执行结果
    /// - 错误信息查看
    /// - 重试操作准备
    ///
    /// # 示例
    ///
    /// ```rust
    /// match repo.find_by_id(456).await? {
    ///     Some(run) => {
    ///         println!("执行状态: {:?}", run.status);
    ///         if let Some(error) = &run.error_message {
    ///             println!("错误信息: {}", error);
    ///         }
    ///     },
    ///     None => println!("执行实例不存在"),
    /// }
    /// ```
    async fn find_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>>;

    /// 根据任务ID查找所有执行实例
    ///
    /// 查询指定任务的所有执行历史记录，按执行时间倒序排列。
    /// 用于分析任务的执行模式、成功率和性能趋势。
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务的唯一标识符
    ///
    /// # 返回值
    ///
    /// 返回该任务的所有执行实例，按started_at时间倒序排列。
    ///
    /// # 错误
    ///
    /// * `TaskNotFound` - 任务不存在
    /// * `DatabaseOperation` - 数据库查询失败
    ///
    /// # 性能考虑
    ///
    /// - 对于执行频繁的任务，记录数量可能很大
    /// - 建议添加分页参数限制返回数量
    /// - 考虑添加时间范围过滤
    /// - 使用数据库索引优化查询性能
    ///
    /// # 分析用途
    ///
    /// ```rust
    /// let runs = repo.find_by_task_id(123).await?;
    ///
    /// // 计算成功率
    /// let total = runs.len();
    /// let successful = runs.iter()
    ///     .filter(|run| run.status == TaskRunStatus::Completed)
    ///     .count();
    /// let success_rate = successful as f64 / total as f64 * 100.0;
    ///
    /// // 计算平均执行时间
    /// let avg_duration = runs.iter()
    ///     .filter_map(|run| {
    ///         run.completed_at.map(|end|
    ///             (end - run.started_at).num_seconds()
    ///         )
    ///     })
    ///     .sum::<i64>() as f64 / successful as f64;
    ///
    /// println!("任务成功率: {:.1}%, 平均执行时间: {:.1}秒",
    ///          success_rate, avg_duration);
    /// ```
    async fn find_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>>;

    /// 更新任务执行实例
    ///
    /// 更新执行实例的状态、结果或错误信息。
    /// 这是执行过程中最频繁的操作，用于跟踪执行进度。
    ///
    /// # 参数
    ///
    /// * `task_run` - 包含更新信息的执行实例，必须包含有效的ID
    ///
    /// # 返回值
    ///
    /// 成功时返回更新后的执行实例。
    ///
    /// # 错误
    ///
    /// * `TaskRunNotFound` - 指定ID的执行实例不存在
    /// * `DatabaseOperation` - 数据库更新失败
    /// * `InvalidTaskParams` - 状态转换无效
    ///
    /// # 状态转换规则
    ///
    /// - 只能从非终态转换到终态
    /// - 终态之间不能相互转换
    /// - 状态转换必须符合业务逻辑
    ///
    /// # 并发控制
    ///
    /// - 支持乐观锁避免并发冲突
    /// - 关键状态更新使用事务保证一致性
    /// - 避免重复更新相同状态
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 开始执行
    /// let mut run = repo.find_by_id(456).await?.unwrap();
    /// run.status = TaskRunStatus::Running;
    /// repo.update(&run).await?;
    ///
    /// // 执行完成
    /// run.status = TaskRunStatus::Completed;
    /// run.completed_at = Some(Utc::now());
    /// run.result = Some(json!({"processed": 1000}));
    /// repo.update(&run).await?;
    /// ```
    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun>;

    /// 删除任务执行实例
    ///
    /// 从系统中删除指定的执行记录。通常用于清理历史数据，
    /// 但应该谨慎使用以保留审计追踪。
    ///
    /// # 参数
    ///
    /// * `id` - 要删除的执行实例ID
    ///
    /// # 返回值
    ///
    /// - `true` - 执行实例已成功删除
    /// - `false` - 执行实例不存在
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库删除失败
    /// * `InvalidTaskParams` - 运行中的实例不能删除
    ///
    /// # 删除策略
    ///
    /// - 不应删除运行中的实例
    /// - 建议批量删除过期记录
    /// - 保留最近的执行记录用于分析
    /// - 考虑归档而非直接删除
    ///
    /// # 数据清理示例
    ///
    /// ```rust
    /// // 清理30天前的已完成记录
    /// async fn cleanup_old_runs(repo: &dyn TaskRunRepository) -> SchedulerResult<usize> {
    ///     let cutoff_date = Utc::now() - Duration::days(30);
    ///     let old_runs = repo.find_completed_before(cutoff_date).await?;
    ///     
    ///     let mut deleted_count = 0;
    ///     for run in old_runs {
    ///         if repo.delete(run.id).await? {
    ///             deleted_count += 1;
    ///         }
    ///     }
    ///     
    ///     Ok(deleted_count)
    /// }
    /// ```
    async fn delete(&self, id: i64) -> SchedulerResult<bool>;

    /// 查找所有运行中的任务实例
    ///
    /// 获取当前正在执行的所有任务实例，用于监控和负载管理。
    /// 这是系统监控的重要查询方法。
    ///
    /// # 返回值
    ///
    /// 返回所有状态为Running或Pending的执行实例。
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库查询失败
    ///
    /// # 查询范围
    ///
    /// 包括以下状态的实例：
    /// - `Pending` - 等待执行
    /// - `Running` - 正在执行
    ///
    /// # 监控用途
    ///
    /// - 系统负载监控
    /// - Worker分配状态
    /// - 执行队列长度
    /// - 超时检测和处理
    ///
    /// # 性能优化
    ///
    /// - 使用数据库索引优化查询
    /// - 缓存查询结果减少数据库压力
    /// - 定期清理僵尸进程记录
    ///
    /// # 示例
    ///
    /// ```rust
    /// let running_tasks = repo.find_running().await?;
    ///
    /// println!("当前运行中的任务: {} 个", running_tasks.len());
    ///
    /// // 按Worker分组统计
    /// let mut worker_loads = HashMap::new();
    /// for run in &running_tasks {
    ///     *worker_loads.entry(&run.worker_id).or_insert(0) += 1;
    /// }
    ///
    /// for (worker_id, count) in worker_loads {
    ///     println!("Worker {}: {} 个任务", worker_id, count);
    /// }
    ///
    /// // 检查超时任务
    /// let timeout_threshold = Utc::now() - Duration::hours(1);
    /// let timeout_tasks: Vec<_> = running_tasks.into_iter()
    ///     .filter(|run| run.started_at < timeout_threshold)
    ///     .collect();
    ///
    /// if !timeout_tasks.is_empty() {
    ///     println!("发现 {} 个超时任务", timeout_tasks.len());
    /// }
    /// ```
    async fn find_running(&self) -> SchedulerResult<Vec<TaskRun>>;
}

/// Worker节点仓储抽象接口
///
/// 定义Worker节点的数据访问操作，管理分布式系统中的工作节点。
/// Worker是执行任务的计算资源，需要动态注册、状态跟踪和负载管理。
///
/// # 设计理念
///
/// - **动态管理**: 支持Worker的动态注册和注销
/// - **状态跟踪**: 实时跟踪Worker的健康状态和负载
/// - **负载均衡**: 提供负载信息支持任务分配决策
/// - **故障恢复**: 支持Worker故障检测和恢复
///
/// # Worker生命周期
///
/// ```text
/// [启动] -> 注册 -> Available -> Busy -> Available
///    |                |           |         |
///    |                v           v         v
///    |           Maintenance -> Available   |
///    |                |                     |
///    v                v                     v
/// [关闭] <- 注销 <- Offline <--------------+
/// ```
///
/// # 数据特点
///
/// - **实时性**: Worker状态需要实时更新
/// - **心跳机制**: 通过心跳检测Worker健康状态
/// - **负载信息**: 记录当前任务数和系统资源使用
/// - **能力描述**: 记录支持的任务类型和执行能力
///
/// # 使用场景
///
/// - Worker节点注册和注销
/// - 任务分配时的Worker选择
/// - 系统负载监控和分析
/// - 故障检测和自动恢复
/// - 集群扩缩容管理
///
/// # 示例用法
///
/// ```rust
/// use scheduler_domain::repositories::WorkerRepository;
/// use scheduler_domain::entities::{Worker, WorkerStatus};
/// use std::sync::Arc;
///
/// async fn manage_workers(repo: Arc<dyn WorkerRepository>) -> SchedulerResult<()> {
///     // 注册新Worker
///     let worker = Worker::new("worker-001", "192.168.1.100");
///     let registered = repo.register(&worker).await?;
///     
///     // 查找可用的Worker
///     let available_workers = repo.find_available().await?;
///     println!("可用Worker数量: {}", available_workers.len());
///     
///     // 更新Worker状态
///     let mut worker = registered;
///     worker.status = WorkerStatus::Busy;
///     worker.current_task_count = 3;
///     repo.update(&worker).await?;
///     
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait WorkerRepository: Send + Sync {
    /// 注册新的Worker节点
    ///
    /// 将新的Worker节点注册到系统中，使其可以接收和执行任务。
    /// 注册时会验证Worker的基本信息和能力配置。
    ///
    /// # 参数
    ///
    /// * `worker` - 要注册的Worker实体，包含节点信息和能力描述
    ///
    /// # 返回值
    ///
    /// 成功时返回注册后的Worker实体，包含系统分配的信息。
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库操作失败
    /// * `InvalidTaskParams` - Worker配置无效
    /// * `Configuration` - Worker ID冲突或格式错误
    ///
    /// # 注册规则
    ///
    /// - Worker ID必须全局唯一
    /// - 主机名和IP地址必须有效
    /// - 支持的任务类型不能为空
    /// - 最大并发任务数必须大于0
    ///
    /// # 初始状态
    ///
    /// - 状态设置为Available
    /// - 当前任务数设置为0
    /// - 注册时间和心跳时间设置为当前时间
    ///
    /// # 示例
    ///
    /// ```rust
    /// let worker = WorkerInfo {
    ///     id: "worker-001".to_string(),
    ///     hostname: "compute-node-1".to_string(),
    ///     ip_address: "192.168.1.100".to_string(),
    ///     status: WorkerStatus::Alive,
    ///     max_concurrent_tasks: 10,
    ///     current_task_count: 0,
    ///     supported_task_types: vec![
    ///         "python".to_string(),
    ///         "shell".to_string(),
    ///     ],
    ///     last_heartbeat: Utc::now(),
    ///     registered_at: Utc::now(),
    /// };
    ///
    /// let registered_worker = repo.register(&worker).await?;
    /// println!("Worker {} 注册成功", registered_worker.id);
    /// ```
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<WorkerInfo>;

    /// 根据ID查找Worker节点
    ///
    /// 通过Worker的唯一标识符查找对应的节点信息。
    /// 用于获取特定Worker的详细信息和当前状态。
    ///
    /// # 参数
    ///
    /// * `id` - Worker的唯一标识符
    ///
    /// # 返回值
    ///
    /// - `Some(Worker)` - 找到对应的Worker节点
    /// - `None` - Worker不存在或已注销
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库查询失败
    /// * `Serialization` - 数据反序列化失败
    ///
    /// # 使用场景
    ///
    /// - 任务分配前检查Worker状态
    /// - 心跳处理时更新Worker信息
    /// - 监控界面显示Worker详情
    /// - 故障诊断和调试
    ///
    /// # 示例
    ///
    /// ```rust
    /// match repo.find_by_id("worker-001").await? {
    ///     Some(worker) => {
    ///         println!("Worker状态: {:?}", worker.status);
    ///         println!("当前负载: {}/{}",
    ///                  worker.current_task_count,
    ///                  worker.max_concurrent_tasks);
    ///         
    ///         // 检查心跳是否正常
    ///         let heartbeat_age = Utc::now() - worker.last_heartbeat;
    ///         if heartbeat_age.num_seconds() > 60 {
    ///             println!("警告: Worker心跳超时");
    ///         }
    ///     },
    ///     None => println!("Worker不存在"),
    /// }
    /// ```
    async fn find_by_id(&self, id: &str) -> SchedulerResult<Option<WorkerInfo>>;

    /// 查找所有Worker节点
    ///
    /// 获取系统中所有已注册的Worker节点信息。
    /// 用于系统监控、负载分析和集群管理。
    ///
    /// # 返回值
    ///
    /// 返回所有Worker节点的列表，按注册时间排序。
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库查询失败
    /// * `Serialization` - 数据反序列化失败
    ///
    /// # 性能考虑
    ///
    /// - Worker数量通常不会太多，全量查询可接受
    /// - 可以考虑缓存结果减少数据库访问
    /// - 支持按状态过滤减少数据传输
    ///
    /// # 监控分析
    ///
    /// ```rust
    /// let all_workers = repo.find_all().await?;
    ///
    /// // 统计各状态Worker数量
    /// let mut status_counts = HashMap::new();
    /// for worker in &all_workers {
    ///     *status_counts.entry(&worker.status).or_insert(0) += 1;
    /// }
    ///
    /// println!("Worker状态统计:");
    /// for (status, count) in status_counts {
    ///     println!("  {:?}: {} 个", status, count);
    /// }
    ///
    /// // 计算总体负载
    /// let total_capacity: i32 = all_workers.iter()
    ///     .map(|w| w.max_concurrent_tasks)
    ///     .sum();
    /// let current_load: i32 = all_workers.iter()
    ///     .map(|w| w.current_task_count)
    ///     .sum();
    ///
    /// let load_percentage = current_load as f64 / total_capacity as f64 * 100.0;
    /// println!("集群负载: {:.1}% ({}/{})",
    ///          load_percentage, current_load, total_capacity);
    /// ```
    async fn find_all(&self) -> SchedulerResult<Vec<WorkerInfo>>;

    /// 更新Worker节点信息
    ///
    /// 更新Worker的状态、负载信息或配置参数。
    /// 这是最频繁的操作，特别是心跳更新和状态变更。
    ///
    /// # 参数
    ///
    /// * `worker` - 包含更新信息的WorkerInfo实体，必须包含有效的ID
    ///
    /// # 返回值
    ///
    /// 成功时返回更新后的WorkerInfo实体。
    ///
    /// # 错误
    ///
    /// * `WorkerNotFound` - 指定ID的Worker不存在
    /// * `DatabaseOperation` - 数据库更新失败
    /// * `InvalidTaskParams` - 更新参数无效
    ///
    /// # 更新类型
    ///
    /// ## 心跳更新
    /// - 更新last_heartbeat时间
    /// - 更新当前任务数量
    /// - 更新系统资源使用情况
    ///
    /// ## 状态变更
    /// - Available <-> Busy 状态切换
    /// - 进入/退出维护模式
    /// - 标记为离线状态
    ///
    /// ## 配置更新
    /// - 调整最大并发任务数
    /// - 更新支持的任务类型
    /// - 修改网络配置信息
    ///
    /// # 并发控制
    ///
    /// - 心跳更新使用乐观锁避免冲突
    /// - 状态变更可能需要事务保证一致性
    /// - 避免并发更新导致的数据不一致
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 心跳更新
    /// let mut worker = repo.find_by_id("worker-001").await?.unwrap();
    /// worker.last_heartbeat = Utc::now();
    /// worker.current_task_count = 5;
    /// repo.update(&worker).await?;
    ///
    /// // 状态变更
    /// worker.status = WorkerStatus::Maintenance;
    /// repo.update(&worker).await?;
    /// println!("Worker进入维护模式");
    /// ```
    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<WorkerInfo>;

    /// 注销Worker节点
    ///
    /// 从系统中移除指定的Worker节点，停止向其分配新任务。
    /// 通常在Worker正常关闭或长期离线时调用。
    ///
    /// # 参数
    ///
    /// * `id` - 要注销的Worker ID
    ///
    /// # 返回值
    ///
    /// - `true` - Worker已成功注销
    /// - `false` - Worker不存在，无需注销
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库删除失败
    /// * `InvalidTaskParams` - Worker有运行中的任务，不能注销
    ///
    /// # 注销规则
    ///
    /// - 有运行中任务的Worker不能直接注销
    /// - 应该先将Worker状态设为Offline
    /// - 等待所有任务完成后再注销
    /// - 注销操作应该记录审计日志
    ///
    /// # 优雅关闭流程
    ///
    /// ```rust
    /// async fn graceful_shutdown_worker(
    ///     repo: &dyn WorkerRepository,
    ///     worker_id: &str
    /// ) -> SchedulerResult<()> {
    ///     // 1. 获取Worker信息
    ///     let mut worker = repo.find_by_id(worker_id).await?
    ///         .ok_or(SchedulerError::WorkerNotFound {
    ///             id: worker_id.to_string()
    ///         })?;
    ///     
    ///     // 2. 设置为离线状态，停止接收新任务
    ///     worker.status = WorkerStatus::Offline;
    ///     repo.update(&worker).await?;
    ///     
    ///     // 3. 等待现有任务完成
    ///     while worker.current_task_count > 0 {
    ///         tokio::time::sleep(Duration::from_secs(5)).await;
    ///         worker = repo.find_by_id(worker_id).await?.unwrap();
    ///     }
    ///     
    ///     // 4. 注销Worker
    ///     repo.unregister(worker_id).await?;
    ///     println!("Worker {} 已优雅关闭", worker_id);
    ///     
    ///     Ok(())
    /// }
    /// ```
    async fn unregister(&self, id: &str) -> SchedulerResult<bool>;

    /// 查找可用的Worker节点
    ///
    /// 获取当前可以接收新任务的Worker节点列表。
    /// 这是任务分配时的核心查询方法。
    ///
    /// # 返回值
    ///
    /// 返回状态为Available且有剩余容量的WorkerInfo列表，
    /// 按负载率排序（负载低的优先）。
    ///
    /// # 可用条件
    ///
    /// Worker必须同时满足：
    /// - 状态为Available
    /// - current_task_count < max_concurrent_tasks
    /// - 心跳时间在合理范围内（如60秒内）
    /// - 支持所需的任务类型
    ///
    /// # 错误
    ///
    /// * `DatabaseOperation` - 数据库查询失败
    ///
    /// # 负载均衡
    ///
    /// 返回的WorkerInfo按以下规则排序：
    /// 1. 负载率低的优先（current/max比值小）
    /// 2. 剩余容量大的优先
    /// 3. 最近心跳时间新的优先
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 为Python任务选择Worker
    /// let available_workers = repo.find_available().await?;
    /// let python_workers: Vec<_> = available_workers.into_iter()
    ///     .filter(|worker| worker.supported_task_types.contains(&"python".to_string()))
    ///     .collect();
    ///
    /// if let Some(best_worker) = python_workers.first() {
    ///     println!("选择Worker: {} (负载: {}/{})",
    ///              best_worker.id,
    ///              best_worker.current_task_count,
    ///              best_worker.max_concurrent_tasks);
    /// } else {
    ///     println!("没有可用的Python Worker");
    /// }
    ///
    /// // 检查集群容量
    /// let total_available_slots: i32 = available_workers.iter()
    ///     .map(|w| w.max_concurrent_tasks - w.current_task_count)
    ///     .sum();
    ///
    /// if total_available_slots == 0 {
    ///     println!("警告: 集群容量已满，考虑扩容");
    /// }
    /// ```
    async fn find_available(&self) -> SchedulerResult<Vec<Worker>>;
}
