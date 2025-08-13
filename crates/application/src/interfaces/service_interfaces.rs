// Service interfaces moved from core/service_interfaces.rs
use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use scheduler_foundation::SchedulerResult;
use scheduler_domain::entities::{Task, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};

pub mod task_services {
    use super::*;
    #[async_trait]
    pub trait TaskControlService: Send + Sync {
        async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;
        async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;
        async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;
        async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;
        async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;
        async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize>;
        async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool>;
        async fn get_recent_executions(
            &self,
            task_id: i64,
            limit: usize,
        ) -> SchedulerResult<Vec<TaskRun>>;
    }
    #[async_trait]
    pub trait TaskSchedulerService: Send + Sync {
        async fn start(&self) -> SchedulerResult<()>;
        async fn stop(&self) -> SchedulerResult<()>;
        async fn schedule_task(&self, task: &Task) -> SchedulerResult<()>;
        async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()>;
        async fn scan_and_schedule(&self) -> SchedulerResult<Vec<TaskRun>>;
        async fn check_dependencies(&self, task: &Task) -> SchedulerResult<bool>;
        async fn create_task_run(&self, task: &Task) -> SchedulerResult<TaskRun>;
        async fn dispatch_to_queue(&self, task_run: &TaskRun) -> SchedulerResult<()>;
        async fn is_running(&self) -> bool;
        async fn get_stats(&self) -> SchedulerResult<SchedulerStats>;
        async fn reload_config(&self) -> SchedulerResult<()>;
    }
    #[async_trait]
    pub trait TaskDispatchService: Send + Sync {
        async fn dispatch_task(&self, task_run: &TaskRun, worker_id: &str) -> SchedulerResult<()>;
        async fn dispatch_tasks(&self, dispatches: &[(TaskRun, String)]) -> SchedulerResult<()>;
        async fn handle_status_update(
            &self,
            task_run_id: i64,
            status: TaskRunStatus,
            error_message: Option<String>,
        ) -> SchedulerResult<()>;
        async fn redispatch_failed_tasks(&self) -> SchedulerResult<usize>;
        async fn get_dispatch_stats(&self) -> SchedulerResult<DispatchStats>;
    }
}

pub mod worker_services {
    use super::*;
    #[async_trait]
    pub trait WorkerManagementService: Send + Sync {
        async fn register_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()>;
        async fn unregister_worker(&self, worker_id: &str) -> SchedulerResult<()>;
        async fn update_worker_status(
            &self,
            worker_id: &str,
            status: WorkerStatus,
        ) -> SchedulerResult<()>;
        async fn get_active_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;
        async fn get_worker_details(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;
        async fn check_worker_health(&self, worker_id: &str) -> SchedulerResult<bool>;
        async fn get_worker_load_stats(&self) -> SchedulerResult<HashMap<String, WorkerLoadStats>>;
        async fn select_best_worker(&self, task_type: &str) -> SchedulerResult<Option<String>>;
        async fn process_heartbeat(
            &self,
            worker_id: &str,
            heartbeat_data: &WorkerHeartbeat,
        ) -> SchedulerResult<()>;
    }
    #[async_trait]
    pub trait WorkerHealthService: Send + Sync {
        async fn perform_health_check(&self, worker_id: &str)
            -> SchedulerResult<HealthCheckResult>;
        async fn get_worker_health_status(
            &self,
            worker_id: &str,
        ) -> SchedulerResult<WorkerHealthStatus>;
        async fn update_health_metrics(
            &self,
            worker_id: &str,
            metrics: WorkerHealthMetrics,
        ) -> SchedulerResult<()>;
        async fn get_unhealthy_workers(&self) -> SchedulerResult<Vec<String>>;
        async fn handle_worker_failure(&self, worker_id: &str) -> SchedulerResult<()>;
    }
}

pub mod system_services {
    use super::*;
    #[async_trait]
    pub trait ConfigurationService: Send + Sync {
        async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<Value>>;
        async fn set_config_value(&self, key: &str, value: &Value) -> SchedulerResult<()>;
        async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;
        async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;
        async fn reload_config(&self) -> SchedulerResult<()>;
        async fn watch_config(&self, key: &str) -> SchedulerResult<Box<dyn ConfigWatcher>>;
    }
    #[async_trait]
    pub trait MonitoringService: Send + Sync {
        async fn record_metric(
            &self,
            name: &str,
            value: f64,
            tags: &HashMap<String, String>,
        ) -> SchedulerResult<()>;
        async fn record_event(&self, event_type: &str, data: &Value) -> SchedulerResult<()>;
        async fn get_system_health(&self) -> SchedulerResult<SystemHealth>;
        async fn get_performance_metrics(
            &self,
            time_range: TimeRange,
        ) -> SchedulerResult<PerformanceMetrics>;
        async fn set_alert_rule(&self, rule: &AlertRule) -> SchedulerResult<()>;
        async fn check_alerts(&self) -> SchedulerResult<Vec<Alert>>;
    }
    #[async_trait]
    pub trait AuditService: Send + Sync {
        async fn log_event(&self, event: &AuditEvent) -> SchedulerResult<()>;
        async fn query_events(&self, query: &AuditQuery) -> SchedulerResult<Vec<AuditEvent>>;
        async fn get_audit_stats(&self, time_range: TimeRange) -> SchedulerResult<AuditStats>;
        async fn export_events(
            &self,
            query: &AuditQuery,
            format: ExportFormat,
        ) -> SchedulerResult<Vec<u8>>;
    }
}

#[async_trait]
pub trait ServiceFactory: Send + Sync {
    async fn create_task_control_service(
        &self,
    ) -> SchedulerResult<Box<dyn task_services::TaskControlService>>;
    async fn create_task_scheduler_service(
        &self,
    ) -> SchedulerResult<Box<dyn task_services::TaskSchedulerService>>;
    async fn create_task_dispatch_service(
        &self,
    ) -> SchedulerResult<Box<dyn task_services::TaskDispatchService>>;
    async fn create_worker_management_service(
        &self,
    ) -> SchedulerResult<Box<dyn worker_services::WorkerManagementService>>;
    async fn create_worker_health_service(
        &self,
    ) -> SchedulerResult<Box<dyn worker_services::WorkerHealthService>>;
    async fn create_configuration_service(
        &self,
    ) -> SchedulerResult<Box<dyn system_services::ConfigurationService>>;
    async fn create_monitoring_service(
        &self,
    ) -> SchedulerResult<Box<dyn system_services::MonitoringService>>;
    async fn create_audit_service(&self)
        -> SchedulerResult<Box<dyn system_services::AuditService>>;
}

// Data structures
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub total_tasks: i64,
    pub active_tasks: i64,
    pub running_task_runs: i64,
    pub pending_task_runs: i64,
    pub uptime_seconds: u64,
    pub last_schedule_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct DispatchStats {
    pub total_dispatched: i64,
    pub successful_dispatched: i64,
    pub failed_dispatched: i64,
    pub redispatched: i64,
    pub avg_dispatch_time_ms: f64,
}

#[derive(Debug, Clone)]
pub struct WorkerLoadStats {
    pub worker_id: String,
    pub current_task_count: i32,
    pub max_concurrent_tasks: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WorkerHeartbeat {
    pub current_task_count: i32,
    pub system_load: Option<f64>,
    pub memory_usage_mb: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub is_healthy: bool,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WorkerHealthStatus {
    pub worker_id: String,
    pub status: HealthStatus,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
    pub metrics: WorkerHealthMetrics,
}

#[derive(Debug, Clone)]
pub struct WorkerHealthMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_usage_mb: u64,
    pub network_io_mb: u64,
    pub task_success_rate: f64,
    pub avg_task_execution_time_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub components: HashMap<String, ComponentHealth>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub task_throughput: f64,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
    pub resource_usage: ResourceUsage,
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
    pub triggered_at: DateTime<Utc>,
    pub resolved: bool,
}

#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub id: String,
    pub event_type: String,
    pub user_id: Option<String>,
    pub resource_id: Option<String>,
    pub action: String,
    pub result: AuditResult,
    pub data: Value,
    pub timestamp: DateTime<Utc>,
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
    pub events_by_type: HashMap<String, i64>,
    pub events_by_user: HashMap<String, i64>,
}

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Excel,
    Pdf,
}

#[async_trait]
pub trait MessageQueue: Send + Sync {
    async fn publish_message(&self, queue: &str, message: &scheduler_domain::entities::Message) -> SchedulerResult<()>;
    async fn consume_messages(&self, queue: &str) -> SchedulerResult<Vec<scheduler_domain::entities::Message>>;
    async fn ack_message(&self, message_id: &str) -> SchedulerResult<()>;
    async fn nack_message(&self, message_id: &str, requeue: bool) -> SchedulerResult<()>;
    async fn create_queue(&self, queue: &str, durable: bool) -> SchedulerResult<()>;
    async fn delete_queue(&self, queue: &str) -> SchedulerResult<()>;
    async fn get_queue_size(&self, queue: &str) -> SchedulerResult<u32>;
    async fn purge_queue(&self, queue: &str) -> SchedulerResult<()>;
}

#[async_trait]
pub trait ConfigWatcher: Send + Sync {
    async fn wait_for_change(&mut self) -> SchedulerResult<ConfigChange>;
    async fn stop(&mut self) -> SchedulerResult<()>;
}

#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub key: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub changed_at: DateTime<Utc>,
}
