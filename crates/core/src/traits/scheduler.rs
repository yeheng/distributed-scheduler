//! 调度系统服务接口定义
//!
//! 此模块定义了任务调度系统的核心服务接口，包括：
//! - 任务调度服务 (TaskSchedulerService)
//! - 状态监听服务 (StateListenerService)
//! - 任务控制服务 (TaskControlService)
//! - 任务分派策略 (TaskDispatchStrategy)
//! - Worker服务接口 (WorkerServiceTrait)
//!
//! ## 架构设计
//!
//! ### 服务分层
//! 调度系统采用分层架构，每个接口负责特定的功能领域：
//! - **调度层** - `TaskSchedulerService` 负责任务扫描和调度决策
//! - **控制层** - `TaskControlService` 提供任务生命周期管理
//! - **监听层** - `StateListenerService` 处理状态变化和事件
//! - **执行层** - `WorkerServiceTrait` 实际执行任务
//! - **策略层** - `TaskDispatchStrategy` 提供可插拔的调度策略
//!
//! ### 异步协作
//! 所有接口都是异步的，支持高并发处理：
//! - 使用 `async_trait` 实现异步trait
//! - 返回 `SchedulerResult<T>` 统一错误处理
//! - 支持 `Send + Sync` 确保线程安全
//!
//! ## 使用示例
//!
//! ### 完整调度流程
//!
//! ```rust
//! use scheduler_core::traits::{
//!     TaskSchedulerService, TaskControlService, 
//!     StateListenerService, WorkerServiceTrait
//! };
//!
//! async fn run_scheduler_system(
//!     scheduler: &dyn TaskSchedulerService,
//!     controller: &dyn TaskControlService,
//!     listener: &dyn StateListenerService,
//!     worker: &dyn WorkerServiceTrait,
//! ) -> SchedulerResult<()> {
//!     // 1. 启动状态监听
//!     let listener_handle = tokio::spawn(async move {
//!         listener.listen_for_updates().await
//!     });
//!     
//!     // 2. 启动Worker服务
//!     worker.start().await?;
//!     
//!     // 3. 调度循环
//!     loop {
//!         // 扫描并调度任务
//!         let scheduled_tasks = scheduler.scan_and_schedule().await?;
//!         
//!         for task_run in scheduled_tasks {
//!             // 分发任务到队列
//!             scheduler.dispatch_to_queue(&task_run).await?;
//!         }
//!         
//!         // Worker轮询并执行任务
//!         worker.poll_and_execute_tasks().await?;
//!         
//!         // 发送心跳
//!         worker.send_heartbeat().await?;
//!         
//!         tokio::time::sleep(Duration::from_secs(10)).await;
//!     }
//! }
//! ```
//!
//! ### 自定义调度策略
//!
//! ```rust
//! use async_trait::async_trait;
//! use scheduler_core::traits::TaskDispatchStrategy;
//!
//! pub struct LoadBalancedStrategy;
//!
//! #[async_trait]
//! impl TaskDispatchStrategy for LoadBalancedStrategy {
//!     async fn select_worker(
//!         &self,
//!         task: &Task,
//!         available_workers: &[WorkerInfo],
//!     ) -> SchedulerResult<Option<String>> {
//!         // 选择负载最低的Worker
//!         let best_worker = available_workers
//!             .iter()
//!             .filter(|w| w.supports_task_type(&task.task_type))
//!             .min_by_key(|w| w.current_task_count);
//!             
//!         Ok(best_worker.map(|w| w.id.clone()))
//!     }
//!
//!     fn name(&self) -> &str {
//!         "load-balanced"
//!     }
//! }
//! ```

use async_trait::async_trait;

use crate::{
    models::{Task, TaskRun, TaskStatusUpdate, WorkerInfo},
    SchedulerResult,
};

/// 任务调度服务接口
///
/// 负责任务的扫描、调度决策和任务分发。这是整个调度系统的核心组件，
/// 实现了任务从定义到执行的调度逻辑。
///
/// # 核心功能
///
/// 1. **任务扫描** - 定期扫描需要执行的任务
/// 2. **依赖检查** - 验证任务的前置依赖是否满足
/// 3. **实例创建** - 为待执行任务创建运行实例
/// 4. **队列分发** - 将任务实例分发到消息队列
///
/// # 调度策略
///
/// - 基于时间的调度（cron表达式、间隔时间）
/// - 基于依赖的调度（前置任务完成）
/// - 基于资源的调度（Worker可用性）
/// - 基于优先级的调度（高优先级优先）
///
/// # 线程安全
///
/// 实现必须是线程安全的，支持多个调度器实例同时运行。
///
/// # 示例实现
///
/// ```rust
/// use async_trait::async_trait;
/// use scheduler_core::traits::TaskSchedulerService;
///
/// pub struct DefaultTaskScheduler {
///     task_repo: Arc<dyn TaskRepository>,
///     task_run_repo: Arc<dyn TaskRunRepository>,
///     message_queue: Arc<dyn MessageQueue>,
/// }
///
/// #[async_trait]
/// impl TaskSchedulerService for DefaultTaskScheduler {
///     async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>> {
///         // 1. 获取所有可调度的任务
///         let schedulable_tasks = self.task_repo
///             .get_schedulable_tasks(Utc::now())
///             .await?;
///         
///         let mut scheduled_runs = Vec::new();
///         
///         for task in schedulable_tasks {
///             // 2. 检查依赖
///             if self.check_dependencies(&task).await? {
///                 // 3. 创建运行实例
///                 let task_run = self.create_task_run(&task).await?;
///                 
///                 // 4. 分发到队列
///                 self.dispatch_to_queue(&task_run).await?;
///                 
///                 scheduled_runs.push(task_run);
///             }
///         }
///         
///         Ok(scheduled_runs)
///     }
///     
///     // ... 其他方法实现
/// }
/// ```
#[async_trait]
pub trait TaskSchedulerService: Send + Sync {
    /// 扫描并调度任务
    ///
    /// 执行完整的任务调度周期：扫描待执行任务、检查依赖、创建执行实例、
    /// 分发到队列。这是调度器的主要工作循环。
    ///
    /// # 返回值
    ///
    /// 返回本次调度周期中成功调度的任务运行实例列表。
    ///
    /// # 调度流程
    ///
    /// 1. 查询所有可调度的任务（时间到期、状态活跃）
    /// 2. 逐个检查任务的依赖关系
    /// 3. 为满足条件的任务创建运行实例
    /// 4. 将任务实例分发到消息队列
    /// 5. 返回调度成功的任务列表
    ///
    /// # 错误
    ///
    /// * `DatabaseError` - 数据库查询失败
    /// * `QueueError` - 消息队列操作失败
    /// * `ValidationError` - 任务数据验证失败
    ///
    /// # 示例
    ///
    /// ```rust
    /// let scheduled_tasks = scheduler.scan_and_schedule().await?;
    /// println!("本次调度了 {} 个任务", scheduled_tasks.len());
    /// 
    /// for task_run in scheduled_tasks {
    ///     println!("调度任务: {} (实例ID: {})", 
    ///              task_run.task_id, task_run.id);
    /// }
    /// ```
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>>;

    /// 检查任务依赖
    ///
    /// 验证指定任务的所有前置依赖是否都已满足。只有依赖满足的任务
    /// 才能被调度执行。
    ///
    /// # 参数
    ///
    /// * `task` - 要检查依赖的任务对象
    ///
    /// # 返回值
    ///
    /// 依赖满足时返回 `true`，否则返回 `false`。
    ///
    /// # 依赖检查逻辑
    ///
    /// 1. 获取任务的所有直接依赖
    /// 2. 检查每个依赖任务的最新执行状态
    /// 3. 验证执行时间是否在有效期内
    /// 4. 检查依赖任务是否成功完成
    /// 5. 递归检查间接依赖（可选）
    ///
    /// # 示例
    ///
    /// ```rust
    /// if scheduler.check_dependencies(&task).await? {
    ///     println!("任务 {} 依赖已满足，可以执行", task.name);
    ///     let task_run = scheduler.create_task_run(&task).await?;
    /// } else {
    ///     println!("任务 {} 依赖未满足，跳过调度", task.name);
    /// }
    /// ```
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool>;

    /// 创建任务运行实例
    ///
    /// 为指定任务创建一个新的运行实例，用于跟踪任务的执行过程。
    /// 每次任务执行都会创建独立的运行实例。
    ///
    /// # 参数
    ///
    /// * `task` - 要执行的任务对象
    ///
    /// # 返回值
    ///
    /// 成功时返回新创建的 `TaskRun` 实例，包含数据库生成的ID。
    ///
    /// # 创建逻辑
    ///
    /// 1. 基于任务配置创建运行实例
    /// 2. 设置初始状态为 `Pending`
    /// 3. 记录创建时间和触发类型
    /// 4. 生成执行参数和上下文
    /// 5. 持久化到数据库
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 任务数据无效
    /// * `DatabaseError` - 数据库操作失败
    /// * `ResourceError` - 系统资源不足
    ///
    /// # 示例
    ///
    /// ```rust
    /// let task_run = scheduler.create_task_run(&task).await?;
    /// println!("创建任务实例: {} (任务: {})", task_run.id, task.name);
    /// 
    /// assert_eq!(task_run.task_id, task.id);
    /// assert_eq!(task_run.status, TaskRunStatus::Pending);
    /// assert!(task_run.created_at.is_some());
    /// ```
    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun>;

    /// 分发任务到消息队列
    ///
    /// 将任务运行实例发送到消息队列，供Worker节点拉取执行。
    /// 这是任务从调度器到执行器的关键传递环节。
    ///
    /// # 参数
    ///
    /// * `task_run` - 要分发的任务运行实例
    ///
    /// # 分发策略
    ///
    /// - 根据任务类型选择合适的队列
    /// - 设置消息优先级和过期时间
    /// - 包含执行所需的完整上下文
    /// - 支持重试和死信队列
    ///
    /// # 错误
    ///
    /// * `QueueError` - 消息队列操作失败
    /// * `SerializationError` - 消息序列化失败
    /// * `NetworkError` - 网络连接异常
    ///
    /// # 消息格式
    ///
    /// 发送到队列的消息通常包含：
    /// - 任务运行实例完整信息
    /// - 执行参数和环境变量
    /// - 资源限制和超时配置
    /// - 回调和通知配置
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 创建任务实例
    /// let task_run = scheduler.create_task_run(&task).await?;
    /// 
    /// // 分发到队列
    /// scheduler.dispatch_to_queue(&task_run).await?;
    /// println!("任务实例 {} 已分发到队列", task_run.id);
    /// 
    /// // 更新状态为已分发
    /// task_run_repo.update_status(
    ///     task_run.id, 
    ///     TaskRunStatus::Queued, 
    ///     None
    /// ).await?;
    /// ```
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()>;
}

/// 状态监听服务接口
///
/// 负责监听和处理任务状态变化事件。这是调度系统中的事件驱动组件，
/// 实现了任务状态的实时监控和响应处理。
///
/// # 核心功能
///
/// 1. **状态监听** - 持续监听来自Worker和外部系统的状态更新
/// 2. **事件处理** - 处理任务状态变化、完成、失败等事件
/// 3. **通知分发** - 向相关组件发送状态变化通知
/// 4. **异常检测** - 检测任务超时、Worker离线等异常情况
///
/// # 监听模式
///
/// - **推送模式** - 通过消息队列接收Worker推送的状态更新
/// - **轮询模式** - 定期查询数据库检查状态变化
/// - **混合模式** - 结合推送和轮询确保状态一致性
///
/// # 事件类型
///
/// - 任务开始执行
/// - 任务执行进度更新
/// - 任务执行完成（成功/失败）
/// - Worker心跳和健康状态
/// - 系统资源状态变化
///
/// # 线程安全
///
/// 实现必须是线程安全的，支持并发处理多个状态更新事件。
///
/// # 示例实现
///
/// ```rust
/// use async_trait::async_trait;
/// use scheduler_core::traits::StateListenerService;
/// use tokio::sync::mpsc;
///
/// pub struct MessageQueueStateListener {
///     queue_consumer: Arc<dyn MessageQueueConsumer>,
///     task_run_repo: Arc<dyn TaskRunRepository>,
///     notification_service: Arc<dyn NotificationService>,
/// }
///
/// #[async_trait]
/// impl StateListenerService for MessageQueueStateListener {
///     async fn listen_for_updates(&self) -> SchedulerResult<()> {
///         let mut receiver = self.queue_consumer
///             .subscribe("task_status_updates")
///             .await?;
///             
///         while let Some(message) = receiver.recv().await {
///             let update: TaskStatusUpdate = serde_json::from_slice(&message)?;
///             
///             // 处理状态更新
///             self.process_status_update(
///                 update.task_run_id,
///                 update.status,
///                 update.result,
///                 update.error_message,
///             ).await?;
///         }
///         
///         Ok(())
///     }
///     
///     async fn process_status_update(
///         &self,
///         task_run_id: i64,
///         status: TaskRunStatus,
///         result: Option<String>,
///         error_message: Option<String>,
///     ) -> SchedulerResult<()> {
///         // 更新数据库状态
///         self.task_run_repo.update_status(
///             task_run_id, 
///             status.clone(), 
///             error_message.clone()
///         ).await?;
///         
///         // 发送通知
///         match status {
///             TaskRunStatus::Completed => {
///                 self.notification_service
///                     .notify_task_completed(task_run_id, result)
///                     .await?;
///             }
///             TaskRunStatus::Failed => {
///                 self.notification_service
///                     .notify_task_failed(task_run_id, error_message)
///                     .await?;
///             }
///             _ => {}
///         }
///         
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait StateListenerService: Send + Sync {
    /// 监听状态更新
    ///
    /// 启动状态监听服务，持续监听来自Worker和其他组件的状态更新事件。
    /// 这是一个长期运行的方法，通常在独立的异步任务中执行。
    ///
    /// # 监听机制
    ///
    /// 1. 订阅消息队列的状态更新主题
    /// 2. 建立WebSocket连接接收实时推送
    /// 3. 启动定时器进行状态轮询
    /// 4. 处理接收到的每个状态更新事件
    ///
    /// # 错误处理
    ///
    /// - 连接中断时自动重连
    /// - 处理异常时记录日志但不终止监听
    /// - 提供熔断机制防止级联故障
    ///
    /// # 错误
    ///
    /// * `ConnectionError` - 消息队列或数据库连接失败
    /// * `ConfigurationError` - 监听配置无效
    /// * `SystemError` - 系统资源不足
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 在后台任务中启动监听
    /// let listener_handle = tokio::spawn(async move {
    ///     if let Err(e) = state_listener.listen_for_updates().await {
    ///         error!("状态监听服务异常: {}", e);
    ///     }
    /// });
    /// 
    /// // 服务运行期间监听会持续进行
    /// // 直到服务关闭或发生不可恢复的错误
    /// ```
    async fn listen_for_updates(&self) -> SchedulerResult<()>;

    /// 处理状态更新
    ///
    /// 处理单个任务状态更新事件，包括状态持久化、通知发送、
    /// 后续动作触发等完整的事件处理流程。
    ///
    /// # 参数
    ///
    /// * `task_run_id` - 任务运行实例ID
    /// * `status` - 新的任务状态
    /// * `result` - 任务执行结果（可选，通常在完成时提供）
    /// * `error_message` - 错误信息（可选，通常在失败时提供）
    ///
    /// # 处理流程
    ///
    /// 1. **状态验证** - 验证状态转换的合法性
    /// 2. **数据更新** - 更新数据库中的任务状态
    /// 3. **事件记录** - 记录状态变化的审计日志
    /// 4. **通知发送** - 向订阅者发送状态变化通知
    /// 5. **后续触发** - 触发依赖于此状态的后续任务
    ///
    /// # 状态转换规则
    ///
    /// - `Pending` → `Running` → `Completed`/`Failed`
    /// - `Queued` → `Running` → `Completed`/`Failed`
    /// - `Running` → `Cancelled` (允许取消)
    /// - 不允许从终态(`Completed`/`Failed`)转换到其他状态
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 状态转换不合法
    /// * `DatabaseError` - 数据库更新失败
    /// * `NotificationError` - 通知发送失败
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 处理任务完成事件
    /// state_listener.process_status_update(
    ///     12345,
    ///     TaskRunStatus::Completed,
    ///     Some("处理了1000条记录".to_string()),
    ///     None,
    /// ).await?;
    /// 
    /// // 处理任务失败事件
    /// state_listener.process_status_update(
    ///     12346,
    ///     TaskRunStatus::Failed,
    ///     None,
    ///     Some("数据库连接超时".to_string()),
    /// ).await?;
    /// ```
    async fn process_status_update(
        &self,
        task_run_id: i64,
        status: crate::models::TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;
}

/// 任务控制服务接口
///
/// 提供任务生命周期的手动控制功能。这是调度系统的管理接口，
/// 允许管理员和系统组件对任务进行直接控制操作。
///
/// # 核心功能
///
/// 1. **手动触发** - 立即触发任务执行，绕过正常调度
/// 2. **状态控制** - 暂停、恢复、取消任务和任务实例
/// 3. **实例管理** - 重启失败的实例，中止运行中的实例
/// 4. **批量操作** - 支持对多个任务进行批量控制
///
/// # 控制范围
///
/// - **任务级别** - 控制任务的调度状态（启用/暂停）
/// - **实例级别** - 控制特定任务运行实例的执行
/// - **系统级别** - 影响整个调度系统的运行状态
///
/// # 权限管理
///
/// 控制操作通常需要适当的权限验证：
/// - 管理员权限用于系统级操作
/// - 任务所有者权限用于任务级操作
/// - 服务间调用权限用于自动化操作
///
/// # 审计日志
///
/// 所有控制操作都应该记录审计日志，包括：
/// - 操作类型和参数
/// - 执行用户和时间
/// - 操作结果和影响范围
///
/// # 线程安全
///
/// 实现必须是线程安全的，支持并发的控制操作。
///
/// # 示例实现
///
/// ```rust
/// use async_trait::async_trait;
/// use scheduler_core::traits::TaskControlService;
///
/// pub struct DefaultTaskControlService {
///     task_repo: Arc<dyn TaskRepository>,
///     task_run_repo: Arc<dyn TaskRunRepository>,
///     scheduler: Arc<dyn TaskSchedulerService>,
///     worker_manager: Arc<dyn WorkerManager>,
///     audit_logger: Arc<dyn AuditLogger>,
/// }
///
/// #[async_trait]
/// impl TaskControlService for DefaultTaskControlService {
///     async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
///         // 1. 验证任务存在且可执行
///         let task = self.task_repo.get_by_id(task_id).await?
///             .ok_or_else(|| SchedulerError::TaskNotFound(task_id))?;
///             
///         if !task.is_active {
///             return Err(SchedulerError::TaskInactive(task_id));
///         }
///         
///         // 2. 检查依赖（如果需要）
///         if !self.scheduler.check_dependencies(&task).await? {
///             return Err(SchedulerError::DependenciesNotMet(task_id));
///         }
///         
///         // 3. 创建运行实例
///         let mut task_run = self.scheduler.create_task_run(&task).await?;
///         task_run.trigger_type = Some("manual".to_string());
///         
///         // 4. 分发到队列
///         self.scheduler.dispatch_to_queue(&task_run).await?;
///         
///         // 5. 记录审计日志
///         self.audit_logger.log_task_triggered(task_id, &task_run).await?;
///         
///         Ok(task_run)
///     }
///     
///     async fn pause_task(&self, task_id: i64) -> SchedulerResult<()> {
///         // 更新任务状态为暂停
///         self.task_repo.update_status(task_id, false).await?;
///         
///         // 记录操作日志
///         self.audit_logger.log_task_paused(task_id).await?;
///         
///         Ok(())
///     }
///     
///     // ... 其他方法实现
/// }
/// ```
#[async_trait]
pub trait TaskControlService: Send + Sync {
    /// 手动触发任务
    ///
    /// 立即触发指定任务的执行，绕过正常的调度时间检查。
    /// 这是一个常用的管理功能，用于测试、紧急执行或手动干预。
    ///
    /// # 参数
    ///
    /// * `task_id` - 要触发的任务ID
    ///
    /// # 返回值
    ///
    /// 成功时返回创建的 `TaskRun` 实例，包含触发信息。
    ///
    /// # 触发条件
    ///
    /// 1. 任务必须存在且处于活跃状态
    /// 2. 任务的依赖关系必须满足（如果配置了检查）
    /// 3. 系统资源足够支持新的任务实例
    /// 4. 没有达到任务的并发执行限制
    ///
    /// # 执行流程
    ///
    /// 1. **任务验证** - 检查任务存在性和可执行性
    /// 2. **依赖检查** - 验证前置依赖是否满足
    /// 3. **实例创建** - 创建新的任务运行实例
    /// 4. **队列分发** - 将实例发送到执行队列
    /// 5. **日志记录** - 记录手动触发的审计信息
    ///
    /// # 错误
    ///
    /// * `TaskNotFound` - 指定的任务不存在
    /// * `TaskInactive` - 任务处于非活跃状态
    /// * `DependenciesNotMet` - 任务依赖未满足
    /// * `ResourceExhausted` - 系统资源不足
    /// * `ConcurrencyLimitExceeded` - 超过并发限制
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 手动触发指定任务
    /// let task_run = task_control.trigger_task(12345).await?;
    /// println!("手动触发任务 {}, 实例ID: {}", 12345, task_run.id);
    /// 
    /// // 等待任务完成
    /// let status = task_run_repo.wait_for_completion(task_run.id).await?;
    /// match status {
    ///     TaskRunStatus::Completed => println!("任务执行成功"),
    ///     TaskRunStatus::Failed => println!("任务执行失败"),
    ///     _ => println!("任务状态: {:?}", status),
    /// }
    /// ```
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;

    /// 暂停任务
    ///
    /// 暂停指定任务的自动调度。暂停后的任务不会被调度器自动触发，
    /// 但仍可以通过手动触发执行。
    ///
    /// # 参数
    ///
    /// * `task_id` - 要暂停的任务ID
    ///
    /// # 暂停效果
    ///
    /// - 任务不再被调度器扫描和调度
    /// - 正在运行的实例不受影响，会继续执行
    /// - 任务配置保持不变，只是暂停调度
    /// - 仍可通过 `trigger_task` 手动执行
    ///
    /// # 使用场景
    ///
    /// - 任务维护期间暂停执行
    /// - 发现问题时紧急停止调度
    /// - 系统负载过高时临时暂停部分任务
    /// - 业务需求变化时暂停不需要的任务
    ///
    /// # 错误
    ///
    /// * `TaskNotFound` - 指定的任务不存在
    /// * `DatabaseError` - 数据库更新失败
    /// * `PermissionDenied` - 无权限执行此操作
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 暂停任务调度
    /// task_control.pause_task(12345).await?;
    /// println!("任务 12345 已暂停调度");
    /// 
    /// // 验证任务状态
    /// let task = task_repo.get_by_id(12345).await?.unwrap();
    /// assert!(!task.is_active);
    /// 
    /// // 即使暂停也可以手动触发
    /// let manual_run = task_control.trigger_task(12345).await?;
    /// println!("暂停期间手动触发成功，实例ID: {}", manual_run.id);
    /// ```
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;

    /// 恢复任务
    ///
    /// 恢复之前暂停的任务，使其重新参与自动调度。
    /// 恢复后任务会按照原有的调度规则正常执行。
    ///
    /// # 参数
    ///
    /// * `task_id` - 要恢复的任务ID
    ///
    /// # 恢复效果
    ///
    /// - 任务重新参与调度器扫描
    /// - 按照cron表达式或间隔时间正常调度
    /// - 恢复后立即检查是否有待执行的调度
    /// - 恢复任务的完整功能和配置
    ///
    /// # 恢复检查
    ///
    /// 恢复时会执行以下检查：
    /// 1. 验证任务配置仍然有效
    /// 2. 检查依赖任务的状态
    /// 3. 确认系统资源可用性
    /// 4. 验证权限和配置完整性
    ///
    /// # 错误
    ///
    /// * `TaskNotFound` - 指定的任务不存在
    /// * `TaskAlreadyActive` - 任务已经处于活跃状态
    /// * `ConfigurationError` - 任务配置无效
    /// * `DatabaseError` - 数据库更新失败
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 恢复暂停的任务
    /// task_control.resume_task(12345).await?;
    /// println!("任务 12345 已恢复调度");
    /// 
    /// // 验证任务状态
    /// let task = task_repo.get_by_id(12345).await?.unwrap();
    /// assert!(task.is_active);
    /// 
    /// // 任务会在下一个调度周期被扫描
    /// println!("任务将在下次调度扫描时参与调度");
    /// ```
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;

    /// 重启任务运行实例
    ///
    /// 重新启动一个失败或异常终止的任务运行实例。这会创建一个新的
    /// 运行实例，使用与原实例相同的参数和配置。
    ///
    /// # 参数
    ///
    /// * `task_run_id` - 要重启的任务运行实例ID
    ///
    /// # 返回值
    ///
    /// 成功时返回新创建的 `TaskRun` 实例。
    ///
    /// # 重启条件
    ///
    /// 1. 原实例必须处于终止状态（失败、取消、超时）
    /// 2. 对应的任务必须仍然存在且活跃
    /// 3. 系统资源足够支持重启
    /// 4. 没有超过重试次数限制
    ///
    /// # 重启逻辑
    ///
    /// 1. **状态检查** - 验证原实例可以重启
    /// 2. **参数复制** - 复制原实例的执行参数
    /// 3. **实例创建** - 创建新的运行实例
    /// 4. **关系建立** - 建立与原实例的重启关系
    /// 5. **队列分发** - 发送到执行队列
    ///
    /// # 错误
    ///
    /// * `TaskRunNotFound` - 指定的任务实例不存在
    /// * `InvalidState` - 实例状态不允许重启
    /// * `TaskNotFound` - 对应的任务不存在
    /// * `RetryLimitExceeded` - 超过重试次数限制
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 查找失败的任务实例
    /// let failed_runs = task_run_repo
    ///     .get_by_status(TaskRunStatus::Failed)
    ///     .await?;
    /// 
    /// for failed_run in failed_runs {
    ///     // 重启失败的实例
    ///     match task_control.restart_task_run(failed_run.id).await {
    ///         Ok(new_run) => {
    ///             println!("重启实例 {} -> {}", failed_run.id, new_run.id);
    ///         }
    ///         Err(e) => {
    ///             warn!("重启实例 {} 失败: {}", failed_run.id, e);
    ///         }
    ///     }
    /// }
    /// ```
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;

    /// 中止任务运行实例
    ///
    /// 强制中止一个正在运行的任务实例。这会向Worker发送取消信号，
    /// 并更新实例状态为已取消。
    ///
    /// # 参数
    ///
    /// * `task_run_id` - 要中止的任务运行实例ID
    ///
    /// # 中止流程
    ///
    /// 1. **状态检查** - 验证实例正在运行
    /// 2. **取消信号** - 向执行Worker发送取消信号
    /// 3. **状态更新** - 更新实例状态为取消中
    /// 4. **超时处理** - 设置强制终止超时
    /// 5. **资源清理** - 清理相关资源和临时文件
    ///
    /// # 取消机制
    ///
    /// - **优雅取消** - 先尝试发送取消信号让任务自主停止
    /// - **强制终止** - 超时后强制杀死执行进程
    /// - **资源清理** - 清理临时文件、数据库连接等资源
    /// - **状态同步** - 确保所有组件状态一致
    ///
    /// # 错误
    ///
    /// * `TaskRunNotFound` - 指定的任务实例不存在
    /// * `InvalidState` - 实例状态不允许中止
    /// * `WorkerNotResponding` - Worker节点无响应
    /// * `TimeoutError` - 中止操作超时
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 中止长时间运行的任务
    /// let running_tasks = task_run_repo
    ///     .get_long_running_tasks(Duration::from_hours(2))
    ///     .await?;
    /// 
    /// for task_run in running_tasks {
    ///     println!("中止长时间运行的任务实例: {}", task_run.id);
    ///     
    ///     match task_control.abort_task_run(task_run.id).await {
    ///         Ok(()) => {
    ///             println!("任务实例 {} 已成功中止", task_run.id);
    ///         }
    ///         Err(e) => {
    ///             error!("中止任务实例 {} 失败: {}", task_run.id, e);
    ///         }
    ///     }
    /// }
    /// ```
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
}

/// 任务分派策略接口
///
/// 定义任务分派到Worker节点的策略算法。这是调度系统的策略模式实现，
/// 允许插拔不同的Worker选择算法来优化任务分配。
///
/// # 核心功能
///
/// 1. **Worker选择** - 从可用Worker中选择最适合的节点
/// 2. **负载均衡** - 平衡各Worker节点的任务负载
/// 3. **亲和性处理** - 支持任务与Worker的亲和性配置
/// 4. **容错机制** - 处理Worker不可用的情况
///
/// # 策略类型
///
/// ## 负载均衡策略
/// - **轮询 (Round Robin)** - 依次分配给每个Worker
/// - **最少连接 (Least Connections)** - 选择当前任务最少的Worker
/// - **权重轮询 (Weighted Round Robin)** - 根据Worker权重分配
/// - **最少响应时间 (Least Response Time)** - 选择响应最快的Worker
///
/// ## 亲和性策略
/// - **节点亲和 (Node Affinity)** - 任务绑定到特定节点
/// - **类型亲和 (Type Affinity)** - 根据任务类型选择专用Worker
/// - **数据本地性 (Data Locality)** - 选择距离数据最近的Worker
///
/// ## 混合策略
/// - **智能分派 (Smart Dispatch)** - 综合考虑多个因素
/// - **动态适应 (Dynamic Adaptive)** - 根据实时状态调整策略
/// - **故障转移 (Failover)** - 主备节点自动切换
///
/// # 选择因素
///
/// Worker选择时通常考虑的因素：
/// - CPU使用率和内存占用
/// - 当前任务数量和队列长度
/// - 网络延迟和带宽情况
/// - 专用资源和依赖软件
/// - 历史执行成功率
/// - 地理位置和数据亲和性
///
/// # 线程安全
///
/// 实现必须是线程安全的，支持并发的Worker选择请求。
///
/// # 示例实现
///
/// ```rust
/// use async_trait::async_trait;
/// use scheduler_core::traits::TaskDispatchStrategy;
///
/// /// 负载均衡分派策略
/// pub struct LoadBalancedStrategy {
///     weight_config: HashMap<String, f32>,
/// }
///
/// #[async_trait]
/// impl TaskDispatchStrategy for LoadBalancedStrategy {
///     async fn select_worker(
///         &self,
///         task: &Task,
///         available_workers: &[WorkerInfo],
///     ) -> SchedulerResult<Option<String>> {
///         // 筛选支持此任务类型的Worker
///         let compatible_workers: Vec<_> = available_workers
///             .iter()
///             .filter(|w| w.supports_task_type(&task.task_type))
///             .filter(|w| w.is_healthy())
///             .collect();
///             
///         if compatible_workers.is_empty() {
///             return Ok(None);
///         }
///         
///         // 计算每个Worker的负载权重
///         let best_worker = compatible_workers
///             .iter()
///             .min_by(|a, b| {
///                 let score_a = self.calculate_load_score(a);
///                 let score_b = self.calculate_load_score(b);
///                 score_a.partial_cmp(&score_b).unwrap()
///             });
///             
///         Ok(best_worker.map(|w| w.id.clone()))
///     }
///     
///     fn name(&self) -> &str {
///         "load-balanced"
///     }
/// }
///
/// impl LoadBalancedStrategy {
///     fn calculate_load_score(&self, worker: &WorkerInfo) -> f32 {
///         let base_weight = self.weight_config
///             .get(&worker.id)
///             .copied()
///             .unwrap_or(1.0);
///             
///         // 综合考虑任务数量、CPU使用率、内存使用率
///         let task_factor = worker.current_task_count as f32 / worker.max_task_count as f32;
///         let cpu_factor = worker.cpu_usage / 100.0;
///         let memory_factor = worker.memory_usage / 100.0;
///         
///         (task_factor * 0.4 + cpu_factor * 0.3 + memory_factor * 0.3) / base_weight
///     }
/// }
///
/// /// 节点亲和策略
/// pub struct NodeAffinityStrategy;
///
/// #[async_trait]
/// impl TaskDispatchStrategy for NodeAffinityStrategy {
///     async fn select_worker(
///         &self,
///         task: &Task,
///         available_workers: &[WorkerInfo],
///     ) -> SchedulerResult<Option<String>> {
///         // 检查任务是否指定了节点亲和
///         if let Some(preferred_node) = &task.node_affinity {
///             // 查找指定的Worker
///             let preferred_worker = available_workers
///                 .iter()
///                 .find(|w| w.id == *preferred_node && w.is_healthy());
///                 
///             if let Some(worker) = preferred_worker {
///                 return Ok(Some(worker.id.clone()));
///             }
///         }
///         
///         // 如果没有指定亲和或指定的Worker不可用，回退到默认策略
///         let fallback_strategy = LoadBalancedStrategy::default();
///         fallback_strategy.select_worker(task, available_workers).await
///     }
///     
///     fn name(&self) -> &str {
///         "node-affinity"
///     }
/// }
/// ```
#[async_trait]
pub trait TaskDispatchStrategy: Send + Sync {
    /// 选择Worker执行任务
    ///
    /// 从可用的Worker列表中选择最适合执行指定任务的Worker节点。
    /// 这是策略的核心方法，实现具体的选择算法。
    ///
    /// # 参数
    ///
    /// * `task` - 要执行的任务对象，包含任务类型、资源需求等信息
    /// * `available_workers` - 当前可用的Worker节点列表
    ///
    /// # 返回值
    ///
    /// - `Some(worker_id)` - 选中的Worker节点ID
    /// - `None` - 没有合适的Worker可用
    ///
    /// # 选择逻辑
    ///
    /// 典型的选择流程包括：
    /// 1. **兼容性筛选** - 筛选支持任务类型的Worker
    /// 2. **健康状态检查** - 排除不健康或离线的Worker
    /// 3. **资源验证** - 检查Worker是否有足够资源
    /// 4. **策略计算** - 根据具体策略算法计算最优选择
    /// 5. **结果返回** - 返回选中的Worker ID
    ///
    /// # 选择标准
    ///
    /// 不同策略可能考虑的标准：
    /// - **负载情况** - 当前任务数量、CPU、内存使用率
    /// - **任务亲和** - 任务类型匹配、地理位置偏好
    /// - **历史表现** - 执行成功率、平均响应时间
    /// - **资源匹配** - CPU核数、内存大小、存储空间
    /// - **网络状况** - 延迟、带宽、连接稳定性
    ///
    /// # 错误处理
    ///
    /// 当没有合适的Worker时，应该返回 `Ok(None)` 而不是错误。
    /// 只有在策略执行过程中发生异常时才返回错误。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let task = Task {
    ///     id: 1,
    ///     task_type: "data-processing".to_string(),
    ///     cpu_requirement: 2,
    ///     memory_requirement: 4096,
    ///     // ... 其他字段
    /// };
    ///
    /// let workers = vec![
    ///     WorkerInfo {
    ///         id: "worker-1".to_string(),
    ///         task_types: vec!["data-processing".to_string()],
    ///         current_task_count: 2,
    ///         max_task_count: 10,
    ///         cpu_usage: 45.0,
    ///         memory_usage: 60.0,
    ///         is_healthy: true,
    ///     },
    ///     // ... 更多Worker
    /// ];
    ///
    /// match strategy.select_worker(&task, &workers).await? {
    ///     Some(worker_id) => {
    ///         println!("选择Worker: {}", worker_id);
    ///         // 分发任务到选中的Worker
    ///     }
    ///     None => {
    ///         println!("没有可用的Worker，任务将等待");
    ///         // 将任务放回队列等待
    ///     }
    /// }
    /// ```
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>>;

    /// 获取策略名称
    ///
    /// 返回策略的唯一标识名称，用于配置、日志记录和监控。
    /// 名称应该简洁明了，能够清楚表达策略的特点。
    ///
    /// # 返回值
    ///
    /// 策略的字符串标识，例如：
    /// - `"round-robin"` - 轮询策略
    /// - `"load-balanced"` - 负载均衡策略
    /// - `"node-affinity"` - 节点亲和策略
    /// - `"least-connections"` - 最少连接策略
    /// - `"smart-dispatch"` - 智能分派策略
    ///
    /// # 命名规范
    ///
    /// - 使用小写字母和连字符
    /// - 简洁明了，避免过长的名称
    /// - 能够清楚表达策略的核心特征
    /// - 在系统中保持唯一性
    ///
    /// # 使用场景
    ///
    /// 策略名称用于：
    /// - 配置文件中的策略选择
    /// - 日志和监控中的策略标识
    /// - 动态策略切换的依据
    /// - 性能统计和分析的分组
    ///
    /// # 示例
    ///
    /// ```rust
    /// impl TaskDispatchStrategy for MyStrategy {
    ///     fn name(&self) -> &str {
    ///         "my-custom-strategy"
    ///     }
    ///     
    ///     // ... 其他方法实现
    /// }
    ///
    /// // 在配置中使用
    /// let config = SchedulerConfig {
    ///     dispatch_strategy: "my-custom-strategy".to_string(),
    ///     // ... 其他配置
    /// };
    ///
    /// // 在日志中记录
    /// info!("使用分派策略: {}", strategy.name());
    /// ```
    fn name(&self) -> &str;
}

/// Worker服务接口
///
/// 定义Worker节点的核心服务接口。Worker是任务执行的核心组件，
/// 负责从消息队列获取任务、执行任务并报告执行状态。
///
/// # 核心功能
///
/// 1. **生命周期管理** - 启动、停止Worker服务
/// 2. **任务执行** - 轮询队列、执行任务、管理并发
/// 3. **状态报告** - 向调度器报告任务执行状态
/// 4. **健康监控** - 发送心跳、监控系统资源
///
/// # 服务架构
///
/// ## Worker生命周期
/// ```text
/// Stopped → Starting → Running → Stopping → Stopped
///     ↑                ↓
///     └── Error ←──────┘
/// ```
///
/// ## 任务执行流程
/// ```text
/// 1. 轮询消息队列
/// 2. 获取待执行任务
/// 3. 验证任务可执行性
/// 4. 创建执行环境
/// 5. 启动任务执行
/// 6. 监控执行进度
/// 7. 收集执行结果
/// 8. 报告执行状态
/// 9. 清理执行环境
/// ```
///
/// # 并发模型
///
/// Worker支持多种并发执行模式：
/// - **单任务模式** - 一次只执行一个任务，适合资源密集型任务
/// - **多任务模式** - 并发执行多个任务，适合I/O密集型任务
/// - **混合模式** - 根据任务类型动态调整并发数
/// - **资源限制** - 根据系统资源动态调整并发级别
///
/// # 容错机制
///
/// - **任务超时处理** - 超时自动终止并报告失败
/// - **异常捕获** - 捕获任务执行异常并详细记录
/// - **资源保护** - 防止任务消耗过多系统资源
/// - **状态恢复** - Worker重启后恢复未完成的任务
///
/// # 监控指标
///
/// Worker提供丰富的监控指标：
/// - 当前执行任务数量
/// - 历史任务执行统计
/// - 系统资源使用情况
/// - 网络连接状态
/// - 错误率和成功率
///
/// # 线程安全
///
/// 实现必须是线程安全的，支持并发的任务执行和状态管理。
///
/// # 示例实现
///
/// ```rust
/// use async_trait::async_trait;
/// use scheduler_core::traits::WorkerServiceTrait;
/// use tokio::sync::RwLock;
/// use std::collections::HashMap;
///
/// pub struct DefaultWorkerService {
///     worker_id: String,
///     queue_consumer: Arc<dyn MessageQueueConsumer>,
///     task_executor: Arc<dyn TaskExecutor>,
///     status_reporter: Arc<dyn StatusReporter>,
///     running_tasks: Arc<RwLock<HashMap<i64, TaskHandle>>>,
///     max_concurrent_tasks: usize,
///     is_running: Arc<AtomicBool>,
/// }
///
/// #[async_trait]
/// impl WorkerServiceTrait for DefaultWorkerService {
///     async fn start(&self) -> SchedulerResult<()> {
///         if self.is_running.load(Ordering::Relaxed) {
///             return Err(SchedulerError::WorkerAlreadyRunning);
///         }
///         
///         // 启动消息队列消费者
///         self.queue_consumer.start().await?;
///         
///         // 启动心跳服务
///         let heartbeat_service = self.clone();
///         tokio::spawn(async move {
///             loop {
///                 if let Err(e) = heartbeat_service.send_heartbeat().await {
///                     error!("心跳发送失败: {}", e);
///                 }
///                 tokio::time::sleep(Duration::from_secs(30)).await;
///             }
///         });
///         
///         self.is_running.store(true, Ordering::Relaxed);
///         info!("Worker {} 已启动", self.worker_id);
///         
///         Ok(())
///     }
///     
///     async fn poll_and_execute_tasks(&self) -> SchedulerResult<()> {
///         // 检查是否可以接受新任务
///         let current_count = self.get_current_task_count().await;
///         if current_count >= self.max_concurrent_tasks as i32 {
///             return Ok(()); // 达到并发限制，等待下次轮询
///         }
///         
///         // 从队列获取任务
///         if let Some(task_message) = self.queue_consumer.poll_task().await? {
///             let task_run: TaskRun = serde_json::from_slice(&task_message)?;
///             
///             // 检查是否可以执行此任务
///             if !self.can_accept_task(&task_run.task_type).await {
///                 // 将任务放回队列
///                 self.queue_consumer.reject_task(task_message).await?;
///                 return Ok(());
///             }
///             
///             // 执行任务
///             let executor = self.task_executor.clone();
///             let reporter = self.status_reporter.clone();
///             let running_tasks = self.running_tasks.clone();
///             
///             let task_handle = tokio::spawn(async move {
///                 // 报告任务开始
///                 let _ = reporter.report_status(
///                     task_run.id,
///                     TaskRunStatus::Running,
///                     None,
///                     None,
///                 ).await;
///                 
///                 // 执行任务
///                 match executor.execute(&task_run).await {
///                     Ok(result) => {
///                         // 报告任务成功
///                         let _ = reporter.report_status(
///                             task_run.id,
///                             TaskRunStatus::Completed,
///                             Some(result),
///                             None,
///                         ).await;
///                     }
///                     Err(e) => {
///                         // 报告任务失败
///                         let _ = reporter.report_status(
///                             task_run.id,
///                             TaskRunStatus::Failed,
///                             None,
///                             Some(e.to_string()),
///                         ).await;
///                     }
///                 }
///                 
///                 // 从运行列表中移除
///                 running_tasks.write().await.remove(&task_run.id);
///             });
///             
///             // 记录正在运行的任务
///             self.running_tasks.write().await.insert(task_run.id, task_handle);
///         }
///         
///         Ok(())
///     }
///     
///     // ... 其他方法实现
/// }
/// ```
#[async_trait]
pub trait WorkerServiceTrait: Send + Sync {
    /// 启动Worker服务
    ///
    /// 初始化并启动Worker服务，包括消息队列连接、心跳服务、
    /// 任务执行器等核心组件。启动后Worker开始轮询任务队列。
    ///
    /// # 启动流程
    ///
    /// 1. **初始化检查** - 验证Worker未在运行中
    /// 2. **资源准备** - 初始化执行环境和依赖资源
    /// 3. **队列连接** - 建立与消息队列的连接
    /// 4. **心跳启动** - 启动心跳发送服务
    /// 5. **状态更新** - 更新Worker状态为运行中
    /// 6. **注册服务** - 向调度器注册Worker信息
    ///
    /// # 启动检查
    ///
    /// 启动前会执行以下检查：
    /// - Worker服务未在运行
    /// - 必要的配置参数完整
    /// - 系统资源充足
    /// - 网络连接正常
    /// - 权限验证通过
    ///
    /// # 错误
    ///
    /// * `WorkerAlreadyRunning` - Worker已在运行中
    /// * `ConfigurationError` - 配置参数无效
    /// * `ConnectionError` - 消息队列连接失败
    /// * `ResourceError` - 系统资源不足
    /// * `PermissionError` - 权限验证失败
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 启动Worker服务
    /// worker.start().await?;
    /// println!("Worker服务已启动");
    /// 
    /// // 启动后Worker会自动开始轮询任务
    /// // 可以通过心跳确认服务正常运行
    /// tokio::time::sleep(Duration::from_secs(5)).await;
    /// 
    /// // 检查服务状态
    /// let task_count = worker.get_current_task_count().await;
    /// println!("当前运行任务数: {}", task_count);
    /// ```
    async fn start(&self) -> SchedulerResult<()>;

    /// 停止Worker服务
    ///
    /// 优雅地停止Worker服务，包括完成正在执行的任务、
    /// 断开队列连接、清理资源等。确保数据完整性和系统稳定性。
    ///
    /// # 停止流程
    ///
    /// 1. **停止接收** - 停止从队列接收新任务
    /// 2. **等待完成** - 等待正在执行的任务完成
    /// 3. **强制终止** - 超时后强制终止未完成的任务
    /// 4. **资源清理** - 清理临时文件、数据库连接等
    /// 5. **状态更新** - 更新Worker状态为已停止
    /// 6. **注销服务** - 从调度器注销Worker信息
    ///
    /// # 优雅停止
    ///
    /// - 设置停止标志，不再接收新任务
    /// - 给正在执行的任务合理的完成时间
    /// - 发送取消信号通知长时间运行的任务
    /// - 清理临时资源和中间文件
    /// - 确保状态一致性
    ///
    /// # 强制停止
    ///
    /// 如果优雅停止超时，会执行强制停止：
    /// - 立即终止所有正在执行的任务
    /// - 强制关闭所有连接和资源
    /// - 记录未完成任务的信息用于恢复
    ///
    /// # 错误
    ///
    /// * `WorkerNotRunning` - Worker未在运行中
    /// * `TasksStillRunning` - 仍有任务在执行且无法终止
    /// * `ResourceCleanupError` - 资源清理失败
    /// * `TimeoutError` - 停止操作超时
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 优雅停止Worker服务
    /// println!("正在停止Worker服务...");
    /// worker.stop().await?;
    /// println!("Worker服务已停止");
    /// 
    /// // 验证所有任务都已完成
    /// let remaining_tasks = worker.get_running_tasks().await;
    /// assert!(remaining_tasks.is_empty());
    /// ```
    async fn stop(&self) -> SchedulerResult<()>;

    /// 轮询并执行任务
    ///
    /// 从消息队列轮询待执行的任务，并启动任务执行。这是Worker的
    /// 核心工作循环，通常在定时器或事件驱动下周期性调用。
    ///
    /// # 轮询流程
    ///
    /// 1. **并发检查** - 检查当前并发数是否达到上限
    /// 2. **队列轮询** - 从消息队列获取待执行任务
    /// 3. **任务验证** - 验证任务的可执行性和兼容性
    /// 4. **执行启动** - 在独立的异步任务中启动执行
    /// 5. **状态跟踪** - 将任务加入运行任务列表
    /// 6. **结果处理** - 处理执行结果并报告状态
    ///
    /// # 并发控制
    ///
    /// - 检查当前运行任务数量
    /// - 根据系统资源动态调整并发数
    /// - 按任务类型分配执行资源
    /// - 防止资源过度消耗
    ///
    /// # 任务筛选
    ///
    /// 轮询时会进行任务筛选：
    /// - 检查任务类型兼容性
    /// - 验证资源需求是否满足
    /// - 检查任务优先级和截止时间
    /// - 确认依赖软件和环境
    ///
    /// # 错误处理
    ///
    /// - 队列连接异常时自动重连
    /// - 任务解析失败时记录错误日志  
    /// - 执行异常时隔离故障任务
    /// - 系统资源不足时降级处理
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 在主循环中轮询任务
    /// loop {
    ///     // 轮询并执行任务
    ///     worker.poll_and_execute_tasks().await?;
    ///     
    ///     // 发送心跳
    ///     worker.send_heartbeat().await?;
    ///     
    ///     // 等待下次轮询
    ///     tokio::time::sleep(Duration::from_secs(5)).await;
    /// }
    /// ```
    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()>;

    /// 发送状态更新
    ///
    /// 向调度系统发送任务执行状态更新。这是Worker与调度器通信的
    /// 主要方式，确保系统状态的实时同步。
    ///
    /// # 参数
    ///
    /// * `update` - 包含任务ID、状态、结果等信息的状态更新对象
    ///
    /// # 更新内容
    ///
    /// 状态更新通常包含：
    /// - 任务运行实例ID
    /// - 新的执行状态
    /// - 执行结果或错误信息
    /// - 进度信息（可选）
    /// - 资源使用情况（可选）
    /// - 预计完成时间（可选）
    ///
    /// # 发送机制
    ///
    /// - **即时发送** - 关键状态变化立即发送
    /// - **批量发送** - 进度更新等可以批量发送
    /// - **重试机制** - 发送失败时自动重试
    /// - **降级处理** - 网络异常时本地缓存
    ///
    /// # 状态类型
    ///
    /// 常见的状态更新：
    /// - `Running` - 任务开始执行
    /// - `Progress` - 执行进度更新
    /// - `Completed` - 任务执行成功
    /// - `Failed` - 任务执行失败
    /// - `Cancelled` - 任务被取消
    ///
    /// # 错误
    ///
    /// * `NetworkError` - 网络连接异常
    /// * `SerializationError` - 状态数据序列化失败
    /// * `ValidationError` - 状态更新数据无效
    /// * `TimeoutError` - 发送超时
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 报告任务开始执行
    /// let update = TaskStatusUpdate {
    ///     task_run_id: 12345,
    ///     status: TaskRunStatus::Running,
    ///     result: None,
    ///     error_message: None,
    ///     progress: Some(0),
    ///     worker_id: "worker-1".to_string(),
    ///     updated_at: Utc::now(),
    /// };
    /// 
    /// worker.send_status_update(update).await?;
    /// 
    /// // 报告执行进度
    /// let progress_update = TaskStatusUpdate {
    ///     task_run_id: 12345,
    ///     status: TaskRunStatus::Running,
    ///     progress: Some(50),
    ///     // ... 其他字段
    /// };
    /// 
    /// worker.send_status_update(progress_update).await?;
    /// 
    /// // 报告任务完成
    /// let completion_update = TaskStatusUpdate {
    ///     task_run_id: 12345,
    ///     status: TaskRunStatus::Completed,
    ///     result: Some("处理了1000条记录".to_string()),
    ///     // ... 其他字段
    /// };
    /// 
    /// worker.send_status_update(completion_update).await?;
    /// ```
    async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()>;

    /// 获取当前运行的任务数量
    ///
    /// 返回Worker当前正在执行的任务数量。这个指标用于
    /// 负载均衡、并发控制和系统监控。
    ///
    /// # 返回值
    ///
    /// 当前正在运行的任务数量，包括：
    /// - 正在执行中的任务
    /// - 准备执行但未开始的任务
    /// - 执行完成但未清理的任务
    ///
    /// # 计数范围
    ///
    /// 计数通常包括：
    /// - 状态为 `Running` 的任务
    /// - 状态为 `Queued` 但已分配给此Worker的任务
    /// - 正在进行资源清理的已完成任务
    ///
    /// # 用途
    ///
    /// - **负载均衡** - 调度器选择负载较低的Worker
    /// - **并发控制** - 限制Worker的最大并发任务数
    /// - **监控告警** - 监控系统负载和性能
    /// - **扩缩容决策** - 自动扩缩容的依据
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 检查当前负载
    /// let current_count = worker.get_current_task_count().await;
    /// let max_count = 10;
    /// 
    /// if current_count >= max_count {
    ///     println!("Worker负载已满，当前任务数: {}", current_count);
    /// } else {
    ///     println!("Worker可接受新任务，当前任务数: {}/{}", current_count, max_count);
    /// }
    /// 
    /// // 负载监控
    /// let load_percentage = (current_count as f32 / max_count as f32) * 100.0;
    /// if load_percentage > 80.0 {
    ///     warn!("Worker负载过高: {:.1}%", load_percentage);
    /// }
    /// ```
    async fn get_current_task_count(&self) -> i32;

    /// 检查是否可以接受新任务
    ///
    /// 根据任务类型和当前系统状态，判断Worker是否可以接受
    /// 指定类型的新任务。这是任务分派前的重要检查。
    ///
    /// # 参数
    ///
    /// * `task_type` - 要检查的任务类型
    ///
    /// # 返回值
    ///
    /// - `true` - 可以接受此类型的任务
    /// - `false` - 无法接受此类型的任务
    ///
    /// # 检查条件
    ///
    /// 通常检查以下条件：
    /// 1. **类型支持** - Worker是否支持此任务类型
    /// 2. **并发限制** - 是否达到最大并发任务数
    /// 3. **资源可用** - 系统资源是否足够
    /// 4. **健康状态** - Worker是否处于健康状态
    /// 5. **特殊限制** - 特定任务类型的额外限制
    ///
    /// # 检查维度
    ///
    /// - **功能维度** - 支持的任务类型列表
    /// - **容量维度** - 当前负载vs最大容量
    /// - **资源维度** - CPU、内存、磁盘可用性
    /// - **状态维度** - Worker运行状态和健康度
    ///
    /// # 使用场景
    ///
    /// - 调度器分派任务前的预检查
    /// - 负载均衡算法的决策依据
    /// - 任务队列的流量控制
    /// - 系统容量规划的参考
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 检查不同类型任务的接受能力
    /// let task_types = vec!["data-processing", "report-generation", "email-sending"];
    /// 
    /// for task_type in task_types {
    ///     let can_accept = worker.can_accept_task(task_type).await;
    ///     println!("任务类型 {}: {}", task_type, 
    ///         if can_accept { "可接受" } else { "不可接受" });
    /// }
    /// 
    /// // 在分派前检查
    /// if worker.can_accept_task(&task.task_type).await {
    ///     // 分派任务到此Worker
    ///     dispatch_task_to_worker(&task, &worker).await?;
    /// } else {
    ///     // 寻找其他可用Worker
    ///     find_alternative_worker(&task).await?;
    /// }
    /// ```
    async fn can_accept_task(&self, task_type: &str) -> bool;

    /// 取消正在运行的任务
    ///
    /// 取消指定的正在运行的任务实例。这是一个强制操作，
    /// 会立即停止任务执行并清理相关资源。
    ///
    /// # 参数
    ///
    /// * `task_run_id` - 要取消的任务运行实例ID
    ///
    /// # 取消流程
    ///
    /// 1. **任务查找** - 在运行任务列表中查找指定任务
    /// 2. **发送信号** - 向任务执行线程发送取消信号
    /// 3. **等待响应** - 等待任务优雅停止
    /// 4. **强制终止** - 超时后强制终止执行
    /// 5. **资源清理** - 清理临时文件和占用资源
    /// 6. **状态更新** - 更新任务状态为已取消
    ///
    /// # 取消机制
    ///
    /// - **协作取消** - 任务代码检查取消标志主动停止
    /// - **信号取消** - 发送系统信号终止执行进程
    /// - **超时取消** - 超过指定时间强制杀死进程
    /// - **资源回收** - 确保所有资源都被正确释放
    ///
    /// # 错误
    ///
    /// * `TaskRunNotFound` - 指定的任务实例不存在
    /// * `TaskNotRunning` - 任务未在运行中
    /// * `CancellationFailed` - 取消操作失败
    /// * `TimeoutError` - 取消操作超时
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 取消长时间运行的任务
    /// let running_tasks = worker.get_running_tasks().await;
    /// 
    /// for task_run in running_tasks {
    ///     let elapsed = Utc::now() - task_run.started_at.unwrap();
    ///     
    ///     // 取消运行超过2小时的任务
    ///     if elapsed > Duration::hours(2) {
    ///         println!("取消长时间运行的任务: {}", task_run.id);
    ///         
    ///         match worker.cancel_task(task_run.id).await {
    ///             Ok(()) => {
    ///                 println!("任务 {} 已成功取消", task_run.id);
    ///             }
    ///             Err(e) => {
    ///                 error!("取消任务 {} 失败: {}", task_run.id, e);
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()>;

    /// 获取正在运行的任务列表
    ///
    /// 返回Worker当前正在执行的所有任务的详细信息列表。
    /// 用于监控、调试和管理正在进行的任务。
    ///
    /// # 返回值
    ///
    /// 包含所有正在运行任务的 `TaskRun` 对象列表，每个对象包含：
    /// - 任务实例ID和基本信息
    /// - 当前执行状态和进度
    /// - 开始时间和预计完成时间
    /// - 资源使用情况
    /// - 执行上下文和参数
    ///
    /// # 任务状态
    ///
    /// 返回的任务通常包括以下状态：
    /// - `Running` - 正在执行中
    /// - `Starting` - 正在启动准备执行
    /// - `Stopping` - 正在停止或清理
    ///
    /// # 用途
    ///
    /// - **运维监控** - 查看Worker当前工作负载
    /// - **调试诊断** - 分析任务执行情况
    /// - **资源管理** - 了解资源占用分布
    /// - **性能优化** - 识别性能瓶颈任务
    ///
    /// # 数据时效性
    ///
    /// 返回的是调用时刻的快照数据，任务状态可能会快速变化。
    /// 对于实时监控，建议定期调用或结合状态更新事件。
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 获取当前运行的任务列表
    /// let running_tasks = worker.get_running_tasks().await;
    /// 
    /// println!("当前运行任务数: {}", running_tasks.len());
    /// 
    /// for task_run in running_tasks {
    ///     let elapsed = task_run.started_at
    ///         .map(|start| Utc::now() - start)
    ///         .unwrap_or_default();
    ///         
    ///     println!("任务 {}: {} (运行时长: {:?})", 
    ///         task_run.id, 
    ///         task_run.task_name,
    ///         elapsed
    ///     );
    /// }
    /// 
    /// // 按任务类型分组统计
    /// let mut type_counts = HashMap::new();
    /// for task_run in &running_tasks {
    ///     *type_counts.entry(&task_run.task_type).or_insert(0) += 1;
    /// }
    /// 
    /// for (task_type, count) in type_counts {
    ///     println!("任务类型 {}: {} 个正在运行", task_type, count);
    /// }
    /// ```
    async fn get_running_tasks(&self) -> Vec<TaskRun>;

    /// 检查任务是否正在运行
    ///
    /// 检查指定的任务实例是否正在此Worker上运行。
    /// 这是一个快速的状态检查方法。
    ///
    /// # 参数
    ///
    /// * `task_run_id` - 要检查的任务运行实例ID
    ///
    /// # 返回值
    ///
    /// - `true` - 任务正在运行中
    /// - `false` - 任务未在运行（可能已完成、失败或未开始）
    ///
    /// # 检查范围
    ///
    /// 检查任务是否处于以下状态之一：
    /// - 正在执行中
    /// - 准备启动
    /// - 正在清理资源
    ///
    /// # 性能考虑
    ///
    /// 这是一个轻量级操作，适合频繁调用。通常只需要
    /// 查找内存中的运行任务列表，不涉及磁盘或网络IO。
    ///
    /// # 使用场景
    ///
    /// - 调度器检查任务分派状态
    /// - 避免重复分派同一任务
    /// - 实现任务执行的幂等性
    /// - 监控系统的健康检查
    ///
    /// # 示例
    ///
    /// ```rust
    /// let task_run_id = 12345;
    /// 
    /// if worker.is_task_running(task_run_id).await {
    ///     println!("任务 {} 正在运行中", task_run_id);
    /// } else {
    ///     println!("任务 {} 未在运行", task_run_id);
    ///     
    ///     // 可能需要重新分派或检查任务状态
    ///     let task_status = task_run_repo.get_status(task_run_id).await?;
    ///     println!("任务状态: {:?}", task_status);
    /// }
    /// 
    /// // 批量检查多个任务
    /// let task_ids = vec![12345, 12346, 12347];
    /// let mut running_count = 0;
    /// 
    /// for task_id in task_ids {
    ///     if worker.is_task_running(task_id).await {
    ///         running_count += 1;
    ///     }
    /// }
    /// 
    /// println!("正在运行的任务数: {}", running_count);
    /// ```
    async fn is_task_running(&self, task_run_id: i64) -> bool;

    /// 发送心跳
    ///
    /// 向调度系统发送心跳信号，表明Worker服务正常运行。
    /// 心跳包含Worker的健康状态和基本运行指标。
    ///
    /// # 心跳内容
    ///
    /// 心跳消息通常包含：
    /// - Worker唯一标识
    /// - 当前时间戳
    /// - 服务运行状态
    /// - 当前任务数量
    /// - 系统资源使用情况
    /// - 支持的任务类型
    /// - 网络连接状态
    ///
    /// # 心跳频率
    ///
    /// 建议的心跳间隔：
    /// - **正常情况** - 每30-60秒发送一次
    /// - **高负载时** - 可适当降低频率避免额外开销
    /// - **故障恢复** - 可适当提高频率快速同步状态
    ///
    /// # 失败处理
    ///
    /// 心跳发送失败时的处理策略：
    /// - 记录错误日志但不影响任务执行
    /// - 实施指数退避重试机制
    /// - 连续失败时触发告警
    /// - 网络恢复后立即发送状态同步
    ///
    /// # 调度器处理
    ///
    /// 调度器根据心跳信息：
    /// - 更新Worker状态和负载信息
    /// - 检测Worker健康状况
    /// - 进行任务分派决策
    /// - 触发故障转移机制
    ///
    /// # 错误
    ///
    /// * `NetworkError` - 网络连接异常
    /// * `SerializationError` - 心跳数据序列化失败
    /// * `TimeoutError` - 发送超时
    /// * `ConfigurationError` - 心跳配置无效
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 启动心跳服务
    /// let worker_clone = worker.clone();
    /// let heartbeat_handle = tokio::spawn(async move {
    ///     loop {
    ///         match worker_clone.send_heartbeat().await {
    ///             Ok(()) => {
    ///                 debug!("心跳发送成功");
    ///             }
    ///             Err(e) => {
    ///                 warn!("心跳发送失败: {}", e);
    ///                 
    ///                 // 实施退避策略
    ///                 tokio::time::sleep(Duration::from_secs(5)).await;
    ///             }
    ///         }
    ///         
    ///         // 正常心跳间隔
    ///         tokio::time::sleep(Duration::from_secs(30)).await;
    ///     }
    /// });
    /// 
    /// // 在主服务循环中也可以发送心跳
    /// loop {
    ///     worker.poll_and_execute_tasks().await?;
    ///     worker.send_heartbeat().await?;
    ///     tokio::time::sleep(Duration::from_secs(10)).await;
    /// }
    /// ```
    async fn send_heartbeat(&self) -> SchedulerResult<()>;
}
