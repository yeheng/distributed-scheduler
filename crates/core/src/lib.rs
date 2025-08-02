pub mod circuit_breaker;
pub mod config;
pub mod container;
pub mod error_handling;
pub mod errors;
pub mod executor_registry;
pub mod logging;
pub mod models;
pub mod service_interfaces;
pub mod service_layer;
pub mod services;
pub mod traits;

pub use circuit_breaker::*;
pub use config::*;
pub use container::*;
pub use error_handling::*;
pub use executor_registry::*;
pub use logging::*;
// Re-export only specific items from models to avoid conflicts
pub use models::{Task, TaskResult, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus, Message};
// Re-export legacy service_layer items for backward compatibility
pub use service_layer::{
    AuditLogService as LegacyAuditLogService, 
    ComponentHealth, 
    HealthStatus,
    MonitoringService as LegacyMonitoringService, 
    PerformanceMetrics, 
    SchedulerService as LegacySchedulerService, 
    SchedulerStats, 
    ServiceFactory,
    SystemHealth, 
    TaskDispatchService as LegacyTaskDispatchService, 
    TimeRange as LegacyTimeRange, 
    WorkerManagementService as LegacyWorkerManagementService,
};
// Re-export new modular services (primary interface)
// Re-export new modular services with explicit naming to avoid conflicts
pub use services::{
    // Config services
    AuditEvent, AuditLogService, AuditQuery, AuditQueryService, AuditResult, AuditStats,
    ConfigChange, ConfigWatcher, ConfigurationQueryService, ConfigurationReloadService, 
    ConfigurationService, ConfigurationServiceExt, ExportFormat, TimeRange,
    // Task services
    DispatchStats, TaskControlService, TaskDispatchService,
    TaskHistoryService, TaskRetryService, TaskRunManagementService, TaskSchedulingService,
    // Worker services  
    WorkerHealthService, WorkerHeartbeat, WorkerLoadStats, WorkerQueryService,
    WorkerRegistrationService, WorkerSelectionService,
    // Monitoring services (avoiding duplicates with legacy imports)
    Alert, AlertCondition, AlertLevel, AlertManagementService, AlertRule,
    EventRecord, EventRecordingService, HealthCheckService, MetricRecord,
    MetricsCollectionService, PerformanceMonitoringService, RealtimeStats,
    ResourceUsage,
};
// Re-export traits (excluding conflicting ones)
pub use traits::{
    ExecutorRegistry, ExecutorStatus, MessageQueue, ResourceLimits, TaskExecutor, 
    TaskExecutionContextTrait, WorkerServiceTrait,
};
pub use errors::*;

/// 统一的Result类型
pub type SchedulerResult<T> = std::result::Result<T, SchedulerError>;
