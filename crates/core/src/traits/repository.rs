use crate::errors::Result;
use crate::models::{Task, TaskFilter, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// 任务仓储接口
#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// 创建新任务
    async fn create(&self, task: &Task) -> Result<Task>;

    /// 根据ID获取任务
    async fn get_by_id(&self, id: i64) -> Result<Option<Task>>;

    /// 根据名称获取任务
    async fn get_by_name(&self, name: &str) -> Result<Option<Task>>;

    /// 更新任务
    async fn update(&self, task: &Task) -> Result<()>;

    /// 删除任务
    async fn delete(&self, id: i64) -> Result<()>;

    /// 根据过滤条件查询任务列表
    async fn list(&self, filter: &TaskFilter) -> Result<Vec<Task>>;

    /// 获取所有活跃任务
    async fn get_active_tasks(&self) -> Result<Vec<Task>>;

    /// 获取需要调度的任务（活跃且到达调度时间）
    async fn get_schedulable_tasks(&self, current_time: DateTime<Utc>) -> Result<Vec<Task>>;

    /// 检查任务依赖是否满足
    async fn check_dependencies(&self, task_id: i64) -> Result<bool>;

    /// 获取任务的依赖列表
    async fn get_dependencies(&self, task_id: i64) -> Result<Vec<Task>>;

    /// 批量更新任务状态
    async fn batch_update_status(
        &self,
        task_ids: &[i64],
        status: crate::models::TaskStatus,
    ) -> Result<()>;
}

/// 任务执行实例仓储接口
#[async_trait]
pub trait TaskRunRepository: Send + Sync {
    /// 创建新的任务执行实例
    async fn create(&self, task_run: &TaskRun) -> Result<TaskRun>;

    /// 根据ID获取任务执行实例
    async fn get_by_id(&self, id: i64) -> Result<Option<TaskRun>>;

    /// 更新任务执行实例
    async fn update(&self, task_run: &TaskRun) -> Result<()>;

    /// 删除任务执行实例
    async fn delete(&self, id: i64) -> Result<()>;

    /// 根据任务ID获取执行实例列表
    async fn get_by_task_id(&self, task_id: i64) -> Result<Vec<TaskRun>>;

    /// 根据Worker ID获取执行实例列表
    async fn get_by_worker_id(&self, worker_id: &str) -> Result<Vec<TaskRun>>;

    /// 获取指定状态的任务执行实例
    async fn get_by_status(&self, status: TaskRunStatus) -> Result<Vec<TaskRun>>;

    /// 获取待调度的任务执行实例
    async fn get_pending_runs(&self, limit: Option<i64>) -> Result<Vec<TaskRun>>;

    /// 获取正在运行的任务执行实例
    async fn get_running_runs(&self) -> Result<Vec<TaskRun>>;

    /// 获取超时的任务执行实例
    async fn get_timeout_runs(&self, timeout_seconds: i64) -> Result<Vec<TaskRun>>;

    /// 更新任务执行状态
    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        worker_id: Option<&str>,
    ) -> Result<()>;

    /// 更新任务执行结果
    async fn update_result(
        &self,
        id: i64,
        result: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()>;

    /// 获取任务的最近执行记录
    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> Result<Vec<TaskRun>>;

    /// 获取任务执行统计信息
    async fn get_execution_stats(&self, task_id: i64, days: i32) -> Result<TaskExecutionStats>;

    /// 清理过期的任务执行记录
    async fn cleanup_old_runs(&self, days: i32) -> Result<u64>;

    /// 批量更新任务执行状态
    async fn batch_update_status(&self, run_ids: &[i64], status: TaskRunStatus) -> Result<()>;
}

/// Worker仓储接口
#[async_trait]
pub trait WorkerRepository: Send + Sync {
    /// 注册新的Worker
    async fn register(&self, worker: &WorkerInfo) -> Result<()>;

    /// 注销Worker
    async fn unregister(&self, worker_id: &str) -> Result<()>;

    /// 根据ID获取Worker信息
    async fn get_by_id(&self, worker_id: &str) -> Result<Option<WorkerInfo>>;

    /// 更新Worker信息
    async fn update(&self, worker: &WorkerInfo) -> Result<()>;

    /// 获取所有Worker列表
    async fn list(&self) -> Result<Vec<WorkerInfo>>;

    /// 获取活跃的Worker列表
    async fn get_alive_workers(&self) -> Result<Vec<WorkerInfo>>;

    /// 获取支持指定任务类型的Worker列表
    async fn get_workers_by_task_type(&self, task_type: &str) -> Result<Vec<WorkerInfo>>;

    /// 更新Worker心跳
    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: DateTime<Utc>,
        current_task_count: i32,
    ) -> Result<()>;

    /// 更新Worker状态
    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> Result<()>;

    /// 获取超时的Worker列表
    async fn get_timeout_workers(&self, timeout_seconds: i64) -> Result<Vec<WorkerInfo>>;

    /// 清理离线Worker
    async fn cleanup_offline_workers(&self, timeout_seconds: i64) -> Result<u64>;

    /// 获取Worker负载统计
    async fn get_worker_load_stats(&self) -> Result<Vec<WorkerLoadStats>>;

    /// 批量更新Worker状态
    async fn batch_update_status(&self, worker_ids: &[String], status: WorkerStatus) -> Result<()>;
}

/// 任务执行统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskExecutionStats {
    pub task_id: i64,
    pub total_runs: i64,
    pub successful_runs: i64,
    pub failed_runs: i64,
    pub timeout_runs: i64,
    pub average_execution_time_ms: Option<f64>,
    pub success_rate: f64,
    pub last_execution: Option<DateTime<Utc>>,
}

/// Worker负载统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerLoadStats {
    pub worker_id: String,
    pub current_task_count: i32,
    pub max_concurrent_tasks: i32,
    pub load_percentage: f64,
    pub total_completed_tasks: i64,
    pub total_failed_tasks: i64,
    pub average_task_duration_ms: Option<f64>,
    pub last_heartbeat: DateTime<Utc>,
}

impl TaskExecutionStats {
    /// 计算成功率
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
    pub fn calculate_load_percentage(&mut self) {
        if self.max_concurrent_tasks > 0 {
            self.load_percentage =
                (self.current_task_count as f64 / self.max_concurrent_tasks as f64) * 100.0;
        } else {
            self.load_percentage = 0.0;
        }
    }
}
