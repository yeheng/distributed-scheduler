use std::sync::Arc;
use std::time::Duration;

use scheduler_core::{SchedulerResult, ServiceLocator, traits::TaskStatusUpdate};
use scheduler_domain::entities::{Message, StatusUpdateMessage, TaskResult, TaskRunStatus};
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{error, info};

use super::DispatcherClient;

pub struct HeartbeatManager {
    worker_id: String,
    service_locator: Arc<ServiceLocator>,
    status_queue: String,
    heartbeat_interval_seconds: u64,
    dispatcher_client: Arc<DispatcherClient>,
}

impl HeartbeatManager {
    pub fn new(
        worker_id: String,
        service_locator: Arc<ServiceLocator>,
        status_queue: String,
        heartbeat_interval_seconds: u64,
        dispatcher_client: Arc<DispatcherClient>,
    ) -> Self {
        Self {
            worker_id,
            service_locator,
            status_queue,
            heartbeat_interval_seconds,
            dispatcher_client,
        }
    }
    pub async fn start_heartbeat_task(
        &self,
        mut shutdown_rx: broadcast::Receiver<()>,
        get_task_count: Box<
            dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = i32> + Send>>
                + Send
                + Sync,
        >,
    ) -> SchedulerResult<()> {
        let mut heartbeat_interval = interval(Duration::from_secs(self.heartbeat_interval_seconds));
        let dispatcher_client = Arc::clone(&self.dispatcher_client);
        let service_locator = Arc::clone(&self.service_locator);
        let worker_id = self.worker_id.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = heartbeat_interval.tick() => {
                        if let Err(e) = Self::send_message_queue_heartbeat(&service_locator, &worker_id).await {
                            error!("Failed to send message queue heartbeat: {}", e);
                        }
                        let current_task_count = get_task_count().await;
                        if let Err(e) = dispatcher_client.send_heartbeat(current_task_count).await {
                            error!("Failed to send dispatcher heartbeat: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Heartbeat task shutting down");
                        break;
                    }
                }
            }
        });

        Ok(())
    }
    pub async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()> {
        let message_queue = self.service_locator.message_queue().await?;
        let status_message = StatusUpdateMessage {
            task_run_id: update.task_run_id,
            status: update.status,
            worker_id: update.worker_id,
            result: update.result.map(|r| TaskResult {
                success: true, // Assume success if result is provided
                output: Some(r),
                error_message: update.error_message.clone(),
                exit_code: Some(0),
                execution_time_ms: 0, // This should be calculated
            }),
            error_message: update.error_message,
            timestamp: update.timestamp,
        };

        let message = Message::status_update(status_message);
        message_queue
            .publish_message(&self.status_queue, &message)
            .await
    }
    async fn send_message_queue_heartbeat(
        service_locator: &ServiceLocator,
        worker_id: &str,
    ) -> SchedulerResult<()> {
        let message_queue = service_locator.message_queue().await?;

        let heartbeat_data = StatusUpdateMessage {
            task_run_id: -1, // Special marker for heartbeat
            status: TaskRunStatus::Running,
            worker_id: worker_id.to_string(),
            result: Some(TaskResult {
                success: true,
                output: Some("heartbeat".to_string()),
                error_message: None,
                exit_code: Some(0),
                execution_time_ms: 0,
            }),
            error_message: None,
            timestamp: chrono::Utc::now(),
        };

        let message = Message::status_update(heartbeat_data);
        message_queue
            .publish_message("worker_heartbeat", &message)
            .await?;
        Ok(())
    }
}
