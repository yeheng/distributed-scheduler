use super::TimeRange;
use crate::SchedulerResult;
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait MetricsCollectionService: Send + Sync {
    async fn record_metric(
        &self,
        name: &str,
        value: f64,
        tags: &HashMap<String, String>,
    ) -> SchedulerResult<()>;
    async fn record_metrics(&self, metrics: &[MetricRecord]) -> SchedulerResult<()>;
}

#[async_trait]
pub trait EventRecordingService: Send + Sync {
    async fn record_event(&self, event_type: &str, data: &serde_json::Value)
        -> SchedulerResult<()>;
    async fn record_events(&self, events: &[EventRecord]) -> SchedulerResult<()>;
}

#[async_trait]
pub trait HealthCheckService: Send + Sync {
    async fn get_system_health(&self) -> SchedulerResult<SystemHealth>;
    async fn check_component_health(&self, component: &str) -> SchedulerResult<ComponentHealth>;
}

#[async_trait]
pub trait PerformanceMonitoringService: Send + Sync {
    async fn get_performance_metrics(
        &self,
        time_range: TimeRange,
    ) -> SchedulerResult<PerformanceMetrics>;
    async fn get_realtime_stats(&self) -> SchedulerResult<RealtimeStats>;
}

#[async_trait]
pub trait AlertManagementService: Send + Sync {
    async fn set_alert_rule(&self, rule: &AlertRule) -> SchedulerResult<()>;
    async fn check_alerts(&self) -> SchedulerResult<Vec<Alert>>;
    async fn resolve_alert(&self, alert_id: &str) -> SchedulerResult<()>;
}

#[derive(Debug, Clone)]
pub struct MetricRecord {
    pub name: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct EventRecord {
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub components: HashMap<String, ComponentHealth>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub task_throughput: f64,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone)]
pub struct RealtimeStats {
    pub active_tasks: i64,
    pub tasks_per_minute: f64,
    pub current_load: f64,
}

#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_usage_mb: u64,
    pub network_io_mb: u64,
}

#[derive(Debug, Clone)]
pub struct AlertRule {
    pub id: String,
    pub name: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub duration_seconds: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub id: String,
    pub rule_id: String,
    pub level: AlertLevel,
    pub message: String,
    pub triggered_at: chrono::DateTime<chrono::Utc>,
    pub resolved: bool,
}

#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}
