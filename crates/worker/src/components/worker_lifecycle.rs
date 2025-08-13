use std::sync::Arc;
use std::time::Duration;

use scheduler_foundation::{models::task_status_update::TaskStatusUpdate, SchedulerError, SchedulerResult, ServiceLocator};
use scheduler_domain::entities::TaskControlMessage;
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

        info!("Stopping worker service: {}", self.worker_id);
        if let Some(tx) = self.shutdown_tx.read().await.as_ref() {
            let _ = tx.send(());
        }
        if let Err(e) = self.dispatcher_client.unregister().await {
            warn!("Failed to unregister from dispatcher: {}", e);
        }

        *is_running = false;
        info!("Worker service {} stopped", self.worker_id);
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
            // Create span from message trace context
            use scheduler_observability::MessageTracingExt;
            let span = message.create_span("consume");
            let _guard = span.enter();
            
            match &message.message_type {
                scheduler_domain::entities::MessageType::TaskExecution(task_execution) => {
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
                    }
                }
                scheduler_domain::entities::MessageType::TaskControl(control_message) => {
                    if let Err(e) =
                        Self::handle_task_control(task_execution_manager, control_message).await
                    {
                        error!("Failed to handle task control: {}", e);
                    }
                }
                _ => {
                    warn!(
                        "Received unsupported message type: {:?}",
                        message.message_type
                    );
                }
            }
            if let Err(e) = message_queue.ack_message(&message.id).await {
                error!("Failed to acknowledge message {}: {}", message.id, e);
            }
        }

        Ok(())
    }
    async fn handle_task_control(
        task_execution_manager: &TaskExecutionManager,
        control_message: &TaskControlMessage,
    ) -> SchedulerResult<()> {
        match control_message.action {
            scheduler_domain::entities::TaskControlAction::Cancel => {
                task_execution_manager
                    .cancel_task(control_message.task_run_id)
                    .await?;
            }
            scheduler_domain::entities::TaskControlAction::Restart => {
                info!(
                    "Restart action not yet implemented for task {}",
                    control_message.task_run_id
                );
            }
            scheduler_domain::entities::TaskControlAction::Pause => {
                info!(
                    "Pause action not yet implemented for task {}",
                    control_message.task_run_id
                );
            }
            scheduler_domain::entities::TaskControlAction::Resume => {
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
