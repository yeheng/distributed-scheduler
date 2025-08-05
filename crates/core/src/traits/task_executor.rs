//! 任务执行器接口定义
//!
//! 此模块定义了任务执行的核心抽象接口，包括：
//! - 任务执行器接口
//! - 执行上下文和配置
//! - 执行器注册表管理
//! - 资源限制和状态监控
//!
//! ## 核心概念
//!
//! ### TaskExecutor
//! 任务执行器是系统中实际执行任务的组件，支持多种任务类型：
//! - Shell命令执行器
//! - Python脚本执行器
//! - Docker容器执行器
//! - HTTP API调用执行器
//!
//! ### ExecutorRegistry
//! 执行器注册表管理所有可用的执行器实例，提供：
//! - 动态注册/注销执行器
//! - 按任务类型查找合适的执行器
//! - 执行器健康检查和状态监控
//!
//! ## 使用示例
//!
//! ### 实现自定义执行器
//!
//! ```rust
//! use async_trait::async_trait;
//! use scheduler_core::traits::{TaskExecutor, TaskExecutionContextTrait};
//! use scheduler_core::{SchedulerResult, models::TaskResult};
//!
//! pub struct PythonExecutor {
//!     name: String,
//! }
//!
//! #[async_trait]
//! impl TaskExecutor for PythonExecutor {
//!     async fn execute_task(
//!         &self,
//!         context: &TaskExecutionContextTrait,
//!     ) -> SchedulerResult<TaskResult> {
//!         // 执行Python脚本逻辑
//!         // 1. 准备执行环境
//!         // 2. 执行脚本
//!         // 3. 收集结果
//!         Ok(TaskResult::success("执行成功".to_string()))
//!     }
//!
//!     fn supports_task_type(&self, task_type: &str) -> bool {
//!         task_type == "python"
//!     }
//!
//!     fn name(&self) -> &str {
//!         &self.name
//!     }
//!
//!     async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()> {
//!         // 实现任务取消逻辑
//!         Ok(())
//!     }
//!
//!     async fn is_running(&self, task_run_id: i64) -> SchedulerResult<bool> {
//!         // 检查任务运行状态
//!         Ok(false)
//!     }
//! }
//! ```
//!
//! ### 使用执行器注册表
//!
//! ```rust
//! use std::sync::Arc;
//! use scheduler_core::traits::{ExecutorRegistry, TaskExecutor};
//!
//! async fn setup_executors(registry: &mut dyn ExecutorRegistry) -> SchedulerResult<()> {
//!     // 注册Python执行器
//!     let python_executor = Arc::new(PythonExecutor {
//!         name: "python-executor".to_string(),
//!     });
//!     registry.register("python".to_string(), python_executor).await?;
//!
//!     // 查找并使用执行器
//!     if let Some(executor) = registry.get("python").await {
//!         let status = executor.get_status().await?;
//!         println!("执行器状态: {:?}", status);
//!     }
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    models::{TaskResult, TaskRun},
    SchedulerResult,
};

/// 任务执行上下文
///
/// 包含任务执行所需的完整上下文信息，包括任务参数、环境配置、
/// 资源限制等。这是执行器执行任务时的核心数据结构。
///
/// # 字段说明
///
/// * `task_run` - 任务运行实例，包含任务的基本信息和状态
/// * `task_type` - 任务类型，用于确定使用哪个执行器
/// * `parameters` - 任务参数，JSON格式的键值对
/// * `timeout_seconds` - 任务超时时间（秒）
/// * `environment` - 环境变量配置
/// * `working_directory` - 任务执行的工作目录
/// * `resource_limits` - 资源使用限制
///
/// # 示例
///
/// ```rust
/// use scheduler_core::traits::{TaskExecutionContextTrait, ResourceLimits};
/// use std::collections::HashMap;
///
/// let mut parameters = HashMap::new();
/// parameters.insert("input_file".to_string(), serde_json::json!("data.csv"));
///
/// let context = TaskExecutionContextTrait {
///     task_run: task_run,
///     task_type: "python".to_string(),
///     parameters,
///     timeout_seconds: 3600,
///     environment: HashMap::new(),
///     working_directory: Some("/tmp/workspace".to_string()),
///     resource_limits: ResourceLimits {
///         max_memory_mb: Some(1024),
///         max_cpu_percent: Some(80.0),
///         ..Default::default()
///     },
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionContextTrait {
    /// 任务运行实例
    pub task_run: TaskRun,
    /// 任务类型
    pub task_type: String,
    /// 任务参数
    pub parameters: HashMap<String, serde_json::Value>,
    /// 超时时间（秒）
    pub timeout_seconds: u64,
    /// 环境变量
    pub environment: HashMap<String, String>,
    /// 工作目录
    pub working_directory: Option<String>,
    /// 资源限制
    pub resource_limits: ResourceLimits,
}

/// 资源限制配置
///
/// 定义任务执行时的资源使用限制，用于防止任务消耗过多系统资源
/// 影响其他任务的正常执行。
///
/// # 字段说明
///
/// * `max_memory_mb` - 最大内存使用量（MB），None表示无限制
/// * `max_cpu_percent` - 最大CPU使用率（百分比，0-100），None表示无限制
/// * `max_disk_mb` - 最大磁盘使用量（MB），None表示无限制
/// * `max_network_kbps` - 最大网络带宽（KB/s），None表示无限制
///
/// # 示例
///
/// ```rust
/// use scheduler_core::traits::ResourceLimits;
///
/// // 创建有限制的资源配置
/// let limits = ResourceLimits {
///     max_memory_mb: Some(1024),      // 最大1GB内存
///     max_cpu_percent: Some(80.0),    // 最大80%CPU
///     max_disk_mb: Some(500),         // 最大500MB磁盘
///     max_network_kbps: Some(1000),   // 最大1MB/s网络
/// };
///
/// // 创建无限制的资源配置
/// let unlimited = ResourceLimits::default();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceLimits {
    /// 最大内存使用量（MB）
    pub max_memory_mb: Option<u64>,
    /// 最大CPU使用率（百分比）
    pub max_cpu_percent: Option<f64>,
    /// 最大磁盘使用量（MB）
    pub max_disk_mb: Option<u64>,
    /// 最大网络带宽（KB/s）
    pub max_network_kbps: Option<u64>,
}

/// 执行器状态信息
///
/// 包含执行器的运行状态和健康信息，用于监控和管理执行器的生命周期。
///
/// # 字段说明
///
/// * `name` - 执行器名称，唯一标识符
/// * `version` - 执行器版本号
/// * `healthy` - 执行器是否健康
/// * `running_tasks` - 当前正在运行的任务数
/// * `supported_task_types` - 支持的任务类型列表
/// * `last_health_check` - 最后一次健康检查时间
/// * `metadata` - 额外的状态元数据
///
/// # 示例
///
/// ```rust
/// use scheduler_core::traits::ExecutorStatus;
/// use chrono::Utc;
/// use std::collections::HashMap;
///
/// let status = ExecutorStatus {
///     name: "python-executor".to_string(),
///     version: "1.2.0".to_string(),
///     healthy: true,
///     running_tasks: 3,
///     supported_task_types: vec!["python".to_string(), "script".to_string()],
///     last_health_check: Utc::now(),
///     metadata: {
///         let mut meta = HashMap::new();
///         meta.insert("python_version".to_string(), serde_json::json!("3.9.7"));
///         meta.insert("max_concurrent".to_string(), serde_json::json!(10));
///         meta
///     },
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorStatus {
    /// 执行器名称
    pub name: String,
    /// 执行器版本
    pub version: String,
    /// 是否健康
    pub healthy: bool,
    /// 正在运行的任务数
    pub running_tasks: i32,
    /// 支持的任务类型
    pub supported_task_types: Vec<String>,
    /// 最后健康检查时间
    pub last_health_check: DateTime<Utc>,
    /// 额外的元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 任务执行器核心接口
///
/// 定义了任务执行的标准接口，所有执行器都必须实现此trait。
/// 支持多种任务类型的执行、任务生命周期管理和状态监控。
///
/// # 核心功能
///
/// 1. **任务执行** - 执行具体的任务逻辑
/// 2. **生命周期管理** - 任务的启动、取消、状态查询
/// 3. **类型支持** - 声明支持的任务类型
/// 4. **健康检查** - 执行器的健康状态监控
/// 5. **资源管理** - 执行器的预热和清理
///
/// # 线程安全
///
/// 此trait要求实现 `Send + Sync`，确保可以在多线程环境中安全使用。
/// 实现者应该使用适当的同步原语来保护共享状态。
///
/// # 实现要求
///
/// 实现者必须：
/// - 正确处理任务超时
/// - 提供准确的任务状态信息
/// - 实现优雅的任务取消机制
/// - 维护执行器健康状态
/// - 清理执行后的资源
///
/// # 示例实现
///
/// ```rust
/// use async_trait::async_trait;
/// use scheduler_core::traits::{TaskExecutor, TaskExecutionContextTrait, ExecutorStatus};
/// use scheduler_core::{SchedulerResult, models::TaskResult};
/// use std::collections::HashMap;
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
///
/// pub struct ShellExecutor {
///     name: String,
///     running_tasks: Arc<Mutex<HashMap<i64, tokio::process::Child>>>,
/// }
///
/// #[async_trait]
/// impl TaskExecutor for ShellExecutor {
///     async fn execute_task(
///         &self,
///         context: &TaskExecutionContextTrait,
///     ) -> SchedulerResult<TaskResult> {
///         // 从参数中获取命令
///         let command = context.parameters
///             .get("command")
///             .and_then(|v| v.as_str())
///             .ok_or_else(|| SchedulerError::ValidationError("缺少command参数".to_string()))?;
///
///         // 执行shell命令
///         let output = tokio::process::Command::new("sh")
///             .arg("-c")
///             .arg(command)
///             .output()
///             .await
///             .map_err(|e| SchedulerError::ExecutionError(e.to_string()))?;
///
///         if output.status.success() {
///             Ok(TaskResult::success(String::from_utf8_lossy(&output.stdout).to_string()))
///         } else {
///             Ok(TaskResult::failure(String::from_utf8_lossy(&output.stderr).to_string()))
///         }
///     }
///
///     fn supports_task_type(&self, task_type: &str) -> bool {
///         task_type == "shell" || task_type == "bash"
///     }
///
///     fn name(&self) -> &str {
///         &self.name
///     }
///
///     async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()> {
///         let mut running = self.running_tasks.lock().await;
///         if let Some(mut child) = running.remove(&task_run_id) {
///             child.kill().await.map_err(|e| SchedulerError::ExecutionError(e.to_string()))?;
///         }
///         Ok(())
///     }
///
///     async fn is_running(&self, task_run_id: i64) -> SchedulerResult<bool> {
///         let running = self.running_tasks.lock().await;
///         Ok(running.contains_key(&task_run_id))
///     }
/// }
/// ```
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// 执行任务 - 增强版接口
    ///
    /// 使用完整的执行上下文执行任务，提供更丰富的执行环境信息。
    ///
    /// # 参数
    ///
    /// * `context` - 任务执行上下文，包含任务信息、参数、环境变量等
    ///
    /// # 返回值
    ///
    /// 成功时返回 `TaskResult`，包含执行结果和输出信息。
    /// 失败时返回 `SchedulerError`。
    ///
    /// # 错误
    ///
    /// * `ValidationError` - 任务参数验证失败
    /// * `ExecutionError` - 任务执行过程中出错
    /// * `TimeoutError` - 任务执行超时
    /// * `ResourceError` - 资源不足或限制
    ///
    /// # 实现注意事项
    ///
    /// - 应该尊重 `context.timeout_seconds` 中的超时设置
    /// - 应该遵守 `context.resource_limits` 中的资源限制
    /// - 执行过程中应该定期检查取消信号
    /// - 应该提供详细的错误信息以便调试
    async fn execute_task(
        &self,
        context: &TaskExecutionContextTrait,
    ) -> SchedulerResult<TaskResult>;

    /// 执行任务 - 向后兼容接口
    ///
    /// 为了保持向后兼容性提供的简化接口。内部会将 `TaskRun` 转换为
    /// `TaskExecutionContextTrait` 后调用 `execute_task` 方法。
    ///
    /// # 参数
    ///
    /// * `task_run` - 任务运行实例
    ///
    /// # 返回值
    ///
    /// 返回任务执行结果，与 `execute_task` 相同。
    ///
    /// # 注意
    ///
    /// 新的实现应该优先使用 `execute_task` 方法，此方法主要用于兼容旧代码。
    async fn execute(&self, task_run: &TaskRun) -> SchedulerResult<TaskResult> {
        // 默认实现，从TaskRun中提取参数构建上下文
        let task_info = task_run
            .result
            .as_ref()
            .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let parameters = task_info
            .get("parameters")
            .and_then(|p| p.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let task_type = task_info
            .get("task_type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown")
            .to_string();

        let context = TaskExecutionContextTrait {
            task_run: task_run.clone(),
            task_type,
            parameters,
            timeout_seconds: 300, // 默认5分钟
            environment: HashMap::new(),
            working_directory: None,
            resource_limits: ResourceLimits::default(),
        };

        self.execute_task(&context).await
    }

    /// 检查是否支持指定的任务类型
    ///
    /// 判断当前执行器是否能够处理指定类型的任务。执行器注册表
    /// 使用此方法来选择合适的执行器处理任务。
    ///
    /// # 参数
    ///
    /// * `task_type` - 要检查的任务类型
    ///
    /// # 返回值
    ///
    /// 如果支持返回 `true`，否则返回 `false`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// // Python执行器只支持python类型的任务
    /// fn supports_task_type(&self, task_type: &str) -> bool {
    ///     match task_type {
    ///         "python" | "py" | "script" => true,
    ///         _ => false,
    ///     }
    /// }
    /// ```
    fn supports_task_type(&self, task_type: &str) -> bool;

    /// 获取执行器名称
    ///
    /// 返回执行器的唯一标识名称，用于在执行器注册表中识别和管理执行器。
    ///
    /// # 返回值
    ///
    /// 执行器的名称字符串引用。
    ///
    /// # 约定
    ///
    /// 名称应该具有描述性并且在系统中唯一，建议使用格式：
    /// - `{type}-executor` (如: `python-executor`)
    /// - `{vendor}-{type}` (如: `docker-python`)
    fn name(&self) -> &str;

    /// 获取执行器版本
    ///
    /// 返回执行器的版本号，用于版本管理和兼容性检查。
    ///
    /// # 返回值
    ///
    /// 版本号字符串，默认返回 "1.0.0"。
    ///
    /// # 版本格式
    ///
    /// 建议使用语义化版本格式 (SemVer)：`MAJOR.MINOR.PATCH`
    fn version(&self) -> &str {
        "1.0.0"
    }

    /// 获取执行器描述
    ///
    /// 返回执行器的详细描述信息，用于文档和用户界面显示。
    ///
    /// # 返回值
    ///
    /// 执行器的描述字符串，默认返回通用描述。
    fn description(&self) -> &str {
        "Generic task executor"
    }

    /// 获取支持的任务类型列表
    fn supported_task_types(&self) -> Vec<String> {
        vec![]
    }

    /// 取消正在执行的任务
    async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()>;

    /// 检查任务是否仍在运行
    async fn is_running(&self, task_run_id: i64) -> SchedulerResult<bool>;

    /// 获取执行器状态信息
    async fn get_status(&self) -> SchedulerResult<ExecutorStatus> {
        Ok(ExecutorStatus {
            name: self.name().to_string(),
            version: self.version().to_string(),
            healthy: true,
            running_tasks: 0,
            supported_task_types: self.supported_task_types(),
            last_health_check: Utc::now(),
            metadata: HashMap::new(),
        })
    }

    /// 健康检查
    async fn health_check(&self) -> SchedulerResult<bool> {
        Ok(true)
    }

    /// 预热执行器（可选实现）
    async fn warm_up(&self) -> SchedulerResult<()> {
        Ok(())
    }

    /// 清理资源（可选实现）
    async fn cleanup(&self) -> SchedulerResult<()> {
        Ok(())
    }
}

/// 执行器工厂trait - 用于创建执行器实例
/// 修复: 返回Arc而不Box以保持一致性
#[async_trait]
pub trait ExecutorFactory: Send + Sync {
    /// 创建执行器实例
    async fn create_executor(
        &self,
        executor_type: &str,
        config: &ExecutorConfig,
    ) -> SchedulerResult<Arc<dyn TaskExecutor>>;

    /// 获取支持的执行器类型
    fn supported_types(&self) -> Vec<String>;

    /// 验证执行器配置
    fn validate_config(&self, executor_type: &str, config: &ExecutorConfig) -> SchedulerResult<()>;
}

/// 执行器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// 配置参数
    pub parameters: HashMap<String, serde_json::Value>,
    /// 资源限制
    pub resource_limits: ResourceLimits,
    /// 是否启用
    pub enabled: bool,
    /// 最大并发数
    pub max_concurrent_tasks: Option<u32>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            parameters: HashMap::new(),
            resource_limits: ResourceLimits::default(),
            enabled: true,
            max_concurrent_tasks: None,
        }
    }
}

/// 执行器注册表trait - 管理执行器生命周期（异步版本）
/// 修复: 统一使用Arc<dyn TaskExecutor>以实现线程安全共享
#[async_trait]
pub trait ExecutorRegistry: Send + Sync {
    /// 注册执行器 - 使用Arc确保线程安全
    async fn register(
        &mut self,
        name: String,
        executor: Arc<dyn TaskExecutor>,
    ) -> SchedulerResult<()>;

    /// 获取执行器(返回Arc包装以支持异步)
    async fn get(&self, name: &str) -> Option<Arc<dyn TaskExecutor>>;

    /// 获取所有执行器名称
    async fn list_executors(&self) -> Vec<String>;

    /// 移除执行器
    async fn unregister(&mut self, name: &str) -> SchedulerResult<bool>;

    /// 清空所有执行器
    async fn clear(&mut self);

    /// 检查执行器是否存在
    async fn contains(&self, name: &str) -> bool;

    /// 获取执行器数量
    async fn count(&self) -> usize;

    /// 获取所有执行器的状态
    async fn get_all_status(&self) -> SchedulerResult<HashMap<String, ExecutorStatus>>;

    /// 健康检查所有执行器
    async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>>;

    /// 获取支持指定任务类型的执行器
    async fn get_by_task_type(
        &self,
        task_type: &str,
    ) -> SchedulerResult<Vec<Arc<dyn TaskExecutor>>>;
}
