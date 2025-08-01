use async_trait::async_trait;

use crate::{
    models::{Task, TaskRun, TaskStatusUpdate, WorkerInfo},
    SchedulerResult,
};

/// 任务调度服务接口
#[async_trait]
pub trait TaskSchedulerService: Send + Sync {
    /// 扫描并调度任务
    async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>>;

    /// 检查任务依赖
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool>;

    /// 创建任务运行实例
    async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun>;

    /// 分发任务到消息队列
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()>;
}

/// 状态监听服务接口
#[async_trait]
pub trait StateListenerService: Send + Sync {
    /// 监听状态更新
    async fn listen_for_updates(&self) -> SchedulerResult<()>;

    /// 处理状态更新
    async fn process_status_update(
        &self,
        task_run_id: i64,
        status: crate::models::TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;
}

/// 任务控制服务接口
#[async_trait]
pub trait TaskControlService: Send + Sync {
    /// 手动触发任务
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;

    /// 暂停任务
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;

    /// 恢复任务
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;

    /// 重启任务运行实例
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;

    /// 中止任务运行实例
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
}

/// 任务分派策略接口
#[async_trait]
pub trait TaskDispatchStrategy: Send + Sync {
    /// 选择Worker执行任务
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>>;

    /// 获取策略名称
    fn name(&self) -> &str;
}

/// Worker服务接口
#[async_trait]
pub trait WorkerServiceTrait: Send + Sync {
    /// 启动Worker服务
    async fn start(&self) -> SchedulerResult<()>;

    /// 停止Worker服务
    async fn stop(&self) -> SchedulerResult<()>;

    /// 轮询并执行任务
    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()>;

    /// 发送状态更新
    async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()>;

    /// 获取当前运行的任务数量
    async fn get_current_task_count(&self) -> i32;

    /// 检查是否可以接受新任务
    async fn can_accept_task(&self, task_type: &str) -> bool;

    /// 取消正在运行的任务
    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()>;

    /// 获取正在运行的任务列表
    async fn get_running_tasks(&self) -> Vec<TaskRun>;

    /// 检查任务是否正在运行
    async fn is_task_running(&self, task_run_id: i64) -> bool;

    /// 发送心跳
    async fn send_heartbeat(&self) -> SchedulerResult<()>;
}
