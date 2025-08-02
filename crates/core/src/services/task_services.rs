use crate::{
    models::{Task, TaskRun, TaskRunStatus},
    SchedulerResult,
};
use async_trait::async_trait;

/// 任务控制服务接口
///
/// 专注于任务的生命周期管理和控制操作
#[async_trait]
pub trait TaskControlService: Send + Sync {
    /// 手动触发任务
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;

    /// 暂停任务
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;

    /// 恢复任务
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;
}

/// 任务运行管理服务接口
///
/// 专注于任务运行实例的管理
#[async_trait]
pub trait TaskRunManagementService: Send + Sync {
    /// 重启任务运行实例
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;

    /// 中止任务运行实例
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;

    /// 批量取消任务的所有运行实例
    async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize>;

    /// 检查任务是否有运行中的实例
    async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool>;
}

/// 任务历史查询服务接口
///
/// 专注于任务执行历史的查询
#[async_trait]
pub trait TaskHistoryService: Send + Sync {
    /// 获取任务的最近执行历史
    async fn get_recent_executions(
        &self,
        task_id: i64,
        limit: usize,
    ) -> SchedulerResult<Vec<TaskRun>>;

    /// 获取任务运行详情
    async fn get_task_run_details(&self, task_run_id: i64) -> SchedulerResult<Option<TaskRun>>;
}

/// 调度器服务接口
///
/// 专注于任务调度功能
#[async_trait]
pub trait SchedulerService: Send + Sync {
    /// 启动调度器
    async fn start(&self) -> SchedulerResult<()>;

    /// 停止调度器
    async fn stop(&self) -> SchedulerResult<()>;

    /// 检查调度器状态
    async fn is_running(&self) -> bool;
}

/// 任务调度管理服务接口
///
/// 专注于任务的调度操作
#[async_trait]
pub trait TaskSchedulingService: Send + Sync {
    /// 调度单个任务
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<()>;

    /// 批量调度任务
    async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()>;

    /// 重新加载调度配置
    async fn reload_config(&self) -> SchedulerResult<()>;
}

/// 任务分发服务接口
///
/// 专注于任务分发到Worker
#[async_trait]
pub trait TaskDispatchService: Send + Sync {
    /// 分发任务到Worker
    async fn dispatch_task(&self, task_run: &TaskRun, worker_id: &str) -> SchedulerResult<()>;

    /// 批量分发任务
    async fn dispatch_tasks(&self, dispatches: &[(TaskRun, String)]) -> SchedulerResult<()>;

    /// 处理任务状态更新
    async fn handle_status_update(
        &self,
        task_run_id: i64,
        status: TaskRunStatus,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;
}

/// 任务重试服务接口
///
/// 专注于失败任务的重试逻辑
#[async_trait]
pub trait TaskRetryService: Send + Sync {
    /// 重新分发失败的任务
    async fn redispatch_failed_tasks(&self) -> SchedulerResult<usize>;

    /// 重试特定任务
    async fn retry_task(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
}

/// 调度器统计信息
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    /// 总任务数
    pub total_tasks: i64,
    /// 活跃任务数
    pub active_tasks: i64,
    /// 正在运行的任务实例数
    pub running_task_runs: i64,
    /// 待处理的任务实例数
    pub pending_task_runs: i64,
    /// 调度器运行时间（秒）
    pub uptime_seconds: u64,
    /// 最后调度时间
    pub last_schedule_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// 分发统计信息
#[derive(Debug, Clone)]
pub struct DispatchStats {
    /// 总分发数
    pub total_dispatched: i64,
    /// 成功分发数
    pub successful_dispatched: i64,
    /// 失败分发数
    pub failed_dispatched: i64,
    /// 重新分发数
    pub redispatched: i64,
    /// 平均分发时间（毫秒）
    pub avg_dispatch_time_ms: f64,
}
