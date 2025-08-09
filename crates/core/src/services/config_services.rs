use super::TimeRange;
use crate::SchedulerResult;
use async_trait::async_trait;

#[async_trait]
pub trait ConfigurationService: Send + Sync {
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<serde_json::Value>>;
    async fn set_config_value(&self, key: &str, value: &serde_json::Value) -> SchedulerResult<()>;
    async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;
}

#[async_trait]
pub trait ConfigurationQueryService: Send + Sync {
    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;
    async fn search_config_keys(&self, pattern: &str) -> SchedulerResult<Vec<String>>;
}

#[async_trait]
pub trait ConfigurationReloadService: Send + Sync {
    async fn reload_config(&self) -> SchedulerResult<()>;
    async fn watch_config(&self, key: &str) -> SchedulerResult<Box<dyn ConfigWatcher>>;
}

#[async_trait]
pub trait AuditLogService: Send + Sync {
    async fn log_event(&self, event: &AuditEvent) -> SchedulerResult<()>;
    async fn log_events(&self, events: &[AuditEvent]) -> SchedulerResult<()>;
}

#[async_trait]
pub trait AuditQueryService: Send + Sync {
    async fn query_events(&self, query: &AuditQuery) -> SchedulerResult<Vec<AuditEvent>>;
    async fn get_audit_stats(&self, time_range: TimeRange) -> SchedulerResult<AuditStats>;
    async fn export_events(
        &self,
        query: &AuditQuery,
        format: ExportFormat,
    ) -> SchedulerResult<Vec<u8>>;
}

pub trait ConfigurationServiceExt {
    fn get_config<T>(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = SchedulerResult<Option<T>>> + Send
    where
        T: serde::de::DeserializeOwned;
    fn set_config<T>(
        &self,
        key: &str,
        value: &T,
    ) -> impl std::future::Future<Output = SchedulerResult<()>> + Send
    where
        T: serde::Serialize + std::marker::Sync;
}

impl<C: ConfigurationService> ConfigurationServiceExt for C {
    async fn get_config<T>(&self, key: &str) -> SchedulerResult<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        match self.get_config_value(key).await? {
            Some(value) => {
                let typed_value: T = serde_json::from_value(value).map_err(|e| {
                    crate::errors::SchedulerError::Internal(format!("配置反序列化失败: {e}"))
                })?;
                Ok(Some(typed_value))
            }
            None => Ok(None),
        }
    }

    async fn set_config<T>(&self, key: &str, value: &T) -> SchedulerResult<()>
    where
        T: serde::Serialize + std::marker::Sync,
    {
        let json_value = serde_json::to_value(value)
            .map_err(|e| crate::errors::SchedulerError::Internal(format!("配置序列化失败: {e}")))?;
        self.set_config_value(key, &json_value).await
    }
}

#[async_trait]
pub trait ConfigWatcher: Send + Sync {
    async fn wait_for_change(&mut self) -> SchedulerResult<ConfigChange>;
    async fn stop(&mut self) -> SchedulerResult<()>;
}

#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub key: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub changed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub id: String,
    pub event_type: String,
    pub user_id: Option<String>,
    pub resource_id: Option<String>,
    pub action: String,
    pub result: AuditResult,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AuditResult {
    Success,
    Failure,
    Error,
}

#[derive(Debug, Clone)]
pub struct AuditQuery {
    pub time_range: Option<TimeRange>,
    pub event_types: Vec<String>,
    pub user_ids: Vec<String>,
    pub resource_ids: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct AuditStats {
    pub total_events: i64,
    pub success_events: i64,
    pub failure_events: i64,
    pub error_events: i64,
    pub events_by_type: std::collections::HashMap<String, i64>,
    pub events_by_user: std::collections::HashMap<String, i64>,
}

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Excel,
    Pdf,
}
