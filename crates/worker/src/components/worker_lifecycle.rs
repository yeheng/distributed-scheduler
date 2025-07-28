use std::sync::Arc;
use std::time::Duration;

use scheduler_core::{
    models::{MessageType, TaskControlMessage},
    Result, SchedulerError, ServiceLocator,
};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{error, info, warn};

use super::{DispatcherClient, HeartbeatManager, TaskExecutionManager};

/// Worker lifecycle manager - Handles service start/stop and task polling
/// Follows SRP: Only responsible for service lifecycle management
pub struct WorkerLifecycle {
    worker_id: String,
    service_locator: Arc<ServiceLocator>,
    task_queue: String,
    poll_interval_ms: u64,
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,
    is_running: Arc<RwLock<bool>>,

    // Component dependencies
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

    /// Start the worker service
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(SchedulerError::Internal(
                "Worker service already running".to_string(),
            ));
        }

        info!("Starting worker service: {}", self.worker_id);

        // Register with dispatcher
        let supported_types = self.task_execution_manager.get_supported_task_types().await;
        if let Err(e) = self.dispatcher_client.register(supported_types).await {
            warn!(
                "Failed to register with dispatcher, but continuing startup: {}",
                e
            );
        }

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx1) = broadcast::channel(1);
        let shutdown_rx2 = shutdown_tx.subscribe();

        {
            let mut tx = self.shutdown_tx.write().await;
            *tx = Some(shutdown_tx);
        }

        // Start heartbeat task
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

        // Start task polling
        if let Err(e) = self.start_task_polling(shutdown_rx2).await {
            error!("Failed to start task polling: {}", e);
            return Err(e);
        }

        *is_running = true;
        info!("Worker service {} started successfully", self.worker_id);
        Ok(())
    }

    /// Stop the worker service
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }

        info!("Stopping worker service: {}", self.worker_id);

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.read().await.as_ref() {
            let _ = tx.send(());
        }

        // Unregister from dispatcher
        if let Err(e) = self.dispatcher_client.unregister().await {
            warn!("Failed to unregister from dispatcher: {}", e);
        }

        *is_running = false;
        info!("Worker service {} stopped", self.worker_id);
        Ok(())
    }

    /// Start task polling loop
    async fn start_task_polling(&self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        let mut poll_interval = interval(Duration::from_millis(self.poll_interval_ms));
        let service_locator = Arc::clone(&self.service_locator);
        let task_queue = self.task_queue.clone();
        let task_execution_manager = Arc::clone(&self.task_execution_manager);
        let heartbeat_manager = Arc::clone(&self.heartbeat_manager);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = poll_interval.tick() => {
                        if let Err(e) = Self::poll_and_execute_tasks(
                            &service_locator,
                            &task_queue,
                            &task_execution_manager,
                            &heartbeat_manager,
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

    /// Poll for tasks and execute them
    async fn poll_and_execute_tasks(
        service_locator: &ServiceLocator,
        task_queue: &str,
        task_execution_manager: &TaskExecutionManager,
        heartbeat_manager: &Arc<HeartbeatManager>,
    ) -> Result<()> {
        let message_queue = service_locator.message_queue().await?;
        let messages = message_queue.consume_messages(task_queue).await?;

        for message in messages {
            match &message.message_type {
                MessageType::TaskExecution(task_execution) => {
                    let task_execution = task_execution.clone();
                    let heartbeat_mgr = Arc::clone(heartbeat_manager);

                    // Create callback for status updates
                    let status_callback = move |task_run_id, status, result, error_message| {
                        let heartbeat_manager = Arc::clone(&heartbeat_mgr);
                        async move {
                            let update = scheduler_core::models::TaskStatusUpdate {
                                task_run_id,
                                status,
                                worker_id: "worker".to_string(),
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
                MessageType::TaskControl(control_message) => {
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

            // Acknowledge message
            if let Err(e) = message_queue.ack_message(&message.id).await {
                error!("Failed to acknowledge message {}: {}", message.id, e);
            }
        }

        Ok(())
    }

    /// Handle task control messages
    async fn handle_task_control(
        task_execution_manager: &TaskExecutionManager,
        control_message: &TaskControlMessage,
    ) -> Result<()> {
        match control_message.action {
            scheduler_core::models::TaskControlAction::Cancel => {
                task_execution_manager
                    .cancel_task(control_message.task_run_id)
                    .await?;
            }
            scheduler_core::models::TaskControlAction::Restart => {
                // In a full implementation, this would restart the task
                info!(
                    "Restart action not yet implemented for task {}",
                    control_message.task_run_id
                );
            }
            scheduler_core::models::TaskControlAction::Pause => {
                info!(
                    "Pause action not yet implemented for task {}",
                    control_message.task_run_id
                );
            }
            scheduler_core::models::TaskControlAction::Resume => {
                info!(
                    "Resume action not yet implemented for task {}",
                    control_message.task_run_id
                );
            }
        }
        Ok(())
    }

    /// Check if service is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}
