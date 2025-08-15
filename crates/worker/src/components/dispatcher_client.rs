use reqwest;
use scheduler_domain::entities::WorkerInfo;
use scheduler_foundation::{SchedulerError, SchedulerResult};
use serde_json::json;
use tracing::{debug, error, info, warn};

pub struct DispatcherClient {
    dispatcher_url: Option<String>,
    worker_id: String,
    hostname: String,
    ip_address: String,
    http_client: reqwest::Client,
}

impl DispatcherClient {
    pub fn new(
        dispatcher_url: Option<String>,
        worker_id: String,
        hostname: String,
        ip_address: String,
    ) -> Self {
        Self {
            dispatcher_url,
            worker_id,
            hostname,
            ip_address,
            http_client: reqwest::Client::new(),
        }
    }
    pub async fn register(&self, supported_task_types: Vec<String>) -> SchedulerResult<()> {
        let Some(ref dispatcher_url) = self.dispatcher_url else {
            debug!("No dispatcher URL configured, skipping registration");
            return Ok(());
        };

        let worker_info = WorkerInfo {
            id: self.worker_id.clone(),
            hostname: self.hostname.clone(),
            ip_address: self.ip_address.clone(),
            supported_task_types,
            max_concurrent_tasks: 5, // This should be configurable
            current_task_count: 0,
            status: scheduler_foundation::WorkerStatus::Alive,
            last_heartbeat: chrono::Utc::now(),
            registered_at: chrono::Utc::now(),
        };

        let url = format!("{dispatcher_url}/api/v1/workers/register");

        match self.http_client.post(&url).json(&worker_info).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!(
                        "Successfully registered worker {} with dispatcher",
                        self.worker_id
                    );
                    Ok(())
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    error!("Failed to register worker: HTTP {} - {}", status, body);
                    Err(SchedulerError::Internal(format!(
                        "Worker registration failed: HTTP {status} - {body}"
                    )))
                }
            }
            Err(e) => {
                error!("Failed to connect to dispatcher for registration: {}", e);
                Err(SchedulerError::Internal(format!(
                    "Dispatcher connection error: {e}"
                )))
            }
        }
    }
    pub async fn send_heartbeat(&self, current_task_count: i32) -> SchedulerResult<()> {
        let Some(ref dispatcher_url) = self.dispatcher_url else {
            debug!("No dispatcher URL configured, skipping heartbeat");
            return Ok(());
        };

        let heartbeat_data = json!({
            "worker_id": self.worker_id,
            "current_task_count": current_task_count,
            "status": "alive",
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        let url = format!(
            "{}/api/v1/workers/{}/heartbeat",
            dispatcher_url, self.worker_id
        );

        match self
            .http_client
            .post(&url)
            .json(&heartbeat_data)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    debug!("Heartbeat sent successfully for worker {}", self.worker_id);
                    Ok(())
                } else {
                    let status = response.status();
                    warn!("Heartbeat failed: HTTP {}", status);
                    Err(SchedulerError::Internal(format!(
                        "Heartbeat failed: HTTP {status}"
                    )))
                }
            }
            Err(e) => {
                warn!("Failed to send heartbeat: {}", e);
                Err(SchedulerError::Internal(format!(
                    "Heartbeat connection error: {e}"
                )))
            }
        }
    }
    pub async fn unregister(&self) -> SchedulerResult<()> {
        let Some(ref dispatcher_url) = self.dispatcher_url else {
            debug!("No dispatcher URL configured, skipping unregistration");
            return Ok(());
        };

        let url = format!("{}/api/v1/workers/{}", dispatcher_url, self.worker_id);

        match self.http_client.delete(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!(
                        "Successfully unregistered worker {} from dispatcher",
                        self.worker_id
                    );
                    Ok(())
                } else {
                    let status = response.status();
                    warn!("Failed to unregister worker: HTTP {}", status);
                    Ok(())
                }
            }
            Err(e) => {
                warn!("Failed to unregister worker: {}", e);
                Ok(())
            }
        }
    }
    pub fn is_configured(&self) -> bool {
        self.dispatcher_url.is_some()
    }
    pub fn get_dispatcher_url(&self) -> &Option<String> {
        &self.dispatcher_url
    }
}
