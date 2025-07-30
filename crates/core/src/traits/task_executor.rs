use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    errors::Result,
    models::{TaskResult, TaskRun},
};

/// 任务执行上下文 - 增强版本，包含更多执行信息
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

/// 任务执行器接口 - 增强版本
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// 执行任务 - 新的增强接口
    async fn execute_task(&self, context: &TaskExecutionContextTrait) -> Result<TaskResult>;

    /// 执行任务 - 保持向后兼容的原有接口
    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult> {
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
    fn supports_task_type(&self, task_type: &str) -> bool;

    /// 获取执行器名称
    fn name(&self) -> &str;

    /// 获取执行器版本
    fn version(&self) -> &str {
        "1.0.0"
    }

    /// 获取执行器描述
    fn description(&self) -> &str {
        "Generic task executor"
    }

    /// 获取支持的任务类型列表
    fn supported_task_types(&self) -> Vec<String> {
        vec![]
    }

    /// 取消正在执行的任务
    async fn cancel(&self, task_run_id: i64) -> Result<()>;

    /// 检查任务是否仍在运行
    async fn is_running(&self, task_run_id: i64) -> Result<bool>;

    /// 获取执行器状态信息
    async fn get_status(&self) -> Result<ExecutorStatus> {
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
    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }

    /// 预热执行器（可选实现）
    async fn warm_up(&self) -> Result<()> {
        Ok(())
    }

    /// 清理资源（可选实现）
    async fn cleanup(&self) -> Result<()> {
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
    ) -> Result<Arc<dyn TaskExecutor>>;

    /// 获取支持的执行器类型
    fn supported_types(&self) -> Vec<String>;

    /// 验证执行器配置
    fn validate_config(&self, executor_type: &str, config: &ExecutorConfig) -> Result<()>;
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
    async fn register(&mut self, name: String, executor: Arc<dyn TaskExecutor>) -> Result<()>;

    /// 获取执行器(返回Arc包装以支持异步)
    async fn get(&self, name: &str) -> Option<Arc<dyn TaskExecutor>>;

    /// 获取所有执行器名称
    async fn list_executors(&self) -> Vec<String>;

    /// 移除执行器
    async fn unregister(&mut self, name: &str) -> Result<bool>;

    /// 清空所有执行器
    async fn clear(&mut self);

    /// 检查执行器是否存在
    async fn contains(&self, name: &str) -> bool;

    /// 获取执行器数量
    async fn count(&self) -> usize;

    /// 获取所有执行器的状态
    async fn get_all_status(&self) -> Result<HashMap<String, ExecutorStatus>>;

    /// 健康检查所有执行器
    async fn health_check_all(&self) -> Result<HashMap<String, bool>>;

    /// 获取支持指定任务类型的执行器
    async fn get_by_task_type(&self, task_type: &str) -> Result<Vec<Arc<dyn TaskExecutor>>>;
}
