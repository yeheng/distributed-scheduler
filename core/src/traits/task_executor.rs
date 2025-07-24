use async_trait::async_trait;

use crate::{models::{TaskRun, TaskResult}, Result};

/// 任务执行器接口
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// 执行任务
    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult>;
    
    /// 检查是否支持指定的任务类型
    fn supports_task_type(&self, task_type: &str) -> bool;
    
    /// 获取执行器名称
    fn name(&self) -> &str;
    
    /// 取消正在执行的任务
    async fn cancel(&self, task_run_id: i64) -> Result<()>;
    
    /// 检查任务是否仍在运行
    async fn is_running(&self, task_run_id: i64) -> Result<bool>;
}

/// 任务执行上下文
#[derive(Debug, Clone)]
pub struct TaskExecutionContext {
    pub task_run_id: i64,
    pub task_id: i64,
    pub task_name: String,
    pub task_type: String,
    pub parameters: serde_json::Value,
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub worker_id: String,
}