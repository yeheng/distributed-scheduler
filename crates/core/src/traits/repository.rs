//! 数据仓储层接口定义
//!
//! 此模块定义了数据持久化层的核心抽象接口，包括：
//! - 任务仓储接口 (TaskRepository)
//! - 任务执行实例仓储接口 (TaskRunRepository)  
//! - Worker仓储接口 (WorkerRepository)
//! - 相关的统计和数据结构
//!
//! ## 设计原则
//!
//! ### 接口隔离
//! 每个仓储接口职责单一，只负责特定实体的数据操作：
//! - `TaskRepository` - 任务定义的CRUD操作
//! - `TaskRunRepository` - 任务执行实例的生命周期管理
//! - `WorkerRepository` - Worker节点的注册和状态管理  
//!
//! ### 异步设计
//! 所有数据库操作都是异步的，支持高并发访问：
//! - 使用 `async/await` 语法
//! - 返回 `SchedulerResult<T>` 统一错误处理
//! - 实现 `Send + Sync` 确保线程安全
//!
//! ### 抽象解耦
//! 接口与具体实现分离，支持多种数据库后端：
//! - PostgreSQL 实现
//! - SQLite 实现  
//! - Redis 实现
//! - 内存实现（测试用）
//!
//! ## 使用示例
//!
//! ### 任务管理
//!
//! ```rust
//! use scheduler_core::traits::TaskRepository;
//! use scheduler_domain::entities::{Task, TaskFilter};
//!
//! async fn manage_tasks(repo: &dyn TaskRepository) -> SchedulerResult<()> {
//!     // 创建新任务
//!     let mut task = Task::new("daily_backup");
//!     task.set_schedule("0 2 * * *"); // 每天凌晨2点
//!     let created_task = repo.create(&task).await?;
//!     
//!     // 查询活跃任务
//!     let active_tasks = repo.get_active_tasks().await?;
//!     println!("活跃任务数量: {}", active_tasks.len());
//!     
//!     // 检查依赖关系
//!     let deps_satisfied = repo.check_dependencies(created_task.id).await?;
//!     if deps_satisfied {
//!         println!("任务依赖已满足，可以执行");
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### 执行实例追踪
//!
//! ```rust
//! use scheduler_core::traits::TaskRunRepository;
//! use scheduler_domain::entities::{TaskRun, TaskRunStatus};
//!
//! async fn track_execution(repo: &dyn TaskRunRepository) -> SchedulerResult<()> {
//!     // 创建执行实例
//!     let task_run = TaskRun::new(task_id, TriggerType::Scheduled);
//!     let run = repo.create(&task_run).await?;
//!     
//!     // 更新执行状态
//!     repo.update_status(run.id, TaskRunStatus::Running, Some("worker-001")).await?;
//!     
//!     // 获取执行统计
//!     let stats = repo.get_execution_stats(task_id, 30).await?;
//!     println!("30天成功率: {:.2}%", stats.success_rate);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Worker管理
//!
//! ```rust
//! use scheduler_core::traits::WorkerRepository;
//! use scheduler_domain::entities::{WorkerInfo, WorkerStatus};
//!
//! async fn manage_workers(repo: &dyn WorkerRepository) -> SchedulerResult<()> {
//!     // 注册新Worker
//!     let worker = WorkerInfo::new("worker-001", "192.168.1.100:8080");
//!     repo.register(&worker).await?;
//!     
//!     // 更新心跳
//!     repo.update_heartbeat("worker-001", Utc::now(), 5).await?;
//!     
//!     // 获取负载统计
//!     let load_stats = repo.get_worker_load_stats().await?;
//!     for stat in load_stats {
//!         println!("Worker {} 负载: {:.1}%", stat.worker_id, stat.load_percentage);
//!     }
//!     
//!     Ok(())
//! }
//! ```

use crate::models::{Task, TaskFilter, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
use crate::SchedulerResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// 任务仓储接口
///
/// 定义任务实体的数据访问和持久化操作。负责任务的生命周期管理，
/// 包括创建、查询、更新、删除，以及复杂的调度逻辑支持。
///
/// # 核心功能
///
/// 1. **基础CRUD操作** - 任务的创建、读取、更新、删除
/// 2. **查询功能** - 支持多种条件的任务查询
/// 3. **调度支持** - 获取可调度任务，检查依赖关系
/// 4. **批量操作** - 提高大量任务操作的性能
///
/// # 线程安全
///
/// 此trait要求实现 `Send + Sync`，确保可以在多线程环境中安全使用。
///
/// # 示例实现
///
/// ```rust
/// use async_trait::async_trait;
/// use crate::traits::TaskRepository;
/// 
/// pub struct SqliteTaskRepository {
///     pool: sqlx::SqlitePool,
/// }
///
/// #[async_trait]
/// impl TaskRepository for SqliteTaskRepository {
///     async fn create(&self, task: &Task) -> SchedulerResult<Task> {
///         // SQLite具体实现
///         todo!()
///     }
///     // ... 其他方法实现
/// }
/// ```
#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// 创建新任务
    ///
    /// 将任务对象持久化到数据库中，并返回包含自动生成ID的任务实例。
    /// 创建时会验证任务的有效性，确保必要字段都已设置。
    ///
    /// # 参数
    ///
    /// * `task` - 要创建的任务对象，ID字段将被忽略
    ///
    /// # 返回值
    ///
    /// 成功时返回包含数据库生成ID的任务对象。
    /// 失败时返回 `SchedulerError`。
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 任务数据验证失败
    /// * `DatabaseError` - 数据库操作失败
    /// * `ConflictError` - 任务名称已存在
    ///
    /// # 示例
    ///
    /// ```rust
    /// let mut task = Task::new("daily_backup");
    /// task.schedule = Some("0 2 * * *".to_string());
    /// task.command = "backup.sh".to_string();
    /// 
    /// let created_task = repository.create(&task).await?;
    /// assert!(created_task.id > 0);
    /// ```
    async fn create(&self, task: &Task) -> SchedulerResult<Task>;

    /// 根据ID获取任务
    ///
    /// 通过唯一标识符查找特定的任务。这是最常用的查询方法，
    /// 通常用于任务执行前的信息获取。
    ///
    /// # 参数
    ///
    /// * `id` - 任务的唯一标识符
    ///
    /// # 返回值
    ///
    /// 找到任务时返回 `Some(Task)`，未找到时返回 `None`。
    /// 数据库错误时返回 `SchedulerError`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// if let Some(task) = repository.get_by_id(123).await? {
    ///     println!("找到任务: {}", task.name);
    /// } else {
    ///     println!("任务不存在");
    /// }
    /// ```
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>>;

    /// 根据名称获取任务
    ///
    /// 通过任务名称查找任务。由于任务名称通常是唯一的，
    /// 此方法主要用于用户界面和API接口中的任务引用。
    ///
    /// # 参数
    ///
    /// * `name` - 任务名称
    ///
    /// # 返回值
    ///
    /// 找到任务时返回 `Some(Task)`，未找到时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let task = repository.get_by_name("daily_backup").await?;
    /// if let Some(t) = task {
    ///     println!("任务ID: {}", t.id);
    /// }
    /// ```
    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>>;

    /// 更新任务
    ///
    /// 更新现有任务的信息。会验证更新后的任务数据有效性，
    /// 并处理可能的并发更新冲突。
    ///
    /// # 参数
    ///
    /// * `task` - 包含更新数据的任务对象，必须包含有效的ID
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 任务数据验证失败
    /// * `NotFoundError` - 任务不存在
    /// * `ConflictError` - 并发更新冲突
    /// * `DatabaseError` - 数据库操作失败
    ///
    /// # 示例
    ///
    /// ```rust
    /// let mut task = repository.get_by_id(123).await?.unwrap();
    /// task.description = Some("更新后的描述".to_string());
    /// repository.update(&task).await?;
    /// ```
    async fn update(&self, task: &Task) -> SchedulerResult<()>;

    /// 删除任务
    ///
    /// 从数据库中删除指定的任务。通常还会删除相关的执行历史
    /// 和依赖关系数据，具体行为取决于实现。
    ///
    /// # 参数
    ///
    /// * `id` - 要删除的任务ID
    ///
    /// # 错误
    ///
    /// * `NotFoundError` - 任务不存在
    /// * `ConflictError` - 任务仍有依赖或正在执行
    /// * `DatabaseError` - 数据库操作失败
    ///
    /// # 注意
    ///
    /// 删除任务是不可逆操作，建议在删除前进行充分验证。
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 确认任务存在且没有正在运行的实例
    /// if let Some(_) = repository.get_by_id(123).await? {
    ///     repository.delete(123).await?;
    ///     println!("任务已删除");
    /// }
    /// ```
    async fn delete(&self, id: i64) -> SchedulerResult<()>;

    /// 根据过滤条件查询任务列表
    ///
    /// 使用复合条件查询任务，支持分页、排序、状态过滤等。
    /// 这是最灵活的查询方法，适用于复杂的查询需求。
    ///
    /// # 参数
    ///
    /// * `filter` - 查询过滤条件，包含各种查询参数
    ///
    /// # 返回值
    ///
    /// 返回符合条件的任务列表，空列表表示没有匹配的任务。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let filter = TaskFilter {
    ///     status: Some(TaskStatus::Active),
    ///     name_pattern: Some("backup%".to_string()),
    ///     limit: Some(10),
    ///     offset: Some(0),
    /// };
    /// 
    /// let tasks = repository.list(&filter).await?;
    /// for task in tasks {
    ///     println!("任务: {} - {}", task.id, task.name);
    /// }
    /// ```
    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>>;

    /// 获取所有活跃任务
    ///
    /// 返回当前处于活跃状态的所有任务。活跃任务是指状态为
    /// `Active` 的任务，这些任务可以被调度执行。
    ///
    /// # 返回值
    ///
    /// 活跃任务列表，按照优先级或创建时间排序。
    ///
    /// # 使用场景
    ///
    /// - 调度器启动时加载可执行任务
    /// - 系统监控和状态展示
    /// - 批量任务管理操作
    ///
    /// # 示例
    ///
    /// ```rust
    /// let active_tasks = repository.get_active_tasks().await?;
    /// println!("当前有 {} 个活跃任务", active_tasks.len());
    /// 
    /// for task in active_tasks {
    ///     if task.is_due() {
    ///         println!("任务 {} 需要执行", task.name);
    ///     }
    /// }
    /// ```
    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>>;

    /// 获取需要调度的任务（活跃且到达调度时间）
    ///
    /// 根据当前时间和任务的调度配置，返回需要立即执行的任务列表。
    /// 这是调度器的核心查询方法。
    ///
    /// # 参数
    ///
    /// * `current_time` - 当前时间，用于计算任务是否到期
    ///
    /// # 返回值
    ///
    /// 需要调度的任务列表，按照优先级排序。
    ///
    /// # 调度逻辑
    ///
    /// 任务被认为可调度需要满足：
    /// 1. 任务状态为活跃
    /// 2. 满足时间调度条件（cron表达式或间隔时间）
    /// 3. 依赖任务已完成（如果有）
    /// 4. 没有超过最大并发限制
    ///
    /// # 示例
    ///
    /// ```rust
    /// use chrono::Utc;
    /// 
    /// let now = Utc::now();
    /// let schedulable = repository.get_schedulable_tasks(now).await?;
    /// 
    /// for task in schedulable {
    ///     println!("调度任务: {} (下次执行: {:?})", task.name, task.next_run_time);
    /// }
    /// ```
    async fn get_schedulable_tasks(
        &self,
        current_time: DateTime<Utc>,
    ) -> SchedulerResult<Vec<Task>>;

    /// 检查任务依赖是否满足
    ///
    /// 验证指定任务的所有前置依赖是否都已成功完成。
    /// 这是任务执行前的重要检查步骤。
    ///
    /// # 参数
    ///
    /// * `task_id` - 要检查依赖的任务ID
    ///
    /// # 返回值
    ///
    /// 依赖满足时返回 `true`，否则返回 `false`。
    ///
    /// # 依赖检查逻辑
    ///
    /// 1. 获取任务的所有直接依赖
    /// 2. 检查每个依赖任务的最近执行状态
    /// 3. 验证执行时间是否在有效期内
    /// 4. 所有依赖都满足才返回 `true`
    ///
    /// # 示例
    ///
    /// ```rust
    /// let task_id = 123;
    /// if repository.check_dependencies(task_id).await? {
    ///     println!("任务 {} 的依赖已满足，可以执行", task_id);
    /// } else {
    ///     println!("任务 {} 的依赖未满足，等待中...", task_id);
    /// }
    /// ```
    async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool>;

    /// 获取任务的依赖列表
    ///
    /// 返回指定任务的所有前置依赖任务。用于依赖关系分析、
    /// 可视化展示以及调度决策。
    ///
    /// # 参数
    ///
    /// * `task_id` - 要获取依赖的任务ID
    ///
    /// # 返回值
    ///
    /// 依赖任务列表，按照依赖层级或优先级排序。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let dependencies = repository.get_dependencies(123).await?;
    /// 
    /// if dependencies.is_empty() {
    ///     println!("任务无依赖，可以直接执行");
    /// } else {
    ///     println!("任务依赖于以下任务:");
    ///     for dep in dependencies {
    ///         println!("  - {} ({})", dep.name, dep.id);
    ///     }
    /// }
    /// ```
    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>>;

    /// 批量更新任务状态
    ///
    /// 一次性更新多个任务的状态，提高批量操作的性能。
    /// 通常用于系统维护、批量启用/禁用任务等场景。
    ///
    /// # 参数
    ///
    /// * `task_ids` - 要更新的任务ID列表
    /// * `status` - 新的任务状态
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 状态值无效
    /// * `DatabaseError` - 数据库操作失败
    /// * `PartialFailure` - 部分任务更新失败
    ///
    /// # 事务特性
    ///
    /// 实现应该保证操作的原子性：要么全部成功，要么全部失败。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use crate::models::TaskStatus;
    /// 
    /// let task_ids = vec![1, 2, 3, 4, 5];
    /// repository.batch_update_status(&task_ids, TaskStatus::Disabled).await?;
    /// println!("批量禁用了 {} 个任务", task_ids.len());
    /// ```
    async fn batch_update_status(
        &self,
        task_ids: &[i64],
        status: crate::models::TaskStatus,
    ) -> SchedulerResult<()>;
}

/// 任务执行实例仓储接口
///
/// 管理任务执行实例的完整生命周期，从创建到完成或失败。
/// 执行实例记录了任务的具体执行情况，包括执行状态、结果、时间等。
///
/// # 核心功能
///
/// 1. **执行实例管理** - 创建、查询、更新执行实例
/// 2. **状态跟踪** - 追踪任务从调度到完成的状态变化
/// 3. **结果存储** - 保存执行结果和错误信息
/// 4. **性能分析** - 提供执行统计和性能数据
/// 5. **清理维护** - 清理过期数据，维护数据库性能
///
/// # 生命周期管理
///
/// 任务执行实例的典型生命周期：
/// 1. 创建 (Pending) - 任务被调度但未开始执行
/// 2. 分配 (Assigned) - 任务分配给特定Worker
/// 3. 运行 (Running) - 任务正在执行
/// 4. 完成 (Success/Failed) - 任务执行完成
///
/// # 线程安全
///
/// 实现必须是线程安全的，支持并发的状态更新和查询操作。
///
/// # 示例使用
///
/// ```rust
/// use scheduler_core::traits::TaskRunRepository;
/// use scheduler_domain::entities::{TaskRun, TaskRunStatus};
///
/// async fn execute_task_workflow(repo: &dyn TaskRunRepository) -> SchedulerResult<()> {
///     // 1. 创建执行实例
///     let task_run = TaskRun::new(task_id, TriggerType::Scheduled);
///     let run = repo.create(&task_run).await?;
///     
///     // 2. 更新为运行状态
///     repo.update_status(run.id, TaskRunStatus::Running, Some("worker-001")).await?;
///     
///     // 3. 更新执行结果
///     repo.update_result(run.id, Some("执行成功"), None).await?;
///     
///     // 4. 最终状态更新
///     repo.update_status(run.id, TaskRunStatus::Success, None).await?;
///     
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait TaskRunRepository: Send + Sync {
    /// 创建新的任务执行实例
    ///
    /// 当任务被调度执行时，创建对应的执行实例来跟踪执行过程。
    /// 新创建的实例通常处于 `Pending` 状态。
    ///
    /// # 参数
    ///
    /// * `task_run` - 要创建的执行实例，ID字段将被忽略
    ///
    /// # 返回值
    ///
    /// 成功时返回包含数据库生成ID的执行实例。
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 执行实例数据无效
    /// * `DatabaseError` - 数据库操作失败
    /// * `ForeignKeyError` - 关联的任务不存在
    ///
    /// # 示例
    ///
    /// ```rust
    /// let task_run = TaskRun {
    ///     task_id: 123,
    ///     trigger_type: TriggerType::Scheduled,
    ///     status: TaskRunStatus::Pending,
    ///     ..Default::default()
    /// };
    /// 
    /// let created_run = repository.create(&task_run).await?;
    /// assert!(created_run.id > 0);
    /// ```
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun>;

    /// 根据ID获取任务执行实例
    ///
    /// 通过唯一标识符查找特定的执行实例。用于检查执行状态、
    /// 获取执行结果等操作。
    ///
    /// # 参数
    ///
    /// * `id` - 执行实例的唯一标识符
    ///
    /// # 返回值
    ///
    /// 找到时返回 `Some(TaskRun)`，未找到时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// if let Some(run) = repository.get_by_id(456).await? {
    ///     println!("执行状态: {:?}", run.status);
    ///     if let Some(result) = run.result {
    ///         println!("执行结果: {}", result);
    ///     }
    /// }
    /// ```
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>>;

    /// 更新任务执行实例
    ///
    /// 更新执行实例的完整信息。通常用于更新复杂的状态变化
    /// 或批量字段更新。
    ///
    /// # 参数
    ///
    /// * `task_run` - 包含更新数据的执行实例，必须包含有效ID
    ///
    /// # 错误
    ///
    /// * `NotFoundError` - 执行实例不存在
    /// * `ValidationError` - 数据验证失败
    /// * `ConflictError` - 并发更新冲突
    ///
    /// # 示例
    ///
    /// ```rust
    /// let mut run = repository.get_by_id(456).await?.unwrap();
    /// run.status = TaskRunStatus::Success;
    /// run.end_time = Some(Utc::now());
    /// repository.update(&run).await?;
    /// ```
    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<()>;

    /// 删除任务执行实例
    ///
    /// 从数据库中删除指定的执行实例。通常用于清理过期数据
    /// 或删除错误创建的实例。
    ///
    /// # 参数
    ///
    /// * `id` - 要删除的执行实例ID
    ///
    /// # 错误
    ///
    /// * `NotFoundError` - 执行实例不存在
    /// * `ConflictError` - 执行实例正在使用中
    ///
    /// # 注意
    ///
    /// 删除正在运行的执行实例可能导致状态不一致。
    async fn delete(&self, id: i64) -> SchedulerResult<()>;

    /// 根据任务ID获取执行实例列表
    ///
    /// 获取指定任务的所有历史执行记录。用于任务执行历史分析、
    /// 故障排查等场景。
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务ID
    ///
    /// # 返回值
    ///
    /// 执行实例列表，按执行时间倒序排列。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let runs = repository.get_by_task_id(123).await?;
    /// println!("任务 123 共执行了 {} 次", runs.len());
    /// 
    /// for run in runs.iter().take(5) {
    ///     println!("执行时间: {:?}, 状态: {:?}", run.start_time, run.status);
    /// }
    /// ```
    async fn get_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>>;

    /// 根据Worker ID获取执行实例列表
    ///
    /// 获取指定Worker当前或历史执行的任务实例。用于Worker
    /// 负载监控、性能分析等。
    ///
    /// # 参数
    ///
    /// * `worker_id` - Worker标识符
    ///
    /// # 返回值
    ///
    /// 该Worker执行的任务实例列表。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let worker_runs = repository.get_by_worker_id("worker-001").await?;
    /// let running_count = worker_runs.iter()
    ///     .filter(|r| r.status == TaskRunStatus::Running)
    ///     .count();
    /// println!("Worker worker-001 当前运行 {} 个任务", running_count);
    /// ```
    async fn get_by_worker_id(&self, worker_id: &str) -> SchedulerResult<Vec<TaskRun>>;

    /// 获取指定状态的任务执行实例
    ///
    /// 根据执行状态查询实例，常用于系统监控、故障恢复等场景。
    ///
    /// # 参数
    ///
    /// * `status` - 要查询的执行状态
    ///
    /// # 返回值
    ///
    /// 指定状态的执行实例列表。
    ///
    /// # 使用场景
    ///
    /// - 查找失败的任务进行重试
    /// - 监控正在运行的任务
    /// - 统计各状态任务数量
    ///
    /// # 示例
    ///
    /// ```rust
    /// let failed_runs = repository.get_by_status(TaskRunStatus::Failed).await?;
    /// println!("发现 {} 个失败的任务执行", failed_runs.len());
    /// 
    /// for run in failed_runs {
    ///     if let Some(error) = run.error_message {
    ///         println!("任务 {} 执行失败: {}", run.task_id, error);
    ///     }
    /// }
    /// ```
    async fn get_by_status(&self, status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>>;

    /// 获取待调度的任务执行实例
    ///
    /// 获取处于 `Pending` 状态且可以分配给Worker执行的任务实例。
    /// 这是调度器分配任务的核心查询方法。
    ///
    /// # 参数
    ///
    /// * `limit` - 限制返回的实例数量，None表示不限制
    ///
    /// # 返回值
    ///
    /// 待调度的执行实例列表，按优先级和创建时间排序。
    ///
    /// # 调度策略
    ///
    /// 实现可以根据以下策略排序：
    /// - 任务优先级（高优先级优先）
    /// - 创建时间（先创建先执行）
    /// - 任务类型（特定类型优先）
    ///
    /// # 示例
    ///
    /// ```rust
    /// let pending_runs = repository.get_pending_runs(Some(10)).await?;
    /// for run in pending_runs {
    ///     println!("待执行任务: {} (优先级: {})", run.task_id, run.priority);
    /// }
    /// ```
    async fn get_pending_runs(&self, limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>>;

    /// 获取正在运行的任务执行实例
    ///
    /// 获取所有处于 `Running` 状态的执行实例。用于系统监控、
    /// 负载统计、超时检测等。
    ///
    /// # 返回值
    ///
    /// 正在运行的执行实例列表。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let running_runs = repository.get_running_runs().await?;
    /// println!("当前运行中的任务: {} 个", running_runs.len());
    /// 
    /// // 检查长时间运行的任务
    /// let now = Utc::now();
    /// for run in running_runs {
    ///     if let Some(start_time) = run.start_time {
    ///         let duration = now.signed_duration_since(start_time);
    ///         if duration.num_minutes() > 60 {
    ///             println!("任务 {} 已运行超过1小时", run.task_id);
    ///         }
    ///     }
    /// }
    /// ```
    async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>>;

    /// 获取超时的任务执行实例
    ///
    /// 查找运行时间超过指定阈值的执行实例。用于超时检测、
    /// 任务终止、系统清理等操作。
    ///
    /// # 参数
    ///
    /// * `timeout_seconds` - 超时阈值（秒）
    ///
    /// # 返回值
    ///
    /// 超时的执行实例列表。
    ///
    /// # 超时判断逻辑
    ///
    /// 任务被认为超时当：
    /// - 状态为 `Running`
    /// - 从开始时间到现在超过了指定秒数
    /// - 没有最近的心跳更新
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 查找运行超过1小时的任务
    /// let timeout_runs = repository.get_timeout_runs(3600).await?;
    /// for run in timeout_runs {
    ///     println!("任务 {} 执行超时，准备终止", run.task_id);
    ///     // 发送终止信号给Worker
    /// }
    /// ```
    async fn get_timeout_runs(&self, timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>>;

    /// 更新任务执行状态
    ///
    /// 快速更新执行实例的状态和Worker分配。这是最常用的更新操作，
    /// 性能经过优化。
    ///
    /// # 参数
    ///
    /// * `id` - 执行实例ID
    /// * `status` - 新的执行状态
    /// * `worker_id` - Worker ID，None表示取消分配
    ///
    /// # 状态转换规则
    ///
    /// 有效的状态转换：
    /// - Pending → Assigned/Running
    /// - Assigned → Running/Failed
    /// - Running → Success/Failed/Timeout
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 将任务分配给Worker
    /// repository.update_status(456, TaskRunStatus::Assigned, Some("worker-001")).await?;
    /// 
    /// // 开始执行
    /// repository.update_status(456, TaskRunStatus::Running, Some("worker-001")).await?;
    /// 
    /// // 执行完成
    /// repository.update_status(456, TaskRunStatus::Success, None).await?;
    /// ```
    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        worker_id: Option<&str>,
    ) -> SchedulerResult<()>;

    /// 更新任务执行结果
    ///
    /// 更新执行实例的结果数据和错误信息。通常在任务完成时调用。
    ///
    /// # 参数
    ///
    /// * `id` - 执行实例ID
    /// * `result` - 执行结果数据，None表示清空
    /// * `error_message` - 错误信息，None表示无错误
    ///
    /// # 结果数据格式
    ///
    /// 结果通常以JSON格式存储，包含：
    /// - 输出数据
    /// - 性能指标
    /// - 自定义元数据
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 成功执行的结果
    /// let result = serde_json::json!({
    ///     "output": "处理了1000条记录",
    ///     "processed_count": 1000,
    ///     "duration_ms": 5500
    /// });
    /// repository.update_result(456, Some(&result.to_string()), None).await?;
    /// 
    /// // 失败执行的错误
    /// repository.update_result(789, None, Some("连接数据库失败")).await?;
    /// ```
    async fn update_result(
        &self,
        id: i64,
        result: Option<&str>,
        error_message: Option<&str>,
    ) -> SchedulerResult<()>;

    /// 获取任务的最近执行记录
    ///
    /// 获取指定任务的最近几次执行记录，用于快速了解任务执行情况。
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务ID
    /// * `limit` - 返回记录数量限制
    ///
    /// # 返回值
    ///
    /// 最近的执行记录列表，按时间倒序排列。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let recent_runs = repository.get_recent_runs(123, 5).await?;
    /// println!("任务 123 最近5次执行:");
    /// 
    /// for (i, run) in recent_runs.iter().enumerate() {
    ///     println!("  {}. {:?} - {:?}", i+1, run.status, run.start_time);
    /// }
    /// ```
    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> SchedulerResult<Vec<TaskRun>>;

    /// 获取任务执行统计信息
    ///
    /// 计算指定任务在给定时间段内的执行统计数据，用于性能分析
    /// 和监控报告。
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务ID
    /// * `days` - 统计时间段（天数）
    ///
    /// # 返回值
    ///
    /// 包含详细统计信息的 `TaskExecutionStats` 结构体。
    ///
    /// # 统计指标
    ///
    /// - 总执行次数
    /// - 成功/失败/超时次数
    /// - 平均执行时间
    /// - 成功率
    /// - 最后执行时间
    ///
    /// # 示例
    ///
    /// ```rust
    /// let stats = repository.get_execution_stats(123, 30).await?;
    /// println!("任务 123 过去30天统计:");
    /// println!("  总执行次数: {}", stats.total_runs);
    /// println!("  成功率: {:.2}%", stats.success_rate);
    /// println!("  平均执行时间: {:.2}秒", 
    ///          stats.average_execution_time_ms.unwrap_or(0.0) / 1000.0);
    /// ```
    async fn get_execution_stats(
        &self,
        task_id: i64,
        days: i32,
    ) -> SchedulerResult<TaskExecutionStats>;

    /// 清理过期的任务执行记录
    ///
    /// 删除超过指定天数的旧执行记录，用于数据库维护和空间回收。
    /// 通常在系统维护期间定期执行。
    ///
    /// # 参数
    ///
    /// * `days` - 保留天数，超过此天数的记录将被删除
    ///
    /// # 返回值
    ///
    /// 返回被删除的记录数量。
    ///
    /// # 清理策略
    ///
    /// - 只删除已完成的执行记录（Success/Failed/Timeout）
    /// - 保留最近的执行记录，即使超过天数限制
    /// - 保留重要的失败记录用于故障分析
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 清理90天前的执行记录
    /// let deleted_count = repository.cleanup_old_runs(90).await?;
    /// println!("清理了 {} 条过期执行记录", deleted_count);
    /// ```
    async fn cleanup_old_runs(&self, days: i32) -> SchedulerResult<u64>;

    /// 批量更新任务执行状态
    ///
    /// 一次性更新多个执行实例的状态，提高批量操作性能。
    /// 用于系统故障恢复、批量任务管理等场景。
    ///
    /// # 参数
    ///
    /// * `run_ids` - 要更新的执行实例ID列表
    /// * `status` - 新的执行状态
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 状态值无效
    /// * `DatabaseError` - 数据库操作失败
    /// * `PartialFailure` - 部分更新失败
    ///
    /// # 事务特性
    ///
    /// 操作应该是原子的：要么全部成功，要么全部失败。
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 将所有超时的任务标记为失败
    /// let timeout_runs = repository.get_timeout_runs(3600).await?;
    /// let timeout_ids: Vec<i64> = timeout_runs.iter().map(|r| r.id).collect();
    /// 
    /// repository.batch_update_status(&timeout_ids, TaskRunStatus::Failed).await?;
    /// println!("批量标记了 {} 个超时任务为失败", timeout_ids.len());
    /// ```
    async fn batch_update_status(
        &self,
        run_ids: &[i64],
        status: TaskRunStatus,
    ) -> SchedulerResult<()>;
}

/// Worker仓储接口
///
/// 管理Worker节点的注册、状态追踪和负载监控。Worker是执行任务的
/// 计算节点，此接口提供了Worker生命周期的完整管理功能。
///
/// # 核心功能
///
/// 1. **节点管理** - Worker的注册、注销和信息更新
/// 2. **状态监控** - 心跳检测、在线状态、健康检查
/// 3. **负载均衡** - 获取Worker负载信息，支持智能调度
/// 4. **故障处理** - 超时检测、离线清理、故障恢复
/// 5. **容量规划** - 负载统计、性能分析、资源监控
///
/// # Worker生命周期
///
/// 1. **注册** - Worker启动时向系统注册
/// 2. **心跳** - 定期发送心跳保持在线状态
/// 3. **工作** - 接收并执行分配的任务
/// 4. **监控** - 系统监控Worker健康状态
/// 5. **清理** - 超时或故障时清理离线Worker
///
/// # 负载均衡支持
///
/// 接口提供了丰富的负载信息，支持多种调度策略：
/// - 最少任务优先
/// - 负载均衡分配
/// - 地理位置就近
/// - 任务类型匹配
///
/// # 示例使用
///
/// ```rust
/// use scheduler_core::traits::WorkerRepository;
/// use scheduler_domain::entities::{WorkerInfo, WorkerStatus};
///
/// async fn manage_worker_lifecycle(repo: &dyn WorkerRepository) -> SchedulerResult<()> {
///     // 1. 新Worker注册
///     let worker = WorkerInfo {
///         id: "worker-001".to_string(),
///         address: "192.168.1.100:8080".to_string(),
///         status: WorkerStatus::Online,
///         max_concurrent_tasks: 10,
///         supported_task_types: vec!["python".to_string(), "shell".to_string()],
///         ..Default::default()
///     };
///     repo.register(&worker).await?;
///     
///     // 2. 心跳更新
///     repo.update_heartbeat("worker-001", Utc::now(), 3).await?;
///     
///     // 3. 获取负载信息
///     let load_stats = repo.get_worker_load_stats().await?;
///     for stat in load_stats {
///         if stat.load_percentage > 80.0 {
///             println!("Worker {} 负载过高: {:.1}%", stat.worker_id, stat.load_percentage);
///         }
///     }
///     
///     // 4. 清理离线Worker
///     let cleaned = repo.cleanup_offline_workers(300).await?; // 5分钟超时
///     println!("清理了 {} 个离线Worker", cleaned);
///     
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait WorkerRepository: Send + Sync {
    /// 注册新的Worker
    ///
    /// 当Worker节点启动时，调用此方法向系统注册。注册成功后，
    /// Worker可以接收任务分配和参与负载均衡。
    ///
    /// # 参数
    ///
    /// * `worker` - Worker信息对象，包含节点的完整配置
    ///
    /// # 错误
    ///
    /// * `ConflictError` - Worker ID已存在
    /// * `ValidationError` - Worker信息验证失败
    /// * `DatabaseError` - 数据库操作失败
    ///
    /// # 注册要求
    ///
    /// - Worker ID必须唯一
    /// - 网络地址必须可达
    /// - 支持的任务类型不能为空
    /// - 最大并发数必须大于0
    ///
    /// # 示例
    ///
    /// ```rust
    /// let worker = WorkerInfo {
    ///     id: "worker-production-001".to_string(),
    ///     address: "10.0.1.100:8080".to_string(),
    ///     status: WorkerStatus::Online,
    ///     max_concurrent_tasks: 20,
    ///     supported_task_types: vec![
    ///         "python".to_string(), 
    ///         "shell".to_string(),
    ///         "docker".to_string()
    ///     ],
    ///     metadata: Some(serde_json::json!({
    ///         "region": "us-west-1",
    ///         "instance_type": "c5.2xlarge"
    ///     })),
    ///     ..Default::default()
    /// };
    /// 
    /// repository.register(&worker).await?;
    /// println!("Worker {} 注册成功", worker.id);
    /// ```
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()>;

    /// 注销Worker
    ///
    /// 当Worker正常关闭时，调用此方法从系统中注销。注销后的Worker
    /// 将不再接收新任务，但正在执行的任务会继续完成。
    ///
    /// # 参数
    ///
    /// * `worker_id` - 要注销的Worker ID
    ///
    /// # 错误
    ///
    /// * `NotFoundError` - Worker不存在
    /// * `ConflictError` - Worker仍有正在执行的任务
    /// * `DatabaseError` - 数据库操作失败
    ///
    /// # 注销策略
    ///
    /// - 检查是否有正在执行的任务
    /// - 等待当前任务完成或转移
    /// - 清理相关的心跳和状态信息
    /// - 更新负载均衡配置
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 优雅关闭Worker
    /// repository.update_status("worker-001", WorkerStatus::Draining).await?;
    /// 
    /// // 等待任务完成
    /// loop {
    ///     let running_tasks = repository.get_by_worker_id("worker-001").await?;
    ///     if running_tasks.is_empty() {
    ///         break;
    ///     }
    ///     tokio::time::sleep(Duration::from_secs(5)).await;
    /// }
    /// 
    /// // 正式注销
    /// repository.unregister("worker-001").await?;
    /// ```
    async fn unregister(&self, worker_id: &str) -> SchedulerResult<()>;

    /// 根据ID获取Worker信息
    ///
    /// 通过Worker ID查询详细的Worker信息，包括配置、状态、
    /// 负载等完整数据。
    ///
    /// # 参数
    ///
    /// * `worker_id` - Worker的唯一标识符
    ///
    /// # 返回值
    ///
    /// 找到时返回 `Some(WorkerInfo)`，未找到时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// if let Some(worker) = repository.get_by_id("worker-001").await? {
    ///     println!("Worker状态: {:?}", worker.status);
    ///     println!("当前任务数: {}", worker.current_task_count);
    ///     println!("最大并发: {}", worker.max_concurrent_tasks);
    ///     
    ///     let load_percent = (worker.current_task_count as f64 / 
    ///                        worker.max_concurrent_tasks as f64) * 100.0;
    ///     println!("负载: {:.1}%", load_percent);
    /// }
    /// ```
    async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;

    /// 更新Worker信息
    ///
    /// 更新Worker的配置信息，如支持的任务类型、最大并发数等。
    /// 通常在Worker重新配置或升级后调用。
    ///
    /// # 参数
    ///
    /// * `worker` - 包含更新数据的Worker信息对象
    ///
    /// # 错误
    ///
    /// * `NotFoundError` - Worker不存在
    /// * `ValidationError` - 更新数据无效
    /// * `ConflictError` - 并发更新冲突
    ///
    /// # 可更新字段
    ///
    /// - 支持的任务类型
    /// - 最大并发任务数
    /// - 网络地址（如果Worker迁移）
    /// - 元数据信息
    /// - 配置参数
    ///
    /// # 示例
    ///
    /// ```rust
    /// let mut worker = repository.get_by_id("worker-001").await?.unwrap();
    /// 
    /// // 增加支持的任务类型
    /// worker.supported_task_types.push("kubernetes".to_string());
    /// 
    /// // 提高并发能力
    /// worker.max_concurrent_tasks = 30;
    /// 
    /// // 更新元数据
    /// worker.metadata = Some(serde_json::json!({
    ///     "version": "2.1.0",
    ///     "upgraded_at": Utc::now()
    /// }));
    /// 
    /// repository.update(&worker).await?;
    /// ```
    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()>;

    /// 获取所有Worker列表
    ///
    /// 返回系统中注册的所有Worker，包括在线和离线的节点。
    /// 用于系统管理、监控面板、容量规划等场景。
    ///
    /// # 返回值
    ///
    /// 所有Worker的信息列表，按注册时间或ID排序。
    ///
    /// # 使用场景
    ///
    /// - 管理界面显示Worker列表
    /// - 系统容量和健康状态检查
    /// - 负载均衡配置更新
    /// - 运维监控和告警
    ///
    /// # 示例
    ///
    /// ```rust
    /// let all_workers = repository.list().await?;
    /// 
    /// println!("系统中共有 {} 个Worker节点:", all_workers.len());
    /// for worker in all_workers {
    ///     let status_str = match worker.status {
    ///         WorkerStatus::Online => "在线",
    ///         WorkerStatus::Offline => "离线",
    ///         WorkerStatus::Draining => "排水中",
    ///         WorkerStatus::Maintenance => "维护中",
    ///     };
    ///     println!("  {} - {} ({})", worker.id, status_str, worker.address);
    /// }
    /// ```
    async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>>;

    /// 获取活跃的Worker列表
    ///
    /// 返回当前在线且可以接收任务的Worker节点。这是任务调度时
    /// 最常用的查询方法。
    ///
    /// # 返回值
    ///
    /// 活跃Worker列表，按负载或优先级排序。
    ///
    /// # 活跃条件
    ///
    /// Worker被认为活跃需要满足：
    /// - 状态为 `Online`
    /// - 心跳在有效期内
    /// - 未超过最大并发限制
    /// - 非维护模式
    ///
    /// # 示例
    ///
    /// ```rust
    /// let alive_workers = repository.get_alive_workers().await?;
    /// 
    /// if alive_workers.is_empty() {
    ///     println!("警告: 没有可用的Worker节点");
    /// } else {
    ///     println!("可用Worker节点 {} 个:", alive_workers.len());
    ///     
    ///     // 选择负载最低的Worker
    ///     let best_worker = alive_workers.iter()
    ///         .min_by_key(|w| w.current_task_count)
    ///         .unwrap();
    ///     println!("推荐使用: {} (负载: {})", 
    ///              best_worker.id, best_worker.current_task_count);
    /// }
    /// ```
    async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;

    /// 获取支持指定任务类型的Worker列表
    ///
    /// 根据任务类型筛选可以执行该类型任务的Worker节点。
    /// 这是任务分配的核心查询方法。
    ///
    /// # 参数
    ///
    /// * `task_type` - 任务类型标识符
    ///
    /// # 返回值
    ///
    /// 支持该任务类型的Worker列表，按适配度排序。
    ///
    /// # 匹配策略
    ///
    /// - 精确匹配任务类型
    /// - 检查Worker的支持列表
    /// - 考虑Worker当前负载
    /// - 优先返回专门优化的Worker
    ///
    /// # 示例
    ///
    /// ```rust
    /// let python_workers = repository.get_workers_by_task_type("python").await?;
    /// 
    /// if python_workers.is_empty() {
    ///     return Err(SchedulerError::NoAvailableWorker(
    ///         "没有支持Python任务的Worker".to_string()
    ///     ));
    /// }
    /// 
    /// // 选择负载最轻的Worker执行Python任务
    /// let selected_worker = python_workers.iter()
    ///     .filter(|w| w.current_task_count < w.max_concurrent_tasks)
    ///     .min_by_key(|w| w.current_task_count)
    ///     .ok_or_else(|| SchedulerError::NoAvailableWorker(
    ///         "所有Python Worker都已满载".to_string()
    ///     ))?;
    /// 
    /// println!("选择Worker {} 执行Python任务", selected_worker.id);
    /// ```
    async fn get_workers_by_task_type(&self, task_type: &str) -> SchedulerResult<Vec<WorkerInfo>>;

    /// 更新Worker心跳
    ///
    /// Worker定期调用此方法报告自己的存活状态和当前负载。
    /// 心跳是Worker健康检查的重要机制。
    ///
    /// # 参数
    ///
    /// * `worker_id` - Worker ID
    /// * `heartbeat_time` - 心跳时间戳
    /// * `current_task_count` - 当前执行的任务数量
    ///
    /// # 错误
    ///
    /// * `NotFoundError` - Worker不存在
    /// * `ValidationError` - 心跳数据无效
    /// * `DatabaseError` - 数据库更新失败
    ///
    /// # 心跳频率
    ///
    /// 建议心跳间隔：
    /// - 正常情况：30-60秒
    /// - 高负载时：15-30秒
    /// - 故障恢复：5-15秒
    ///
    /// # 示例
    ///
    /// ```rust
    /// // Worker端心跳循环
    /// loop {
    ///     let current_time = Utc::now();
    ///     let task_count = get_current_task_count(); // 获取当前任务数
    ///     
    ///     match repository.update_heartbeat("worker-001", current_time, task_count).await {
    ///         Ok(_) => tracing::debug!("心跳发送成功"),
    ///         Err(e) => tracing::error!("心跳发送失败: {}", e),
    ///     }
    ///     
    ///     tokio::time::sleep(Duration::from_secs(30)).await;
    /// }
    /// ```
    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: DateTime<Utc>,
        current_task_count: i32,
    ) -> SchedulerResult<()>;

    /// 更新Worker状态
    ///
    /// 更新Worker的运行状态，如在线、离线、维护等。
    /// 状态变化会影响任务分配和负载均衡。
    ///
    /// # 参数
    ///
    /// * `worker_id` - Worker ID
    /// * `status` - 新的Worker状态
    ///
    /// # 状态转换
    ///
    /// 有效的状态转换：
    /// - Online → Offline/Draining/Maintenance
    /// - Offline → Online
    /// - Draining → Online/Offline
    /// - Maintenance → Online
    ///
    /// # 状态说明
    ///
    /// - `Online`: 正常工作，可接收新任务
    /// - `Offline`: 离线，不接收任务
    /// - `Draining`: 排水模式，完成现有任务但不接收新任务
    /// - `Maintenance`: 维护模式，暂停所有任务处理
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 进入维护模式
    /// repository.update_status("worker-001", WorkerStatus::Maintenance).await?;
    /// println!("Worker已进入维护模式");
    /// 
    /// // 维护完成，恢复在线
    /// repository.update_status("worker-001", WorkerStatus::Online).await?;
    /// println!("Worker已恢复在线");
    /// ```
    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()>;

    /// 获取超时的Worker列表
    ///
    /// 查找心跳超时的Worker节点，用于故障检测和自动恢复。
    /// 超时的Worker可能已经宕机或网络不通。
    ///
    /// # 参数
    ///
    /// * `timeout_seconds` - 心跳超时阈值（秒）
    ///
    /// # 返回值
    ///
    /// 心跳超时的Worker列表。
    ///
    /// # 超时判断
    ///
    /// Worker被认为超时当：
    /// - 最后心跳时间超过阈值
    /// - 状态为Online但无响应
    /// - 网络连接异常
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 检查5分钟内无心跳的Worker
    /// let timeout_workers = repository.get_timeout_workers(300).await?;
    /// 
    /// for worker in timeout_workers {
    ///     tracing::warn!("Worker {} 心跳超时", worker.id);
    ///     
    ///     // 尝试恢复或标记为离线
    ///     match ping_worker(&worker.address).await {
    ///         Ok(_) => {
    ///             tracing::info!("Worker {} 网络正常，可能是心跳延迟", worker.id);
    ///         }
    ///         Err(_) => {
    ///             repository.update_status(&worker.id, WorkerStatus::Offline).await?;
    ///             tracing::error!("Worker {} 已离线", worker.id);
    ///         }
    ///     }
    /// }
    /// ```
    async fn get_timeout_workers(&self, timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>>;

    /// 清理离线Worker
    ///
    /// 删除长时间离线的Worker记录，释放数据库空间并清理
    /// 过期的配置信息。
    ///
    /// # 参数
    ///
    /// * `timeout_seconds` - 离线时间阈值（秒）
    ///
    /// # 返回值
    ///
    /// 返回被清理的Worker数量。
    ///
    /// # 清理策略
    ///
    /// - 只清理长时间离线的Worker
    /// - 保留最近的离线记录用于故障分析
    /// - 清理相关的任务分配记录
    /// - 更新负载均衡配置
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 清理1小时以上离线的Worker
    /// let cleaned_count = repository.cleanup_offline_workers(3600).await?;
    /// 
    /// if cleaned_count > 0 {
    ///     tracing::info!("清理了 {} 个长期离线的Worker", cleaned_count);
    ///     
    ///     // 重新计算集群容量
    ///     let alive_workers = repository.get_alive_workers().await?;
    ///     let total_capacity: i32 = alive_workers.iter()
    ///         .map(|w| w.max_concurrent_tasks)
    ///         .sum();
    ///         
    ///     tracing::info!("当前集群总容量: {} 个并发任务", total_capacity);
    /// }
    /// ```
    async fn cleanup_offline_workers(&self, timeout_seconds: i64) -> SchedulerResult<u64>;

    /// 获取Worker负载统计
    ///
    /// 获取所有Worker的详细负载统计信息，用于监控、报告和
    /// 容量规划。
    ///
    /// # 返回值
    ///
    /// Worker负载统计列表，包含详细的性能指标。
    ///
    /// # 统计指标
    ///
    /// - 当前任务数和最大并发数
    /// - 负载百分比
    /// - 历史完成任务数
    /// - 平均任务执行时间
    /// - 最后心跳时间
    ///
    /// # 示例
    ///
    /// ```rust
    /// let load_stats = repository.get_worker_load_stats().await?;
    /// 
    /// println!("=== Worker负载报告 ===");
    /// for stat in load_stats {
    ///     println!("Worker: {}", stat.worker_id);
    ///     println!("  负载: {:.1}% ({}/{})", 
    ///              stat.load_percentage,
    ///              stat.current_task_count, 
    ///              stat.max_concurrent_tasks);
    ///     println!("  完成任务: {} 成功, {} 失败", 
    ///              stat.total_completed_tasks,
    ///              stat.total_failed_tasks);
    ///     
    ///     if let Some(avg_duration) = stat.average_task_duration_ms {
    ///         println!("  平均执行时间: {:.2}秒", avg_duration / 1000.0);
    ///     }
    ///     
    ///     let heart_age = Utc::now().signed_duration_since(stat.last_heartbeat);
    ///     println!("  心跳: {}秒前", heart_age.num_seconds());
    ///     println!();
    /// }
    /// ```
    async fn get_worker_load_stats(&self) -> SchedulerResult<Vec<WorkerLoadStats>>;

    /// 批量更新Worker状态
    ///
    /// 一次性更新多个Worker的状态，用于批量运维操作。
    ///
    /// # 参数
    ///
    /// * `worker_ids` - 要更新的Worker ID列表
    /// * `status` - 新的Worker状态
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 状态值无效
    /// * `DatabaseError` - 数据库操作失败
    /// * `PartialFailure` - 部分Worker更新失败
    ///
    /// # 使用场景
    ///
    /// - 批量维护操作
    /// - 区域性故障恢复
    /// - 集群升级部署
    /// - 紧急状态处理
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 批量进入维护模式
    /// let maintenance_workers = vec![
    ///     "worker-001".to_string(),
    ///     "worker-002".to_string(),
    ///     "worker-003".to_string(),
    /// ];
    /// 
    /// repository.batch_update_status(&maintenance_workers, WorkerStatus::Maintenance).await?;
    /// println!("批量设置 {} 个Worker进入维护模式", maintenance_workers.len());
    /// 
    /// // 维护完成后批量恢复
    /// repository.batch_update_status(&maintenance_workers, WorkerStatus::Online).await?;
    /// println!("批量恢复 {} 个Worker到在线状态", maintenance_workers.len());
    /// ```
    async fn batch_update_status(
        &self,
        worker_ids: &[String],
        status: WorkerStatus,
    ) -> SchedulerResult<()>;
}

/// 任务执行统计信息
///
/// 包含任务在指定时间段内的详细执行统计数据，用于性能分析、
/// 监控报告和系统优化决策。
///
/// # 字段说明
///
/// * `task_id` - 任务的唯一标识符
/// * `total_runs` - 总执行次数
/// * `successful_runs` - 成功执行次数
/// * `failed_runs` - 失败执行次数
/// * `timeout_runs` - 超时执行次数
/// * `average_execution_time_ms` - 平均执行时间（毫秒），None表示无数据
/// * `success_rate` - 成功率（百分比，0-100）
/// * `last_execution` - 最后一次执行时间，None表示从未执行
///
/// # 使用场景
///
/// - 任务性能监控和报警
/// - 系统容量规划和优化
/// - SLA指标计算和报告
/// - 故障分析和趋势预测
///
/// # 示例
///
/// ```rust
/// use scheduler_core::traits::TaskExecutionStats;
/// use chrono::Utc;
///
/// let stats = TaskExecutionStats {
///     task_id: 123,
///     total_runs: 100,
///     successful_runs: 95,
///     failed_runs: 4,
///     timeout_runs: 1,
///     average_execution_time_ms: Some(2500.0),
///     success_rate: 95.0,
///     last_execution: Some(Utc::now()),
/// };
///
/// println!("任务 {} 统计:", stats.task_id);
/// println!("  成功率: {:.1}%", stats.success_rate);
/// println!("  平均执行时间: {:.2}秒", 
///          stats.average_execution_time_ms.unwrap_or(0.0) / 1000.0);
/// ```
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskExecutionStats {
    /// 任务ID
    pub task_id: i64,
    /// 总执行次数
    pub total_runs: i64,
    /// 成功执行次数
    pub successful_runs: i64,
    /// 失败执行次数
    pub failed_runs: i64,
    /// 超时执行次数
    pub timeout_runs: i64,
    /// 平均执行时间（毫秒）
    pub average_execution_time_ms: Option<f64>,
    /// 成功率（百分比）
    pub success_rate: f64,
    /// 最后执行时间
    pub last_execution: Option<DateTime<Utc>>,
}

/// Worker负载统计信息
///
/// 包含Worker节点的详细负载和性能统计数据，用于负载均衡、
/// 容量规划和系统监控。
///
/// # 字段说明
///
/// * `worker_id` - Worker的唯一标识符
/// * `current_task_count` - 当前执行的任务数量
/// * `max_concurrent_tasks` - 最大并发任务限制
/// * `load_percentage` - 负载百分比（0-100）
/// * `total_completed_tasks` - 历史完成任务总数
/// * `total_failed_tasks` - 历史失败任务总数
/// * `average_task_duration_ms` - 平均任务执行时间（毫秒）
/// * `last_heartbeat` - 最后心跳时间
///
/// # 负载计算
///
/// 负载百分比 = (当前任务数 / 最大并发数) × 100
///
/// # 使用场景
///
/// - 任务调度时的Worker选择
/// - 集群负载监控和告警
/// - Worker性能分析和优化
/// - 容量规划和扩缩容决策
///
/// # 示例
///
/// ```rust
/// use scheduler_core::traits::WorkerLoadStats;
/// use chrono::Utc;
///
/// let stats = WorkerLoadStats {
///     worker_id: "worker-001".to_string(),
///     current_task_count: 8,
///     max_concurrent_tasks: 10,
///     load_percentage: 80.0,
///     total_completed_tasks: 1500,
///     total_failed_tasks: 25,
///     average_task_duration_ms: Some(3200.0),
///     last_heartbeat: Utc::now(),
/// };
///
/// println!("Worker {} 状态:", stats.worker_id);
/// println!("  负载: {:.1}% ({}/{})", 
///          stats.load_percentage,
///          stats.current_task_count, 
///          stats.max_concurrent_tasks);
/// 
/// let success_rate = (stats.total_completed_tasks as f64 / 
///                    (stats.total_completed_tasks + stats.total_failed_tasks) as f64) * 100.0;
/// println!("  历史成功率: {:.1}%", success_rate);
/// ```
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerLoadStats {
    /// Worker ID
    pub worker_id: String,
    /// 当前任务数
    pub current_task_count: i32,
    /// 最大并发任务数
    pub max_concurrent_tasks: i32,
    /// 负载百分比
    pub load_percentage: f64,
    /// 总完成任务数
    pub total_completed_tasks: i64,
    /// 总失败任务数
    pub total_failed_tasks: i64,
    /// 平均任务执行时间（毫秒）
    pub average_task_duration_ms: Option<f64>,
    /// 最后心跳时间
    pub last_heartbeat: DateTime<Utc>,
}

impl TaskExecutionStats {
    /// 计算成功率
    ///
    /// 根据成功执行次数和总执行次数计算成功率百分比。
    /// 如果总执行次数为0，则成功率为0。
    ///
    /// # 计算公式
    ///
    /// 成功率 = (成功次数 / 总次数) × 100
    ///
    /// # 示例
    ///
    /// ```rust
    /// let mut stats = TaskExecutionStats {
    ///     task_id: 123,
    ///     total_runs: 100,
    ///     successful_runs: 95,
    ///     failed_runs: 5,
    ///     timeout_runs: 0,
    ///     success_rate: 0.0, // 待计算
    ///     ..Default::default()
    /// };
    ///
    /// stats.calculate_success_rate();
    /// assert_eq!(stats.success_rate, 95.0);
    /// ```
    pub fn calculate_success_rate(&mut self) {
        if self.total_runs > 0 {
            self.success_rate = (self.successful_runs as f64 / self.total_runs as f64) * 100.0;
        } else {
            self.success_rate = 0.0;
        }
    }
}

impl WorkerLoadStats {
    /// 计算负载百分比
    ///
    /// 根据当前任务数和最大并发数计算负载百分比。
    /// 如果最大并发数为0，则负载为0。
    ///
    /// # 计算公式
    ///
    /// 负载百分比 = (当前任务数 / 最大并发数) × 100
    ///
    /// # 示例
    ///
    /// ```rust
    /// let mut stats = WorkerLoadStats {
    ///     worker_id: "worker-001".to_string(),
    ///     current_task_count: 8,
    ///     max_concurrent_tasks: 10,
    ///     load_percentage: 0.0, // 待计算
    ///     ..Default::default()
    /// };
    ///
    /// stats.calculate_load_percentage();
    /// assert_eq!(stats.load_percentage, 80.0);
    /// ```
    pub fn calculate_load_percentage(&mut self) {
        if self.max_concurrent_tasks > 0 {
            self.load_percentage =
                (self.current_task_count as f64 / self.max_concurrent_tasks as f64) * 100.0;
        } else {
            self.load_percentage = 0.0;
        }
    }
}
