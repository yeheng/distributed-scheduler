//! # 领域服务模块
//!
//! 实现核心业务逻辑的领域服务，封装复杂的业务规则和流程。
//! 领域服务是DDD架构中的重要组件，处理不属于单个实体的业务逻辑。
//!
//! ## 设计原则
//!
//! ### 领域驱动设计（DDD）
//! - **业务导向**: 服务方法直接对应业务用例
//! - **领域专家语言**: 使用业务术语命名和设计
//! - **封装复杂性**: 将复杂的业务规则封装在服务中
//! - **无状态设计**: 服务本身不保存状态，依赖仓储获取数据
//!
//! ### 职责分离
//! - **业务逻辑**: 处理业务规则和流程
//! - **数据访问**: 通过仓储接口访问数据
//! - **外部集成**: 协调多个仓储和外部服务
//! - **事务管理**: 确保业务操作的一致性
//!
//! ## 服务分类
//!
//! ### 任务调度服务
//! - 任务执行调度逻辑
//! - 依赖关系处理
//! - 重试机制实现
//! - 执行状态管理
//!
//! ## 使用示例
//!
//! ### 服务依赖注入
//!
//! ```rust
//! use scheduler_domain::services::TaskSchedulingService;
//! use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};
//! use std::sync::Arc;
//!
//! pub struct ApplicationService {
//!     task_scheduling: Arc<dyn TaskSchedulingService>,
//! }
//!
//! impl ApplicationService {
//!     pub fn new(
//!         task_repo: Arc<dyn TaskRepository>,
//!         task_run_repo: Arc<dyn TaskRunRepository>,
//!     ) -> Self {
//!         let task_scheduling = Arc::new(
//!             DefaultTaskSchedulingService::new(task_run_repo)
//!         );
//!         
//!         Self { task_scheduling }
//!     }
//! }
//! ```
//!
//! ### 业务流程编排
//!
//! ```rust
//! async fn execute_business_process(
//!     service: &dyn TaskSchedulingService,
//!     task: &Task
//! ) -> SchedulerResult<TaskRun> {
//!     // 1. 验证业务规则
//!     validate_task_execution_rules(task)?;
//!     
//!     // 2. 调度任务执行
//!     let task_run = service.schedule_task(task).await?;
//!     
//!     // 3. 记录业务事件
//!     record_task_scheduled_event(&task_run).await?;
//!     
//!     Ok(task_run)
//! }
//! ```

use crate::{
    entities::{Task, TaskRun},
    repositories::TaskRunRepository,
};
use async_trait::async_trait;
use scheduler_core::{SchedulerError, SchedulerResult};

/// 任务调度服务接口
///
/// 定义任务调度的核心业务逻辑，包括任务执行调度、取消和重试等操作。
/// 这是任务调度系统的核心领域服务，封装了复杂的调度规则和业务流程。
///
/// # 设计理念
///
/// - **业务中心**: 以业务用例为中心设计服务方法
/// - **规则封装**: 将复杂的调度规则封装在服务内部
/// - **状态管理**: 管理任务执行的完整生命周期
/// - **异常处理**: 提供完善的错误处理和恢复机制
///
/// # 核心职责
///
/// ## 任务调度
/// - 根据调度规则创建任务执行实例
/// - 处理任务依赖关系和前置条件
/// - 分配合适的Worker执行任务
/// - 管理任务执行队列和优先级
///
/// ## 执行控制
/// - 取消正在执行或等待的任务
/// - 处理任务执行失败的重试逻辑
/// - 管理任务超时和异常情况
/// - 维护执行状态的一致性
///
/// ## 业务规则
/// - 验证任务执行的前置条件
/// - 处理并发执行限制
/// - 实施重试策略和退避算法
/// - 管理资源分配和负载均衡
///
/// # 实现要求
///
/// - 必须是线程安全的 (`Send + Sync`)
/// - 所有操作都是异步的
/// - 支持事务性操作保证一致性
/// - 提供详细的错误信息和上下文
/// - 支持可观测性和监控
///
/// # 使用场景
///
/// - 定时任务的自动调度
/// - 手动触发任务执行
/// - 任务失败后的重试处理
/// - 任务取消和清理操作
/// - 依赖任务的协调执行
///
/// # 示例实现
///
/// ```rust
/// use async_trait::async_trait;
/// use scheduler_domain::services::TaskSchedulingService;
/// use scheduler_domain::entities::{Task, TaskRun, TaskRunStatus};
/// use scheduler_core::SchedulerResult;
///
/// pub struct MyTaskSchedulingService {
///     task_run_repo: Arc<dyn TaskRunRepository>,
///     worker_selector: Arc<dyn WorkerSelector>,
///     message_queue: Arc<dyn MessageQueue>,
/// }
///
/// #[async_trait]
/// impl TaskSchedulingService for MyTaskSchedulingService {
///     async fn schedule_task(&self, task: &Task) -> SchedulerResult<TaskRun> {
///         // 1. 验证任务可以执行
///         self.validate_task_execution(task).await?;
///         
///         // 2. 选择合适的Worker
///         let worker_id = self.worker_selector
///             .select_best_worker(&task.task_type).await?;
///         
///         // 3. 创建执行实例
///         let task_run = TaskRun::new(task.id, worker_id);
///         let created_run = self.task_run_repo.create(&task_run).await?;
///         
///         // 4. 发送到执行队列
///         self.message_queue.send_task(&created_run).await?;
///         
///         Ok(created_run)
///     }
///     
///     // 其他方法实现...
/// }
/// ```
#[async_trait]
pub trait TaskSchedulingService: Send + Sync {
    /// 调度任务执行
    ///
    /// 根据任务定义和调度规则创建新的执行实例，并安排到合适的Worker上执行。
    /// 这是任务调度的核心方法，包含完整的调度逻辑和业务规则验证。
    ///
    /// # 参数
    ///
    /// * `task` - 要调度执行的任务实体
    ///
    /// # 返回值
    ///
    /// 成功时返回新创建的 `TaskRun` 执行实例。
    ///
    /// # 错误
    ///
    /// * `InvalidTaskParams` - 任务状态不允许执行或参数无效
    /// * `CircularDependency` - 任务依赖关系形成循环
    /// * `WorkerNotFound` - 没有可用的Worker执行任务
    /// * `MessageQueue` - 发送到执行队列失败
    /// * `DatabaseOperation` - 创建执行实例失败
    ///
    /// # 调度逻辑
    ///
    /// ## 1. 前置条件验证
    /// - 检查任务状态是否为Active
    /// - 验证任务配置的完整性和有效性
    /// - 检查是否超过最大重试次数
    /// - 验证任务类型是否被支持
    ///
    /// ## 2. 依赖关系处理
    /// - 检查任务的前置依赖是否已完成
    /// - 验证依赖关系不形成循环
    /// - 等待依赖任务完成（如果需要）
    ///
    /// ## 3. 资源分配
    /// - 根据任务类型选择合适的Worker
    /// - 检查Worker的可用容量和负载
    /// - 考虑任务优先级和调度策略
    /// - 预留执行资源
    ///
    /// ## 4. 执行实例创建
    /// - 创建新的TaskRun记录
    /// - 设置初始状态为Pending
    /// - 记录分配的Worker和开始时间
    /// - 生成执行上下文和参数
    ///
    /// ## 5. 队列分发
    /// - 将执行实例发送到消息队列
    /// - 设置适当的优先级和延迟
    /// - 配置重试和超时参数
    /// - 记录分发日志
    ///
    /// # 业务规则
    ///
    /// - 同一任务的并发执行数量限制
    /// - 任务优先级影响调度顺序
    /// - Worker负载均衡和容量管理
    /// - 执行时间窗口限制
    /// - 资源配额和权限检查
    ///
    /// # 示例
    ///
    /// ```rust
    /// use scheduler_domain::entities::{Task, TaskStatus};
    /// use scheduler_domain::services::TaskSchedulingService;
    ///
    /// async fn schedule_daily_report(
    ///     service: &dyn TaskSchedulingService,
    ///     task: &Task
    /// ) -> SchedulerResult<()> {
    ///     // 验证任务可以执行
    ///     if task.status != TaskStatus::Active {
    ///         return Err(SchedulerError::InvalidTaskParams(
    ///             "任务未激活".to_string()
    ///         ));
    ///     }
    ///     
    ///     // 调度执行
    ///     let task_run = service.schedule_task(task).await?;
    ///     
    ///     println!("任务已调度执行: 实例ID={}, Worker={}", 
    ///              task_run.id, task_run.worker_id);
    ///     
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # 性能考虑
    ///
    /// - 调度决策应该快速完成，避免阻塞
    /// - 使用缓存减少重复的Worker查询
    /// - 批量处理多个任务的调度请求
    /// - 异步处理依赖关系检查
    ///
    /// # 监控指标
    ///
    /// - 调度成功率和失败率
    /// - 平均调度延迟时间
    /// - Worker选择的分布情况
    /// - 队列积压和处理速度
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<TaskRun>;

    /// 取消任务执行实例
    ///
    /// 取消指定的任务执行实例，停止其执行并清理相关资源。
    /// 支持取消等待中和运行中的任务实例。
    ///
    /// # 参数
    ///
    /// * `task_run_id` - 要取消的任务执行实例ID
    ///
    /// # 返回值
    ///
    /// 成功时返回 `Ok(())`。
    ///
    /// # 错误
    ///
    /// * `TaskRunNotFound` - 指定的执行实例不存在
    /// * `InvalidTaskParams` - 实例状态不允许取消（如已完成）
    /// * `MessageQueue` - 发送取消信号失败
    /// * `DatabaseOperation` - 更新实例状态失败
    ///
    /// # 取消逻辑
    ///
    /// ## 1. 状态验证
    /// - 检查执行实例是否存在
    /// - 验证当前状态是否允许取消
    /// - 确认取消操作的权限
    ///
    /// ## 2. 取消信号发送
    /// - 向Worker发送取消信号
    /// - 从执行队列中移除任务
    /// - 释放预留的资源
    ///
    /// ## 3. 状态更新
    /// - 将实例状态更新为Cancelled
    /// - 设置完成时间
    /// - 记录取消原因和操作者
    ///
    /// ## 4. 资源清理
    /// - 清理临时文件和数据
    /// - 释放Worker容量
    /// - 更新相关统计信息
    ///
    /// # 取消策略
    ///
    /// ## Pending状态
    /// - 直接从队列中移除
    /// - 更新状态为Cancelled
    /// - 不需要Worker交互
    ///
    /// ## Running状态
    /// - 发送中断信号给Worker
    /// - 等待Worker确认取消
    /// - 强制终止超时的任务
    ///
    /// # 示例
    ///
    /// ```rust
    /// async fn cancel_long_running_task(
    ///     service: &dyn TaskSchedulingService,
    ///     task_run_id: i64
    /// ) -> SchedulerResult<()> {
    ///     match service.cancel_task_run(task_run_id).await {
    ///         Ok(()) => {
    ///             println!("任务实例 {} 已成功取消", task_run_id);
    ///         },
    ///         Err(SchedulerError::TaskRunNotFound { id }) => {
    ///             println!("任务实例 {} 不存在", id);
    ///         },
    ///         Err(SchedulerError::InvalidTaskParams(msg)) => {
    ///             println!("无法取消任务: {}", msg);
    ///         },
    ///         Err(e) => {
    ///             eprintln!("取消任务时发生错误: {}", e);
    ///             return Err(e);
    ///         }
    ///     }
    ///     
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # 优雅取消
    ///
    /// ```rust
    /// async fn graceful_cancel_with_timeout(
    ///     service: &dyn TaskSchedulingService,
    ///     task_run_id: i64,
    ///     timeout_seconds: u64
    /// ) -> SchedulerResult<()> {
    ///     // 发送取消信号
    ///     service.cancel_task_run(task_run_id).await?;
    ///     
    ///     // 等待任务优雅停止
    ///     let start_time = Utc::now();
    ///     loop {
    ///         if let Some(run) = task_run_repo.find_by_id(task_run_id).await? {
    ///             if run.status == TaskRunStatus::Cancelled {
    ///                 break; // 任务已成功取消
    ///             }
    ///         }
    ///         
    ///         // 检查超时
    ///         let elapsed = (Utc::now() - start_time).num_seconds() as u64;
    ///         if elapsed > timeout_seconds {
    ///             // 强制终止
    ///             force_terminate_task(task_run_id).await?;
    ///             break;
    ///         }
    ///         
    ///         tokio::time::sleep(Duration::from_secs(1)).await;
    ///     }
    ///     
    ///     Ok(())
    /// }
    /// ```
    async fn cancel_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;

    /// 重试失败的任务
    ///
    /// 为失败的任务创建新的执行实例进行重试。实现智能重试策略，
    /// 包括重试次数限制、退避算法和失败原因分析。
    ///
    /// # 参数
    ///
    /// * `task_run_id` - 失败的任务执行实例ID
    ///
    /// # 返回值
    ///
    /// 成功时返回新创建的重试执行实例。
    ///
    /// # 错误
    ///
    /// * `TaskRunNotFound` - 指定的执行实例不存在
    /// * `InvalidTaskParams` - 实例状态不是失败或超过最大重试次数
    /// * `WorkerNotFound` - 没有可用的Worker执行重试
    /// * `DatabaseOperation` - 创建重试实例失败
    ///
    /// # 重试逻辑
    ///
    /// ## 1. 重试条件验证
    /// - 检查原实例状态是否为Failed
    /// - 验证重试次数是否超过限制
    /// - 分析失败原因是否可重试
    /// - 检查任务是否仍然有效
    ///
    /// ## 2. 重试策略选择
    /// - 根据失败类型选择重试策略
    /// - 计算重试延迟时间（指数退避）
    /// - 选择不同的Worker避免重复失败
    /// - 调整任务参数或配置
    ///
    /// ## 3. 重试实例创建
    /// - 复制原实例的基本信息
    /// - 增加重试计数器
    /// - 设置新的开始时间
    /// - 清空之前的错误信息
    ///
    /// ## 4. 调度重试执行
    /// - 应用重试延迟
    /// - 发送到执行队列
    /// - 记录重试日志
    /// - 更新监控指标
    ///
    /// # 重试策略
    ///
    /// ## 指数退避
    /// ```text
    /// 重试延迟 = base_delay * (2 ^ retry_count) + jitter
    /// 第1次重试: 1秒 + 随机抖动
    /// 第2次重试: 2秒 + 随机抖动  
    /// 第3次重试: 4秒 + 随机抖动
    /// ```
    ///
    /// ## 失败分类
    /// - **临时失败**: 网络超时、资源不足等，可重试
    /// - **永久失败**: 配置错误、权限问题等，不可重试
    /// - **系统失败**: Worker故障、数据库错误等，需要系统恢复
    ///
    /// # 示例
    ///
    /// ```rust
    /// async fn handle_task_failure(
    ///     service: &dyn TaskSchedulingService,
    ///     failed_run_id: i64
    /// ) -> SchedulerResult<()> {
    ///     // 获取失败的执行实例
    ///     let failed_run = task_run_repo.find_by_id(failed_run_id).await?
    ///         .ok_or(SchedulerError::TaskRunNotFound { id: failed_run_id })?;
    ///     
    ///     // 分析失败原因
    ///     let is_retryable = analyze_failure_cause(&failed_run.error_message);
    ///     
    ///     if is_retryable && failed_run.retry_count < 3 {
    ///         // 执行重试
    ///         match service.retry_failed_task(failed_run_id).await {
    ///             Ok(retry_run) => {
    ///                 println!("任务重试已调度: 实例ID={}, 重试次数={}", 
    ///                          retry_run.id, retry_run.retry_count);
    ///             },
    ///             Err(e) => {
    ///                 eprintln!("重试调度失败: {}", e);
    ///                 // 标记为最终失败
    ///                 mark_task_as_permanently_failed(failed_run_id).await?;
    ///             }
    ///         }
    ///     } else {
    ///         println!("任务不可重试或已达到最大重试次数");
    ///         send_failure_alert(&failed_run).await?;
    ///     }
    ///     
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # 智能重试
    ///
    /// ```rust
    /// async fn intelligent_retry(
    ///     service: &dyn TaskSchedulingService,
    ///     failed_run: &TaskRun
    /// ) -> SchedulerResult<Option<TaskRun>> {
    ///     // 分析失败模式
    ///     let failure_pattern = analyze_failure_pattern(failed_run).await?;
    ///     
    ///     match failure_pattern {
    ///         FailurePattern::TransientNetworkError => {
    ///             // 短延迟重试，使用相同Worker
    ///             service.retry_failed_task(failed_run.id).await.map(Some)
    ///         },
    ///         FailurePattern::WorkerOverload => {
    ///             // 延长延迟，选择不同Worker
    ///             let retry_run = service.retry_failed_task(failed_run.id).await?;
    ///             // 调整Worker选择策略
    ///             Ok(Some(retry_run))
    ///         },
    ///         FailurePattern::ConfigurationError => {
    ///             // 不可重试，需要人工干预
    ///             Ok(None)
    ///         },
    ///         _ => {
    ///             // 默认重试策略
    ///             service.retry_failed_task(failed_run.id).await.map(Some)
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// # 监控和告警
    ///
    /// - 重试成功率统计
    /// - 重试延迟时间分布
    /// - 失败原因分类统计
    /// - 重试次数达到上限的告警
    async fn retry_failed_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
}

/// 默认任务调度服务实现
///
/// 提供任务调度服务接口的基础实现，包含核心的调度逻辑和业务规则。
/// 这是一个泛型实现，可以与不同的仓储实现配合使用。
///
/// # 设计特点
///
/// - **泛型设计**: 支持不同的TaskRunRepository实现
/// - **依赖注入**: 通过构造函数注入依赖的仓储
/// - **无状态**: 服务本身不保存状态，所有数据通过仓储获取
/// - **线程安全**: 支持多线程并发访问
///
/// # 实现策略
///
/// ## 简化实现
/// 当前实现提供基础的调度功能，包括：
/// - 创建任务执行实例
/// - 基本的状态管理
/// - 简单的重试逻辑
///
/// ## 扩展点
/// 可以通过以下方式扩展功能：
/// - 添加Worker选择策略
/// - 实现复杂的依赖关系处理
/// - 集成消息队列系统
/// - 添加监控和日志记录
///
/// # 使用示例
///
/// ```rust
/// use scheduler_domain::services::DefaultTaskSchedulingService;
/// use scheduler_domain::repositories::TaskRunRepository;
/// use std::sync::Arc;
///
/// // 创建服务实例
/// let task_run_repo = Arc::new(PostgresTaskRunRepository::new(pool));
/// let scheduling_service = DefaultTaskSchedulingService::new(task_run_repo);
///
/// // 使用服务
/// let task_run = scheduling_service.schedule_task(&task).await?;
/// println!("任务已调度: {}", task_run.id);
/// ```
///
/// # 配置和定制
///
/// ```rust
/// impl<TRR> DefaultTaskSchedulingService<TRR>
/// where
///     TRR: TaskRunRepository,
/// {
///     // 可以添加配置参数
///     pub fn with_config(
///         task_run_repo: TRR,
///         config: SchedulingConfig
///     ) -> Self {
///         // 自定义配置实现
///         todo!()
///     }
///     
///     // 可以添加策略注入
///     pub fn with_worker_selector(
///         self,
///         selector: Arc<dyn WorkerSelector>
///     ) -> Self {
///         // 注入Worker选择策略
///         todo!()
///     }
/// }
/// ```
pub struct DefaultTaskSchedulingService<TRR>
where
    TRR: TaskRunRepository,
{
    /// 任务执行实例仓储
    ///
    /// 用于创建、查询和更新任务执行实例。
    /// 通过泛型参数支持不同的存储实现。
    task_run_repo: TRR,
}

impl<TRR> DefaultTaskSchedulingService<TRR>
where
    TRR: TaskRunRepository,
{
    /// 创建新的默认任务调度服务实例
    ///
    /// 使用提供的任务执行实例仓储创建服务实例。
    /// 这是最简单的构造方式，适用于基础的调度需求。
    ///
    /// # 参数
    ///
    /// * `task_run_repo` - 任务执行实例仓储实现
    ///
    /// # 返回值
    ///
    /// 返回新创建的服务实例。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use scheduler_domain::services::DefaultTaskSchedulingService;
    /// use scheduler_infrastructure::repositories::PostgresTaskRunRepository;
    ///
    /// let task_run_repo = PostgresTaskRunRepository::new(db_pool);
    /// let service = DefaultTaskSchedulingService::new(task_run_repo);
    /// ```
    ///
    /// # 依赖要求
    ///
    /// - `task_run_repo` 必须实现 `TaskRunRepository` trait
    /// - 仓储实现必须是线程安全的 (`Send + Sync`)
    /// - 建议使用连接池等资源管理机制
    pub fn new(task_run_repo: TRR) -> Self {
        Self { task_run_repo }
    }
}

#[async_trait]
impl<TRR> TaskSchedulingService for DefaultTaskSchedulingService<TRR>
where
    TRR: TaskRunRepository + 'static,
{
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<TaskRun> {
        // 基础实现：创建任务执行实例
        let task_run = TaskRun {
            id: 0, // 数据库生成
            task_id: task.id,
            worker_id: "".to_string(), // 稍后分配
            status: crate::entities::TaskRunStatus::Pending,
            started_at: chrono::Utc::now(),
            completed_at: None,
            error_message: None,
            result: None,
            retry_count: 0,
        };

        self.task_run_repo.create(&task_run).await
    }

    async fn cancel_task_run(&self, task_run_id: i64) -> SchedulerResult<()> {
        if let Some(mut task_run) = self.task_run_repo.find_by_id(task_run_id).await? {
            task_run.status = crate::entities::TaskRunStatus::Cancelled;
            task_run.completed_at = Some(chrono::Utc::now());
            self.task_run_repo.update(&task_run).await?;
        }
        Ok(())
    }

    async fn retry_failed_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun> {
        if let Some(failed_run) = self.task_run_repo.find_by_id(task_run_id).await? {
            let retry_run = TaskRun {
                id: 0,
                task_id: failed_run.task_id,
                worker_id: "".to_string(),
                status: crate::entities::TaskRunStatus::Pending,
                started_at: chrono::Utc::now(),
                completed_at: None,
                error_message: None,
                result: None,
                retry_count: failed_run.retry_count + 1,
            };

            self.task_run_repo.create(&retry_run).await
        } else {
            Err(SchedulerError::TaskRunNotFound { id: task_run_id })
        }
    }
}
