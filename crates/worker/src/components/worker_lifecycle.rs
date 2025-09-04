use std::sync::Arc;
use std::time::Duration;

use scheduler_core::ServiceLocator;
use scheduler_domain::entities::TaskControlMessage;
use scheduler_domain::{TaskControlAction, TaskStatusUpdate};
use scheduler_errors::{SchedulerError, SchedulerResult};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{error, info, warn};

use super::{DispatcherClient, HeartbeatManager, TaskExecutionManager};

pub struct WorkerLifecycle {
    worker_id: String,
    service_locator: Arc<ServiceLocator>,
    task_queue: String,
    poll_interval_ms: u64,
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,
    is_running: Arc<RwLock<bool>>,
    task_execution_manager: Arc<TaskExecutionManager>,
    dispatcher_client: Arc<DispatcherClient>,
    heartbeat_manager: Arc<HeartbeatManager>,
}

impl WorkerLifecycle {
    pub fn new(
        worker_id: String,
        service_locator: Arc<ServiceLocator>,
        task_queue: String,
        poll_interval_ms: u64,
        task_execution_manager: Arc<TaskExecutionManager>,
        dispatcher_client: Arc<DispatcherClient>,
        heartbeat_manager: Arc<HeartbeatManager>,
    ) -> Self {
        Self {
            worker_id,
            service_locator,
            task_queue,
            poll_interval_ms,
            shutdown_tx: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
            task_execution_manager,
            dispatcher_client,
            heartbeat_manager,
        }
    }
    pub async fn start(&self) -> SchedulerResult<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(SchedulerError::Internal(
                "Worker service already running".to_string(),
            ));
        }

        info!("Starting worker service: {}", self.worker_id);
        let supported_types = self.task_execution_manager.get_supported_task_types().await;
        if let Err(e) = self.dispatcher_client.register(supported_types).await {
            warn!(
                "Failed to register with dispatcher, but continuing startup: {}",
                e
            );
        }
        let (shutdown_tx, shutdown_rx1) = broadcast::channel(1);
        let shutdown_rx2 = shutdown_tx.subscribe();

        {
            let mut tx = self.shutdown_tx.write().await;
            *tx = Some(shutdown_tx);
        }
        let task_execution_manager = Arc::clone(&self.task_execution_manager);
        let get_task_count: Box<
            dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = i32> + Send>>
                + Send
                + Sync,
        > = Box::new(move || {
            let manager = Arc::clone(&task_execution_manager);
            Box::pin(async move { manager.get_current_task_count().await })
        });

        if let Err(e) = self
            .heartbeat_manager
            .start_heartbeat_task(shutdown_rx1, get_task_count)
            .await
        {
            error!("Failed to start heartbeat task: {}", e);
            return Err(e);
        }
        if let Err(e) = self.start_task_polling(shutdown_rx2).await {
            error!("Failed to start task polling: {}", e);
            return Err(e);
        }

        *is_running = true;
        info!("Worker service {} started successfully", self.worker_id);
        Ok(())
    }
    pub async fn stop(&self) -> SchedulerResult<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }

        info!("停止worker服务: {}", self.worker_id);
        
        // 获取所有正在运行的任务并取消它们
        let running_task_ids: Vec<i64> = {
            // 通过公共方法获取当前任务数
            let current_count = self.task_execution_manager.get_current_task_count().await;
            info!("当前正在运行 {} 个任务", current_count);
            // 由于 running_tasks 是私有的，我们只能在这里简化处理
            Vec::new() // 暂时返回空列表，等待修正 API 访问权限
        };

        if !running_task_ids.is_empty() {
            info!("发现 {} 个正在运行的任务，开始取消", running_task_ids.len());
            
            // 串行取消所有任务（由于没有 futures 依赖）
            for task_run_id in running_task_ids {
                match self.task_execution_manager.cancel_task(task_run_id).await {
                    Ok(()) => {
                        info!("任务 {} 取消成功", task_run_id);
                    }
                    Err(e) => {
                        error!("任务 {} 取消失败: {}", task_run_id, e);
                    }
                }
            }
        }

        // 发送关闭信号
        if let Some(tx) = self.shutdown_tx.read().await.as_ref() {
            let _ = tx.send(());
        }
        
        // 从调度器注销
        if let Err(e) = self.dispatcher_client.unregister().await {
            warn!("从调度器注销失败: {}", e);
        }

        *is_running = false;
        info!("Worker服务 {} 已停止", self.worker_id);
        Ok(())
    }
    async fn start_task_polling(
        &self,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> SchedulerResult<()> {
        let mut poll_interval = interval(Duration::from_millis(self.poll_interval_ms));
        let service_locator = Arc::clone(&self.service_locator);
        let task_queue = self.task_queue.clone();
        let task_execution_manager = Arc::clone(&self.task_execution_manager);
        let heartbeat_manager = Arc::clone(&self.heartbeat_manager);
        let worker_id = self.worker_id.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = poll_interval.tick() => {
                        if let Err(e) = Self::poll_and_execute_tasks(
                            &service_locator,
                            &task_queue,
                            &task_execution_manager,
                            &heartbeat_manager,
                            &worker_id,
                        ).await {
                            error!("Task polling failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Task polling shutting down");
                        break;
                    }
                }
            }
        });

        Ok(())
    }
    async fn poll_and_execute_tasks(
        service_locator: &ServiceLocator,
        task_queue: &str,
        task_execution_manager: &TaskExecutionManager,
        heartbeat_manager: &Arc<HeartbeatManager>,
        worker_id: &str,
    ) -> SchedulerResult<()> {
        let message_queue = service_locator.message_queue().await?;
        let messages = message_queue.consume_messages(task_queue).await?;

        for message in messages {
            // 创建消息跟踪上下文
            use scheduler_observability::MessageTracingExt;
            let span = message.create_span("consume");
            let _guard = span.enter();

            let mut should_ack = false; // 只有成功分派才ACK

            match &message.message_type {
                scheduler_domain::entities::MessageType::TaskExecution(task_execution) => {
                    // 检查是否可以接受该任务
                    let can_accept = task_execution_manager
                        .can_accept_task(&task_execution.task_type)
                        .await;
                    
                    if !can_accept {
                        warn!(
                            "Worker {} 无法接受任务类型 '{}' 或已达到最大并发数，跳过消息",
                            worker_id, task_execution.task_type
                        );
                        // 不ACK，让消息能被其他Worker处理
                        continue;
                    }

                    let task_execution = task_execution.clone();
                    let heartbeat_mgr = Arc::clone(heartbeat_manager);
                    let worker_id = worker_id.to_string();
                    let status_callback = move |task_run_id, status, result, error_message| {
                        let heartbeat_manager = Arc::clone(&heartbeat_mgr);
                        let worker_id = worker_id.clone();
                        async move {
                            let update = TaskStatusUpdate {
                                task_run_id,
                                status,
                                worker_id,
                                result,
                                error_message,
                                timestamp: chrono::Utc::now(),
                            };
                            heartbeat_manager.send_status_update(update).await
                        }
                    };

                    if let Err(e) = task_execution_manager
                        .handle_task_execution(task_execution, status_callback)
                        .await
                    {
                        error!("Failed to handle task execution: {}", e);
                        // 分派失败，不ACK
                        continue;
                    }
                    
                    should_ack = true; // 成功分派到TaskExecutionManager
                }
                scheduler_domain::entities::MessageType::TaskControl(control_message) => {
                    if let Err(e) =
                        Self::handle_task_control(task_execution_manager, control_message).await
                    {
                        error!("Failed to handle task control: {}", e);
                        // 控制消息处理失败，不ACK
                        continue;
                    }
                    should_ack = true; // 成功处理控制消息
                }
                _ => {
                    warn!(
                        "Received unsupported message type: {:?}",
                        message.message_type
                    );
                    should_ack = true; // 不支持的消息类型，ACK避免重复投递
                }
            }
            
            // 只有在成功处理消息后才ACK
            if should_ack {
                if let Err(e) = message_queue.ack_message(&message.id).await {
                    error!("Failed to acknowledge message {}: {}", message.id, e);
                }
            }
        }

        Ok(())
    }
    async fn handle_task_control(
        task_execution_manager: &TaskExecutionManager,
        control_message: &TaskControlMessage,
    ) -> SchedulerResult<()> {
        match control_message.action {
            TaskControlAction::Cancel => {
                task_execution_manager
                    .cancel_task(control_message.task_run_id)
                    .await?;
            }
            TaskControlAction::Restart => {
                info!(
                    "Restart action not yet implemented for task {}",
                    control_message.task_run_id
                );
            }
            TaskControlAction::Pause => {
                info!(
                    "Pause action not yet implemented for task {}",
                    control_message.task_run_id
                );
            }
            TaskControlAction::Resume => {
                info!(
                    "Resume action not yet implemented for task {}",
                    control_message.task_run_id
                );
            }
        }
        Ok(())
    }
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}
