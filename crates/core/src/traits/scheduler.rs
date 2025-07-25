use async_trait::async_trait;

use crate::{
    models::{Task, TaskRun, WorkerInfo},
    Result,
};

/// 任务调度服务接口
#[async_trait]
pub trait TaskSchedulerService: Send + Sync {
    /// 扫描并调度任务
    async fn scan_and_schedule(&self) -> Result<Vec<TaskRun>>;

    /// 检查任务依赖
    async fn check_dependencies(&self, task: &Task) -> Result<bool>;

    /// 创建任务运行实例
    async fn create_task_run(&self, task: &Task) -> Result<TaskRun>;

    /// 分发任务到消息队列
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> Result<()>;
}

/// 状态监听服务接口
#[async_trait]
pub trait StateListenerService: Send + Sync {
    /// 监听状态更新
    async fn listen_for_updates(&self) -> Result<()>;

    /// 处理状态更新
    async fn process_status_update(
        &self,
        task_run_id: i64,
        status: crate::models::TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> Result<()>;
}

/// 任务控制服务接口
#[async_trait]
pub trait TaskControlService: Send + Sync {
    /// 手动触发任务
    async fn trigger_task(&self, task_id: i64) -> Result<TaskRun>;

    /// 暂停任务
    async fn pause_task(&self, task_id: i64) -> Result<()>;

    /// 恢复任务
    async fn resume_task(&self, task_id: i64) -> Result<()>;

    /// 重启任务运行实例
    async fn restart_task_run(&self, task_run_id: i64) -> Result<TaskRun>;

    /// 中止任务运行实例
    async fn abort_task_run(&self, task_run_id: i64) -> Result<()>;
}

/// 任务分派策略接口
#[async_trait]
pub trait TaskDispatchStrategy: Send + Sync {
    /// 选择Worker执行任务
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> Result<Option<String>>;

    /// 获取策略名称
    fn name(&self) -> &str;
}

/// Worker服务接口
#[async_trait]
pub trait WorkerService: Send + Sync {
    /// 启动Worker服务
    async fn start(&self) -> Result<()>;

    /// 停止Worker服务
    async fn stop(&self) -> Result<()>;

    /// 轮询并执行任务
    async fn poll_and_execute_tasks(&self) -> Result<()>;

    /// 发送状态更新
    async fn send_status_update(
        &self,
        task_run_id: i64,
        status: crate::models::TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> Result<()>;

    /// 发送心跳
    async fn send_heartbeat(&self) -> Result<()>;
}
