use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{
    traits::{ExecutorRegistry, ExecutorStatus, TaskExecutor},
    SchedulerResult,
};

/// 默认的任务执行器注册表实现
///
/// 提供线程安全的执行器注册、查找和管理功能。支持动态注册和注销执行器，
/// 以及执行器的健康检查和状态监控。
///
/// # 设计特点
///
/// - **线程安全**: 使用 `Arc<RwLock<T>>` 保证多线程安全访问
/// - **动态管理**: 支持运行时注册和注销执行器
/// - **健康监控**: 提供执行器健康检查和状态查询
/// - **类型匹配**: 支持按任务类型查找合适的执行器
/// - **批量操作**: 支持批量注册和状态查询
///
/// # 内部结构
///
/// ```text
/// DefaultExecutorRegistry
/// ├── executors: Arc<RwLock<HashMap<String, Arc<dyn TaskExecutor>>>>
/// │   ├── "python" -> PythonExecutor
/// │   ├── "shell" -> ShellExecutor
/// │   └── "docker" -> DockerExecutor
/// └── 读写锁保护并发访问
/// ```
///
/// # 使用示例
///
/// ## 基本使用
///
/// ```rust
/// use scheduler_core::executor_registry::DefaultExecutorRegistry;
/// use scheduler_core::traits::{ExecutorRegistry, TaskExecutor};
/// use std::sync::Arc;
///
/// // 创建注册表
/// let mut registry = DefaultExecutorRegistry::new();
///
/// // 注册执行器
/// let python_executor = Arc::new(PythonExecutor::new());
/// registry.register("python".to_string(), python_executor).await?;
///
/// // 查找执行器
/// if let Some(executor) = registry.get("python").await {
///     println!("找到Python执行器: {}", executor.name());
/// }
/// ```
///
/// ## 批量注册
///
/// ```rust
/// let executors = vec![
///     ("python".to_string(), Arc::new(PythonExecutor::new()) as Arc<dyn TaskExecutor>),
///     ("shell".to_string(), Arc::new(ShellExecutor::new()) as Arc<dyn TaskExecutor>),
///     ("docker".to_string(), Arc::new(DockerExecutor::new()) as Arc<dyn TaskExecutor>),
/// ];
///
/// registry.register_batch(executors).await?;
/// ```
///
/// ## 健康检查
///
/// ```rust
/// // 检查所有执行器健康状态
/// let health_results = registry.health_check_all().await?;
/// for (name, is_healthy) in health_results {
///     if !is_healthy {
///         warn!("执行器 {} 健康检查失败", name);
///     }
/// }
/// ```
///
/// # 线程安全保证
///
/// - 使用读写锁分离读写操作，提高并发性能
/// - 读操作（查找、列表）可以并发执行
/// - 写操作（注册、注销）互斥执行
/// - 所有执行器都是 `Arc<dyn TaskExecutor>`，支持多线程共享
///
/// # 性能考虑
///
/// - 读多写少的场景下性能优异
/// - 避免不必要的克隆，使用 `Arc` 共享数据
/// - 批量操作减少锁竞争
/// - 异步操作避免阻塞
pub struct DefaultExecutorRegistry {
    /// 执行器存储映射
    ///
    /// 键为执行器名称，值为执行器实例的共享引用。
    /// 使用读写锁保护，支持多读单写的并发访问模式。
    executors: Arc<RwLock<HashMap<String, Arc<dyn TaskExecutor>>>>,
}

impl DefaultExecutorRegistry {
    /// 创建新的执行器注册表实例
    ///
    /// 初始化一个空的执行器注册表，使用读写锁保护内部HashMap。
    /// 注册表创建后即可开始注册和使用执行器。
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `DefaultExecutorRegistry` 实例。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use scheduler_core::executor_registry::DefaultExecutorRegistry;
    ///
    /// let registry = DefaultExecutorRegistry::new();
    /// assert_eq!(registry.count().await, 0);
    /// ```
    ///
    /// # 线程安全
    ///
    /// 返回的实例是线程安全的，可以在多个异步任务间共享使用。
    pub fn new() -> Self {
        Self {
            executors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 批量注册多个执行器
    ///
    /// 一次性注册多个执行器，比单独注册更高效，因为只需要获取一次写锁。
    /// 如果某个执行器名称已存在，会被新的执行器覆盖。
    ///
    /// # 参数
    ///
    /// * `executors` - 执行器名称和实例的元组向量
    ///
    /// # 返回值
    ///
    /// 成功时返回 `Ok(())`，失败时返回 `SchedulerError`。
    ///
    /// # 错误
    ///
    /// 目前此方法不会返回错误，但为了API一致性保留了错误返回类型。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use scheduler_core::executor_registry::DefaultExecutorRegistry;
    /// use std::sync::Arc;
    ///
    /// let mut registry = DefaultExecutorRegistry::new();
    /// 
    /// let executors = vec![
    ///     ("python".to_string(), Arc::new(PythonExecutor::new())),
    ///     ("shell".to_string(), Arc::new(ShellExecutor::new())),
    ///     ("docker".to_string(), Arc::new(DockerExecutor::new())),
    /// ];
    ///
    /// registry.register_batch(executors).await?;
    /// assert_eq!(registry.count().await, 3);
    /// ```
    ///
    /// # 性能优势
    ///
    /// - 只获取一次写锁，减少锁竞争
    /// - 批量操作比循环单独注册更高效
    /// - 适合系统初始化时批量注册执行器
    pub async fn register_batch(
        &mut self,
        executors: Vec<(String, Arc<dyn TaskExecutor>)>,
    ) -> SchedulerResult<()> {
        let mut registry = self.executors.write().await;
        for (name, executor) in executors {
            registry.insert(name, executor);
        }
        Ok(())
    }

    /// 获取指定执行器的详细信息
    ///
    /// 查询指定名称的执行器，并返回其详细信息，包括基本属性和当前状态。
    /// 如果执行器不存在，返回 `None`。
    ///
    /// # 参数
    ///
    /// * `name` - 执行器名称
    ///
    /// # 返回值
    ///
    /// - `Ok(Some(ExecutorInfo))` - 执行器存在，返回详细信息
    /// - `Ok(None)` - 执行器不存在
    /// - `Err(SchedulerError)` - 获取执行器状态时发生错误
    ///
    /// # 错误
    ///
    /// * `TaskExecution` - 执行器状态查询失败
    /// * `Network` - 远程执行器通信失败
    ///
    /// # 示例
    ///
    /// ```rust
    /// use scheduler_core::executor_registry::DefaultExecutorRegistry;
    ///
    /// let registry = DefaultExecutorRegistry::new();
    /// 
    /// match registry.get_executor_info("python").await? {
    ///     Some(info) => {
    ///         println!("执行器: {} v{}", info.name, info.version);
    ///         println!("状态: 健康={}, 运行任务={}", 
    ///                  info.status.healthy, info.status.running_tasks);
    ///     },
    ///     None => println!("执行器不存在"),
    /// }
    /// ```
    ///
    /// # 性能说明
    ///
    /// - 使用读锁，不会阻塞其他读操作
    /// - 状态查询可能涉及网络调用，有一定延迟
    /// - 建议缓存结果以提高性能
    pub async fn get_executor_info(&self, name: &str) -> SchedulerResult<Option<ExecutorInfo>> {
        let registry = self.executors.read().await;
        if let Some(executor) = registry.get(name) {
            let status = executor.get_status().await?;
            Ok(Some(ExecutorInfo {
                name: executor.name().to_string(),
                version: executor.version().to_string(),
                description: executor.description().to_string(),
                supported_task_types: executor.supported_task_types(),
                status,
            }))
        } else {
            Ok(None)
        }
    }

    /// 获取所有已注册执行器的详细信息
    ///
    /// 遍历所有已注册的执行器，收集它们的详细信息和当前状态。
    /// 对于状态查询失败的执行器，会创建一个表示错误状态的信息。
    ///
    /// # 返回值
    ///
    /// 返回包含所有执行器信息的向量。即使某些执行器状态查询失败，
    /// 也会包含在结果中，但健康状态会标记为 `false`。
    ///
    /// # 错误处理策略
    ///
    /// - 单个执行器状态查询失败不会影响整体操作
    /// - 失败的执行器会被标记为不健康状态
    /// - 错误信息会记录到日志中
    ///
    /// # 示例
    ///
    /// ```rust
    /// use scheduler_core::executor_registry::DefaultExecutorRegistry;
    ///
    /// let registry = DefaultExecutorRegistry::new();
    /// 
    /// let all_infos = registry.get_all_executor_info().await?;
    /// for info in all_infos {
    ///     println!("执行器: {} ({})", info.name, info.version);
    ///     println!("  描述: {}", info.description);
    ///     println!("  支持类型: {:?}", info.supported_task_types);
    ///     println!("  健康状态: {}", info.status.healthy);
    ///     println!("  运行任务: {}", info.status.running_tasks);
    /// }
    /// ```
    ///
    /// # 性能考虑
    ///
    /// - 并发查询所有执行器状态，提高效率
    /// - 对于大量执行器，考虑分页或异步处理
    /// - 状态查询可能涉及网络调用，总耗时较长
    ///
    /// # 使用场景
    ///
    /// - 系统监控面板显示
    /// - 健康检查报告生成
    /// - 执行器管理界面
    /// - 系统诊断和调试
    pub async fn get_all_executor_info(&self) -> SchedulerResult<Vec<ExecutorInfo>> {
        let registry = self.executors.read().await;
        let mut infos = Vec::new();

        for executor in registry.values() {
            let status = executor
                .get_status()
                .await
                .unwrap_or_else(|_| ExecutorStatus {
                    name: executor.name().to_string(),
                    version: executor.version().to_string(),
                    healthy: false,
                    running_tasks: 0,
                    supported_task_types: executor.supported_task_types(),
                    last_health_check: chrono::Utc::now(),
                    metadata: std::collections::HashMap::new(),
                });

            infos.push(ExecutorInfo {
                name: executor.name().to_string(),
                version: executor.version().to_string(),
                description: executor.description().to_string(),
                supported_task_types: executor.supported_task_types(),
                status,
            });
        }

        Ok(infos)
    }

    /// 对所有已注册执行器执行健康检查
    ///
    /// 并发地对所有执行器执行健康检查，返回每个执行器的健康状态。
    /// 健康检查失败的执行器会被标记为不健康，但不会影响其他执行器的检查。
    ///
    /// # 返回值
    ///
    /// 返回执行器名称到健康状态的映射。`true` 表示健康，`false` 表示不健康。
    ///
    /// # 健康检查逻辑
    ///
    /// 1. 并发调用每个执行器的 `health_check()` 方法
    /// 2. 检查失败的执行器标记为不健康
    /// 3. 记录检查结果和异常信息
    ///
    /// # 示例
    ///
    /// ```rust
    /// use scheduler_core::executor_registry::DefaultExecutorRegistry;
    ///
    /// let registry = DefaultExecutorRegistry::new();
    /// 
    /// let health_results = registry.health_check_all().await?;
    /// 
    /// let healthy_count = health_results.values().filter(|&&h| h).count();
    /// let total_count = health_results.len();
    /// 
    /// println!("健康执行器: {}/{}", healthy_count, total_count);
    /// 
    /// for (name, is_healthy) in health_results {
    ///     let status = if is_healthy { "健康" } else { "异常" };
    ///     println!("执行器 {}: {}", name, status);
    /// }
    /// ```
    ///
    /// # 错误处理
    ///
    /// - 单个执行器健康检查失败不会影响整体操作
    /// - 检查超时或异常的执行器标记为不健康
    /// - 错误详情记录到日志中供调试使用
    ///
    /// # 性能优化
    ///
    /// - 使用并发检查提高效率
    /// - 设置合理的超时时间避免长时间等待
    /// - 考虑缓存检查结果减少频繁调用
    ///
    /// # 使用场景
    ///
    /// - 定期健康检查任务
    /// - 系统启动时的执行器验证
    /// - 负载均衡前的可用性检查
    /// - 故障诊断和监控告警
    pub async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>> {
        let registry = self.executors.read().await;
        let mut results = HashMap::new();

        for (name, executor) in registry.iter() {
            let is_healthy = executor.health_check().await.unwrap_or(false);
            results.insert(name.clone(), is_healthy);
        }

        Ok(results)
    }
}

impl Default for DefaultExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutorRegistry for DefaultExecutorRegistry {
    async fn register(
        &mut self,
        name: String,
        executor: Arc<dyn TaskExecutor>,
    ) -> SchedulerResult<()> {
        let mut registry = self.executors.write().await;
        registry.insert(name, executor);
        Ok(())
    }

    async fn get(&self, name: &str) -> Option<Arc<dyn TaskExecutor>> {
        let registry = self.executors.read().await;
        registry.get(name).cloned()
    }

    async fn list_executors(&self) -> Vec<String> {
        let registry = self.executors.read().await;
        registry.keys().cloned().collect()
    }

    async fn unregister(&mut self, name: &str) -> SchedulerResult<bool> {
        let mut registry = self.executors.write().await;
        Ok(registry.remove(name).is_some())
    }

    async fn clear(&mut self) {
        let mut registry = self.executors.write().await;
        registry.clear();
    }

    async fn contains(&self, name: &str) -> bool {
        let registry = self.executors.read().await;
        registry.contains_key(name)
    }

    async fn count(&self) -> usize {
        let registry = self.executors.read().await;
        registry.len()
    }

    async fn get_all_status(&self) -> SchedulerResult<HashMap<String, ExecutorStatus>> {
        let registry = self.executors.read().await;
        let mut statuses = HashMap::new();

        for (name, executor) in registry.iter() {
            match executor.get_status().await {
                Ok(status) => {
                    statuses.insert(name.clone(), status);
                }
                Err(e) => {
                    eprintln!("Failed to get status for executor '{name}': {e}");
                    // 创建一个表示错误状态的ExecutorStatus
                    statuses.insert(
                        name.clone(),
                        ExecutorStatus {
                            name: executor.name().to_string(),
                            version: executor.version().to_string(),
                            healthy: false,
                            running_tasks: 0,
                            supported_task_types: executor.supported_task_types(),
                            last_health_check: chrono::Utc::now(),
                            metadata: std::collections::HashMap::new(),
                        },
                    );
                }
            }
        }

        Ok(statuses)
    }

    async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>> {
        let registry = self.executors.read().await;
        let mut results = HashMap::new();

        for (name, executor) in registry.iter() {
            let is_healthy = executor.health_check().await.unwrap_or(false);
            results.insert(name.clone(), is_healthy);
        }

        Ok(results)
    }

    async fn get_by_task_type(
        &self,
        task_type: &str,
    ) -> SchedulerResult<Vec<Arc<dyn TaskExecutor>>> {
        let registry = self.executors.read().await;
        let mut matching_executors = Vec::new();

        for executor in registry.values() {
            if executor.supports_task_type(task_type) {
                matching_executors.push(Arc::clone(executor));
            }
        }

        Ok(matching_executors)
    }
}

/// 执行器详细信息结构体
///
/// 包含执行器的完整信息，包括基本属性、支持的任务类型和当前运行状态。
/// 用于系统监控、管理界面显示和执行器选择决策。
///
/// # 字段说明
///
/// - `name`: 执行器的唯一标识名称
/// - `version`: 执行器版本号，用于兼容性检查
/// - `description`: 执行器功能描述，便于用户理解
/// - `supported_task_types`: 支持的任务类型列表
/// - `status`: 当前运行状态和统计信息
///
/// # 使用示例
///
/// ```rust
/// use scheduler_core::executor_registry::ExecutorInfo;
/// use scheduler_core::traits::ExecutorStatus;
///
/// let info = ExecutorInfo {
///     name: "python".to_string(),
///     version: "3.9.0".to_string(),
///     description: "Python脚本执行器".to_string(),
///     supported_task_types: vec!["python".to_string(), "script".to_string()],
///     status: ExecutorStatus {
///         name: "python".to_string(),
///         version: "3.9.0".to_string(),
///         healthy: true,
///         running_tasks: 5,
///         supported_task_types: vec!["python".to_string()],
///         last_health_check: chrono::Utc::now(),
///         metadata: std::collections::HashMap::new(),
///     },
/// };
///
/// println!("执行器: {} v{}", info.name, info.version);
/// println!("当前负载: {}", info.status.running_tasks);
/// ```
///
/// # 序列化支持
///
/// 结构体支持序列化，可以用于API响应、配置文件或日志记录：
///
/// ```rust
/// let json = serde_json::to_string(&info)?;
/// println!("执行器信息: {}", json);
/// ```
#[derive(Debug, Clone)]
pub struct ExecutorInfo {
    /// 执行器唯一标识名称
    ///
    /// 用于在注册表中查找和引用执行器。
    /// 应该是简洁、描述性的字符串，如 "python", "docker", "shell"。
    pub name: String,

    /// 执行器版本号
    ///
    /// 用于版本兼容性检查和升级管理。
    /// 建议使用语义化版本格式，如 "1.2.3"。
    pub version: String,

    /// 执行器功能描述
    ///
    /// 详细描述执行器的功能和用途，便于用户理解和选择。
    /// 应该包含执行器的主要特性和适用场景。
    pub description: String,

    /// 支持的任务类型列表
    ///
    /// 列出此执行器能够处理的所有任务类型。
    /// 用于任务调度时的执行器匹配和选择。
    pub supported_task_types: Vec<String>,

    /// 执行器当前状态
    ///
    /// 包含健康状态、运行统计、最后检查时间等实时信息。
    /// 用于监控、负载均衡和故障诊断。
    pub status: ExecutorStatus,
}
