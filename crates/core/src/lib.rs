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
pub use models::{Message, Task, TaskResult, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
// Re-export legacy service_layer items for backward compatibility
pub use service_layer::{
    AuditLogService as LegacyAuditLogService, ComponentHealth, HealthStatus,
    MonitoringService as LegacyMonitoringService, PerformanceMetrics,
    SchedulerService as LegacySchedulerService, ServiceFactory, SystemHealth,
    TaskDispatchService as LegacyTaskDispatchService,
    WorkerManagementService as LegacyWorkerManagementService,
};
// Re-export new modular services (primary interface)
// Re-export new modular services with explicit naming to avoid conflicts
pub use services::{
    // Monitoring services (avoiding duplicates with legacy imports)
    Alert,
    AlertCondition,
    AlertLevel,
    AlertManagementService,
    AlertRule,
    // Config services
    AuditEvent,
    AuditLogService,
    AuditQuery,
    AuditQueryService,
    AuditResult,
    AuditStats,
    ConfigChange,
    ConfigWatcher,
    ConfigurationQueryService,
    ConfigurationReloadService,
    ConfigurationService,
    ConfigurationServiceExt,
    // Task services
    DispatchStats,
    EventRecord,
    EventRecordingService,
    ExportFormat,
    HealthCheckService,
    MetricRecord,
    MetricsCollectionService,
    PerformanceMonitoringService,
    RealtimeStats,
    ResourceUsage,
    TaskControlService,
    TaskDispatchService,
    TaskHistoryService,
    TaskRetryService,
    TaskRunManagementService,
    TaskSchedulingService,
    TimeRange,
    // Worker services
    WorkerHealthService,
    WorkerHeartbeat,
    WorkerLoadStats,
    WorkerQueryService,
    WorkerRegistrationService,
    WorkerSelectionService,
};
// Re-export traits (excluding conflicting ones)
pub use errors::*;
pub use traits::{
    ExecutorRegistry, ExecutorStatus, MessageQueue, ResourceLimits, TaskExecutionContextTrait,
    TaskExecutor, WorkerServiceTrait,
};

/// 统一的Result类型
pub type SchedulerResult<T> = std::result::Result<T, SchedulerError>;
