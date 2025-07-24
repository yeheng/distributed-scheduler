use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    models::{Task, TaskFilter, TaskRun, TaskRunStatus, WorkerInfo},
    Result,
};

/// 任务仓储接口
#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// 创建任务
    async fn create_task(&self, task: &Task) -> Result<Task>;

    /// 根据ID获取任务
    async fn get_task(&self, id: i64) -> Result<Option<Task>>;

    /// 更新任务
    async fn update_task(&self, task: &Task) -> Result<()>;

    /// 删除任务（软删除，设置为INACTIVE）
    async fn delete_task(&self, id: i64) -> Result<()>;

    /// 获取任务列表
    async fn list_tasks(&self, filter: TaskFilter) -> Result<Vec<Task>>;

    /// 获取可调度的任务
    async fn get_schedulable_tasks(&self, now: DateTime<Utc>) -> Result<Vec<Task>>;

    /// 检查任务依赖是否满足
    async fn check_dependencies(&self, task_id: i64) -> Result<bool>;
}

/// 任务运行仓储接口
#[async_trait]
pub trait TaskRunRepository: Send + Sync {
    /// 创建任务运行实例
    async fn create_task_run(&self, task_run: &TaskRun) -> Result<TaskRun>;

    /// 根据ID获取任务运行实例
    async fn get_task_run(&self, id: i64) -> Result<Option<TaskRun>>;

    /// 更新任务运行状态
    async fn update_task_run_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> Result<()>;

    /// 分配Worker到任务运行实例
    async fn assign_worker(&self, task_run_id: i64, worker_id: &str) -> Result<()>;

    /// 获取待处理的任务运行实例
    async fn get_pending_task_runs(&self, worker_id: Option<&str>) -> Result<Vec<TaskRun>>;

    /// 获取Worker正在运行的任务
    async fn get_running_tasks_for_worker(&self, worker_id: &str) -> Result<Vec<TaskRun>>;

    /// 获取任务的执行历史
    async fn get_task_execution_history(
        &self,
        task_id: i64,
        limit: Option<i64>,
    ) -> Result<Vec<TaskRun>>;

    /// 获取需要重试的失败任务
    async fn get_failed_tasks_for_retry(&self, now: DateTime<Utc>) -> Result<Vec<TaskRun>>;

    /// 获取超时的任务
    async fn get_timeout_tasks(&self, now: DateTime<Utc>) -> Result<Vec<TaskRun>>;
}

/// Worker仓储接口
#[async_trait]
pub trait WorkerRepository: Send + Sync {
    /// 注册Worker
    async fn register_worker(&self, worker: &WorkerInfo) -> Result<()>;

    /// 注销Worker
    async fn unregister_worker(&self, worker_id: &str) -> Result<()>;

    /// 更新Worker心跳
    async fn update_heartbeat(&self, worker_id: &str, timestamp: DateTime<Utc>) -> Result<()>;

    /// 获取所有活跃的Worker
    async fn get_active_workers(&self) -> Result<Vec<WorkerInfo>>;

    /// 根据ID获取Worker
    async fn get_worker(&self, worker_id: &str) -> Result<Option<WorkerInfo>>;

    /// 标记超时的Worker为下线状态
    async fn mark_timeout_workers(&self, timeout_seconds: i64) -> Result<Vec<String>>;

    /// 获取支持特定任务类型的Worker
    async fn get_workers_for_task_type(&self, task_type: &str) -> Result<Vec<WorkerInfo>>;
}
