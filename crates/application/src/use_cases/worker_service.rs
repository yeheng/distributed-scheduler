// Worker service implementation - 协调器模式
use crate::scheduler::WorkerService;
use scheduler_common::TaskType;
use scheduler_domain::entities::{TaskRun, WorkerInfo, WorkerStatus};
use scheduler_domain::events::TaskStatusUpdate;
use scheduler_domain::repositories::WorkerRepository;
use scheduler_errors::SchedulerResult;
use std::sync::Arc;
use tracing::{info, instrument};

pub struct WorkerServiceImpl {
    worker_id: String,
    worker_repository: Arc<dyn WorkerRepository>,
    supported_task_types: Vec<TaskType>,
    max_concurrent_tasks: i32,
}

impl WorkerServiceImpl {
    pub fn new(
        worker_id: String,
        worker_repository: Arc<dyn WorkerRepository>,
        supported_task_types: Vec<TaskType>,
        max_concurrent_tasks: i32,
    ) -> Self {
        Self {
            worker_id,
            worker_repository,
            supported_task_types,
            max_concurrent_tasks,
        }
    }
}

#[async_trait::async_trait]
impl WorkerService for WorkerServiceImpl {
    #[instrument(skip(self))]
    async fn start(&self) -> SchedulerResult<()> {
        info!("启动Worker服务: {}", self.worker_id);

        // 注册Worker到数据库 - 实际的任务执行由 WorkerLifecycle 和 TaskExecutionManager 处理
        let worker_info = WorkerInfo {
            id: self.worker_id.clone(),
            hostname: get_hostname().unwrap_or_else(|_| "unknown".to_string()),
            ip_address: get_local_ip().unwrap_or_else(|_| "127.0.0.1".to_string()),
            supported_task_types: self.supported_task_types.clone(),
            max_concurrent_tasks: self.max_concurrent_tasks,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: chrono::Utc::now(),
            registered_at: chrono::Utc::now(),
        };

        self.worker_repository.register(&worker_info).await?;
        info!("Worker服务启动成功: {}", self.worker_id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop(&self) -> SchedulerResult<()> {
        info!("停止Worker服务: {}", self.worker_id);

        // 更新Worker状态为离线 - 实际的任务取消由 WorkerLifecycle 处理
        self.worker_repository
            .update_status(&self.worker_id, WorkerStatus::Down)
            .await?;

        info!("Worker服务已停止: {}", self.worker_id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()> {
        // 注意：实际的任务轮询和执行由 WorkerLifecycle 和 TaskExecutionManager 处理
        // 这个方法在新架构中不再需要具体实现
        info!("Worker {} 轮询任务由 WorkerLifecycle 组件处理", self.worker_id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn send_status_update(&self, _update: TaskStatusUpdate) -> SchedulerResult<()> {
        // 状态更新由 HeartbeatManager 组件处理
        info!("状态更新由 HeartbeatManager 组件处理");
        Ok(())
    }

    async fn get_current_task_count(&self) -> i32 {
        // 当前任务数由 TaskExecutionManager 管理
        0
    }

    async fn can_accept_task(&self, task_type: &TaskType) -> bool {
        // 任务接受逻辑由 TaskExecutionManager 处理
        self.supported_task_types.contains(task_type)
    }

    async fn cancel_task(&self, _task_run_id: i64) -> SchedulerResult<()> {
        // 任务取消由 TaskExecutionManager 处理
        info!("任务取消由 TaskExecutionManager 处理");
        Ok(())
    }

    async fn get_running_tasks(&self) -> Vec<TaskRun> {
        // 运行中任务列表由 TaskExecutionManager 管理
        Vec::new()
    }

    async fn is_task_running(&self, _task_run_id: i64) -> bool {
        // 任务运行状态由 TaskExecutionManager 管理
        false
    }

    #[instrument(skip(self))]
    async fn send_heartbeat(&self) -> SchedulerResult<()> {
        // 心跳发送由 HeartbeatManager 组件处理
        info!("心跳发送由 HeartbeatManager 组件处理");
        Ok(())
    }
}

// 系统信息获取帮助函数
fn get_hostname() -> SchedulerResult<String> {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .or_else(|_| Ok("unknown".to_string()))
        .map_err(|_e: std::env::VarError| {
            scheduler_errors::SchedulerError::Configuration(
                "Failed to get hostname".to_string()
            )
        })
}

fn get_local_ip() -> SchedulerResult<String> {
    std::env::var("WORKER_IP")
        .or_else(|_| {
            use std::net::UdpSocket;
            if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
                if socket.connect("8.8.8.8:80").is_ok() {
                    if let Ok(local_addr) = socket.local_addr() {
                        return Ok(local_addr.ip().to_string());
                    }
                }
            }
            Ok("127.0.0.1".to_string())
        })
        .map_err(|_e: std::env::VarError| {
            scheduler_errors::SchedulerError::Configuration(
                "Failed to get local IP".to_string()
            )
        })
}
