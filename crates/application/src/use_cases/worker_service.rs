// Worker service implementation placeholder
use crate::ports::service_interfaces::MessageQueue;
use crate::scheduler::WorkerService;
use chrono::Utc;
use scheduler_domain::entities::{
    TaskRun, TaskRunStatus, WorkerHeartbeat, WorkerInfo, WorkerStatus,
};
use scheduler_common::TaskType;
use scheduler_domain::events::TaskStatusUpdate;
use scheduler_domain::repositories::TaskRunRepository;
use scheduler_domain::repositories::WorkerRepository;
use scheduler_errors::SchedulerResult;
use std::collections::HashMap;
use std::env::VarError;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

pub struct WorkerServiceImpl {
    worker_id: String,
    worker_repository: Arc<dyn WorkerRepository>,
    task_run_repository: Arc<dyn TaskRunRepository>,
    message_queue: Arc<dyn MessageQueue>,
    running_tasks: Arc<RwLock<HashMap<i64, TaskRun>>>,
    supported_task_types: Vec<TaskType>,
    max_concurrent_tasks: i32,
    current_task_count: Arc<RwLock<i32>>,
}

impl WorkerServiceImpl {
    pub fn new(
        worker_id: String,
        worker_repository: Arc<dyn WorkerRepository>,
        task_run_repository: Arc<dyn TaskRunRepository>,
        message_queue: Arc<dyn MessageQueue>,
        supported_task_types: Vec<TaskType>,
        max_concurrent_tasks: i32,
    ) -> Self {
        Self {
            worker_id,
            worker_repository,
            task_run_repository,
            message_queue,
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
            supported_task_types,
            max_concurrent_tasks,
            current_task_count: Arc::new(RwLock::new(0)),
        }
    }

    async fn execute_task(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        let start_time = Utc::now();

        info!(
            "开始执行任务: task_run_id={}, task_id={}",
            task_run.id, task_run.task_id
        );

        // 更新任务状态为运行中
        self.task_run_repository
            .update_status(task_run.id, TaskRunStatus::Running, Some(&self.worker_id))
            .await?;

        // 添加到运行中的任务列表
        {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.insert(task_run.id, task_run.clone());
        }

        let result = self.execute_task_logic(task_run).await;

        // 从运行中的任务列表移除
        {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.remove(&task_run.id);
        }

        let completion_time = Utc::now();
        let execution_duration = (completion_time - start_time).num_milliseconds();

        match result {
            Ok(task_result) => {
                info!(
                    "任务执行成功: task_run_id={}, 执行时间={}ms",
                    task_run.id, execution_duration
                );

                self.task_run_repository
                    .update_result(task_run.id, Some(&task_result), None)
                    .await?;

                self.task_run_repository
                    .update_status(task_run.id, TaskRunStatus::Completed, Some(&self.worker_id))
                    .await?;

                // 发送状态更新消息
                let status_update = TaskStatusUpdate {
                    task_run_id: task_run.id,
                    status: TaskRunStatus::Completed,
                    worker_id: self.worker_id.clone(),
                    result: Some(task_result),
                    error_message: None,
                    timestamp: completion_time,
                };

                if let Err(e) = self.send_status_update(status_update).await {
                    warn!("发送任务完成状态更新失败: {}", e);
                }
            }
            Err(e) => {
                error!("任务执行失败: task_run_id={}, 错误: {}", task_run.id, e);

                self.task_run_repository
                    .update_result(task_run.id, None, Some(&format!("任务执行失败: {e}")))
                    .await?;

                self.task_run_repository
                    .update_status(task_run.id, TaskRunStatus::Failed, Some(&self.worker_id))
                    .await?;

                // 发送状态更新消息
                let status_update = TaskStatusUpdate {
                    task_run_id: task_run.id,
                    status: TaskRunStatus::Failed,
                    worker_id: self.worker_id.clone(),
                    result: None,
                    error_message: Some(format!("任务执行失败: {e}")),
                    timestamp: completion_time,
                };

                if let Err(e) = self.send_status_update(status_update).await {
                    warn!("发送任务失败状态更新失败: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn execute_task_logic(&self, task_run: &TaskRun) -> SchedulerResult<String> {
        // 这里应该根据任务类型执行具体的任务逻辑
        // 目前返回模拟结果
        let result = format!("Task {} executed successfully", task_run.task_id);
        debug!("任务执行逻辑完成: {}", result);
        Ok(result)
    }

    async fn poll_tasks_from_queue(&self) -> SchedulerResult<Vec<TaskRun>> {
        let messages = self
            .message_queue
            .consume_messages(&format!("worker.{}", self.worker_id))
            .await?;

        let mut task_runs = Vec::new();
        for message in messages {
            if let Some(task_run) = self.extract_task_run_from_message(&message) {
                task_runs.push(task_run);
                // 确认消息已处理
                self.message_queue.ack_message(&message.id).await?;
            }
        }

        Ok(task_runs)
    }

    fn extract_task_run_from_message(
        &self,
        message: &scheduler_domain::entities::Message,
    ) -> Option<TaskRun> {
        // 这里应该从消息中提取任务运行信息
        // 目前返回None表示需要实现消息解析逻辑
        warn!("消息解析逻辑未实现，跳过消息: {}", message.id);
        None
    }
}

#[async_trait::async_trait]
impl WorkerService for WorkerServiceImpl {
    #[instrument(skip(self))]
    async fn start(&self) -> SchedulerResult<()> {
        info!("启动Worker服务: {}", self.worker_id);

        // 注册Worker
        let worker_info = WorkerInfo {
            id: self.worker_id.clone(),
            hostname: get_hostname().unwrap_or_else(|_| "unknown".to_string()),
            ip_address: get_local_ip().unwrap_or_else(|_| "127.0.0.1".to_string()),
            supported_task_types: self.supported_task_types.clone(),
            max_concurrent_tasks: self.max_concurrent_tasks,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
        };

        self.worker_repository.register(&worker_info).await?;

        info!("Worker服务启动成功: {}", self.worker_id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop(&self) -> SchedulerResult<()> {
        info!("停止Worker服务: {}", self.worker_id);

        // 取消所有运行中的任务
        let running_tasks = self.get_running_tasks().await;
        for task_run in running_tasks {
            if let Err(e) = self.cancel_task(task_run.id).await {
                warn!("取消任务失败: task_run_id={}, 错误: {}", task_run.id, e);
            }
        }

        // 更新Worker状态为离线
        self.worker_repository
            .update_status(&self.worker_id, WorkerStatus::Down)
            .await?;

        info!("Worker服务已停止: {}", self.worker_id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()> {
        debug!("Worker开始轮询任务: {}", self.worker_id);

        // 检查是否可以接受新任务
        let current_count = self.get_current_task_count().await;
        if current_count >= self.max_concurrent_tasks {
            debug!(
                "Worker已达到最大并发任务数: {}/{}",
                current_count, self.max_concurrent_tasks
            );
            return Ok(());
        }

        // 从消息队列获取任务
        let task_runs = self.poll_tasks_from_queue().await?;

        // 执行任务
        for task_run in task_runs {
            // TODO: 需要实现基于任务类型的检查，当前跳过检查直接执行
            if let Err(e) = self.execute_task(&task_run).await {
                error!("执行任务失败: task_run_id={}, 错误: {}", task_run.id, e);
            }
        }

        Ok(())
    }

    #[instrument(skip(self, update))]
    async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()> {
        debug!(
            "发送状态更新: task_run_id={}, status={:?}",
            update.task_run_id, update.status
        );

        // 创建状态更新消息
        let status_message = scheduler_domain::entities::Message::status_update(
            scheduler_domain::entities::StatusUpdateMessage {
                task_run_id: update.task_run_id,
                status: update.status,
                worker_id: update.worker_id,
                result: update
                    .result
                    .map(|r| scheduler_domain::entities::TaskResult {
                        success: update.status == TaskRunStatus::Completed,
                        output: Some(r),
                        error_message: None,
                        exit_code: Some(0),
                        execution_time_ms: 0,
                    }),
                error_message: update.error_message,
                timestamp: update.timestamp,
            },
        );

        // 发送到状态队列
        self.message_queue
            .publish_message("status_updates", &status_message)
            .await?;

        Ok(())
    }

    async fn get_current_task_count(&self) -> i32 {
        let count = *self.current_task_count.read().await;
        debug!("当前任务数: {}", count);
        count
    }

    async fn can_accept_task(&self, task_type: &TaskType) -> bool {
        let current_count = self.get_current_task_count().await;
        let can_accept = current_count < self.max_concurrent_tasks
            && self.supported_task_types.contains(task_type);

        debug!("检查是否可接受任务: task_type={:?}, current_count={}, max_concurrent={}, supported_types={:?}, can_accept={}", 
               task_type, current_count, self.max_concurrent_tasks, self.supported_task_types, can_accept);

        can_accept
    }

    #[instrument(skip(self, task_run_id))]
    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()> {
        info!("取消任务: task_run_id={}", task_run_id);

        // 检查任务是否在运行
        if self.is_task_running(task_run_id).await {
            // 从运行中的任务列表移除
            {
                let mut running_tasks = self.running_tasks.write().await;
                running_tasks.remove(&task_run_id);
            }

            // 更新任务状态
            self.task_run_repository
                .update_status(task_run_id, TaskRunStatus::Failed, Some(&self.worker_id))
                .await?;

            // 更新当前任务数
            let mut count = self.current_task_count.write().await;
            *count = count.saturating_sub(1);
        }

        info!("任务已取消: task_run_id={}", task_run_id);
        Ok(())
    }

    async fn get_running_tasks(&self) -> Vec<TaskRun> {
        let running_tasks = self.running_tasks.read().await;
        let tasks: Vec<TaskRun> = running_tasks.values().cloned().collect();
        debug!("获取运行中的任务: {} 个", tasks.len());
        tasks
    }

    async fn is_task_running(&self, task_run_id: i64) -> bool {
        let running_tasks = self.running_tasks.read().await;
        let is_running = running_tasks.contains_key(&task_run_id);
        debug!(
            "检查任务是否在运行: task_run_id={}, running={}",
            task_run_id, is_running
        );
        is_running
    }

    #[instrument(skip(self))]
    async fn send_heartbeat(&self) -> SchedulerResult<()> {
        debug!("发送心跳: {}", self.worker_id);

        let current_task_count = self.get_current_task_count().await;
        let heartbeat = WorkerHeartbeat {
            worker_id: self.worker_id.clone(),
            current_task_count,
            system_load: Some(get_system_load().unwrap_or(0.0)),
            memory_usage_mb: Some(get_memory_usage().unwrap_or(0)),
            timestamp: Utc::now(),
        };

        // 更新数据库中的心跳
        self.worker_repository
            .update_heartbeat(
                &self.worker_id,
                heartbeat.timestamp,
                heartbeat.current_task_count,
            )
            .await?;

        // 发送心跳消息
        let heartbeat_message = scheduler_domain::entities::Message::worker_heartbeat(
            scheduler_domain::entities::WorkerHeartbeatMessage {
                worker_id: heartbeat.worker_id,
                current_task_count: heartbeat.current_task_count,
                system_load: heartbeat.system_load,
                memory_usage_mb: heartbeat.memory_usage_mb,
                timestamp: heartbeat.timestamp,
            },
        );

        self.message_queue
            .publish_message("worker_heartbeats", &heartbeat_message)
            .await?;

        debug!("心跳发送成功: {}", self.worker_id);
        Ok(())
    }
}

// Helper functions for system information
fn get_hostname() -> SchedulerResult<String> {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME").or_else(|_| Ok("unknown".to_string())))
        .map_err(|e: VarError| {
            scheduler_errors::SchedulerError::Configuration(format!("Failed to get hostname: {e}"))
        })
}

fn get_local_ip() -> SchedulerResult<String> {
    std::env::var("WORKER_IP")
        .or_else(|_| {
            // Try to get local IP address
            Ok(get_local_ip_address().unwrap_or_else(|_| "127.0.0.1".to_string()))
        })
        .map_err(|e: VarError| {
            scheduler_errors::SchedulerError::Configuration(format!("Failed to get local IP: {e}"))
        })
}

fn get_local_ip_address() -> SchedulerResult<String> {
    use std::net::UdpSocket;

    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.connect("8.8.8.8:80").unwrap();
    let local_addr = socket.local_addr().unwrap();
    Ok(local_addr.ip().to_string())
}

fn get_system_load() -> SchedulerResult<f64> {
    // This is a placeholder - in a real implementation, you would use
    // a crate like `sysinfo` to get actual system load
    #[cfg(target_os = "linux")]
    {
        if let Ok(load) = std::fs::read_to_string("/proc/loadavg") {
            if let Some(load_str) = load.split_whitespace().next() {
                if let Ok(load_val) = load_str.parse::<f64>() {
                    return Ok(load_val);
                }
            }
        }
    }

    // Fallback value
    Ok(0.0)
}

fn get_memory_usage() -> SchedulerResult<u64> {
    // This is a placeholder - in a real implementation, you would use
    // a crate like `sysinfo` to get actual memory usage
    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for line in meminfo.lines() {
                if line.starts_with("MemAvailable:") {
                    if let Some(parts) = line.split_whitespace().collect::<Vec<_>>().get(1) {
                        if let Ok(kb) = parts.parse::<u64>() {
                            return Ok(kb / 1024); // Convert to MB
                        }
                    }
                }
            }
        }
    }

    // Fallback value
    Ok(0)
}
