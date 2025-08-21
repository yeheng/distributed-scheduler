pub mod alerting;
pub mod cross_component_tracer;
pub mod dashboard;
pub mod message_tracing;
pub mod metrics_collector;
pub mod performance_testing;
pub mod structured_logger;
pub mod task_tracer;
pub mod telemetry_setup;

pub use alerting::{
    Alert, AlertCondition, AlertManager, AlertRule, AlertSeverity, EmailNotificationChannel,
    LogNotificationChannel, NotificationChannel, WebhookNotificationChannel,
};
pub use cross_component_tracer::CrossComponentTracer;
pub use dashboard::{
    AlertSummary, BenchmarkSummary, DashboardData, HealthLevel, HealthStatus, PerformanceDashboard,
    PerformanceMetricsData, SystemMetrics,
};
pub use message_tracing::MessageTracingExt;
pub use metrics_collector::{MetricsCollector, PerformanceMetrics};
pub use performance_testing::{
    BenchmarkResult, CriterionCondition, CriterionResult, PerformanceBenchmark,
    PerformanceCriterion, PerformanceRegressionTester, PerformanceTrend, RegressionReport,
    RegressionSeverity, TrendDirection,
};
pub use structured_logger::{LoggingConfig, StructuredLogger};
pub use task_tracer::TaskTracer;
pub use telemetry_setup::{
    init_logging_and_tracing, init_metrics, init_observability, init_structured_logging,
    init_tracing, shutdown_observability,
};
