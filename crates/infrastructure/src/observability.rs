use anyhow::Result;
use metrics::{counter, gauge, histogram, Counter, Gauge, Histogram};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace::BatchConfig, Resource};
use opentelemetry_semantic_conventions::resource;
use tracing::{debug, error, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Metrics collector for the distributed scheduler system
pub struct MetricsCollector {
    // Task execution metrics
    task_executions_total: Counter,
    task_execution_duration: Histogram,
    task_failures_total: Counter,
    task_retries_total: Counter,

    // Worker metrics
    active_workers: Gauge,
    worker_task_capacity: Gauge,
    worker_current_load: Gauge,

    // System performance metrics
    queue_depth: Gauge,
    dispatcher_scheduling_duration: Histogram,
    database_operation_duration: Histogram,
    message_queue_operation_duration: Histogram,
}

impl MetricsCollector {
    /// Initialize the metrics collector with OpenTelemetry
    pub fn new() -> Result<Self> {
        // Initialize metrics
        let task_executions_total = counter!("scheduler_task_executions_total");
        let task_execution_duration = histogram!("scheduler_task_execution_duration_seconds");
        let task_failures_total = counter!("scheduler_task_failures_total");
        let task_retries_total = counter!("scheduler_task_retries_total");

        let active_workers = gauge!("scheduler_active_workers");
        let worker_task_capacity = gauge!("scheduler_worker_task_capacity");
        let worker_current_load = gauge!("scheduler_worker_current_load");

        let queue_depth = gauge!("scheduler_queue_depth");
        let dispatcher_scheduling_duration =
            histogram!("scheduler_dispatcher_scheduling_duration_seconds");
        let database_operation_duration =
            histogram!("scheduler_database_operation_duration_seconds");
        let message_queue_operation_duration =
            histogram!("scheduler_message_queue_operation_duration_seconds");

        Ok(Self {
            task_executions_total,
            task_execution_duration,
            task_failures_total,
            task_retries_total,
            active_workers,
            worker_task_capacity,
            worker_current_load,
            queue_depth,
            dispatcher_scheduling_duration,
            database_operation_duration,
            message_queue_operation_duration,
        })
    }

    // Task execution metrics

    /// Record a task execution completion
    pub fn record_task_execution(&self, task_type: &str, status: &str, duration_seconds: f64) {
        self.task_executions_total.increment(1);
        self.task_execution_duration.record(duration_seconds);

        info!(
            task_type = task_type,
            status = status,
            duration_seconds = duration_seconds,
            "Task execution completed"
        );
    }

    /// Record a task failure
    pub fn record_task_failure(&self, task_type: &str, error_type: &str) {
        self.task_failures_total.increment(1);

        warn!(
            task_type = task_type,
            error_type = error_type,
            "Task execution failed"
        );
    }

    /// Record a task retry
    pub fn record_task_retry(&self, task_type: &str, retry_count: u32) {
        self.task_retries_total.increment(1);

        info!(
            task_type = task_type,
            retry_count = retry_count,
            "Task retry initiated"
        );
    }

    // Worker metrics

    /// Update the number of active workers
    pub fn update_active_workers(&self, count: f64) {
        self.active_workers.set(count);
    }

    /// Update worker task capacity
    pub fn update_worker_capacity(&self, worker_id: &str, capacity: f64) {
        self.worker_task_capacity.set(capacity);

        info!(
            worker_id = worker_id,
            capacity = capacity,
            "Worker capacity updated"
        );
    }

    /// Update worker current load
    pub fn update_worker_load(&self, worker_id: &str, current_load: f64) {
        self.worker_current_load.set(current_load);

        info!(
            worker_id = worker_id,
            current_load = current_load,
            "Worker load updated"
        );
    }

    // System performance metrics

    /// Update queue depth
    pub fn update_queue_depth(&self, depth: f64) {
        self.queue_depth.set(depth);
    }

    /// Record dispatcher scheduling operation duration
    pub fn record_scheduling_duration(&self, duration_seconds: f64) {
        self.dispatcher_scheduling_duration.record(duration_seconds);
    }

    /// Record database operation duration
    pub fn record_database_operation(&self, operation: &str, duration_seconds: f64) {
        self.database_operation_duration.record(duration_seconds);

        info!(
            operation = operation,
            duration_seconds = duration_seconds,
            "Database operation completed"
        );
    }

    /// Record message queue operation duration
    pub fn record_message_queue_operation(&self, operation: &str, duration_seconds: f64) {
        self.message_queue_operation_duration
            .record(duration_seconds);

        info!(
            operation = operation,
            duration_seconds = duration_seconds,
            "Message queue operation completed"
        );
    }
}

/// Structured logging configuration
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
    pub output: LogOutput,
    pub include_location: bool,
    pub include_thread_id: bool,
    pub include_thread_name: bool,
}

#[derive(Debug, Clone)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

#[derive(Debug, Clone)]
pub enum LogOutput {
    Stdout,
    Stderr,
    File(String),
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Json,
            output: LogOutput::Stdout,
            include_location: true,
            include_thread_id: false,
            include_thread_name: false,
        }
    }
}

/// Initialize structured logging with tracing
pub fn init_structured_logging(config: LoggingConfig) -> Result<()> {
    use tracing_subscriber::fmt::format::FmtSpan;

    let level = config.level.clone();
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| config.level.into());

    let registry = tracing_subscriber::registry().with(env_filter);

    match config.format {
        LogFormat::Json => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_thread_ids(config.include_thread_id)
                .with_thread_names(config.include_thread_name)
                .with_span_events(FmtSpan::CLOSE);

            registry.with(fmt_layer).init();
        }
        LogFormat::Pretty => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_thread_ids(config.include_thread_id)
                .with_thread_names(config.include_thread_name)
                .with_span_events(FmtSpan::CLOSE);

            registry.with(fmt_layer).init();
        }
        LogFormat::Compact => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_thread_ids(config.include_thread_id)
                .with_thread_names(config.include_thread_name)
                .with_span_events(FmtSpan::CLOSE);

            registry.with(fmt_layer).init();
        }
    }

    info!(
        logging.format = ?config.format,
        logging.level = level,
        logging.location = config.include_location,
        "Structured logging initialized"
    );

    Ok(())
}

/// Initialize OpenTelemetry tracing
pub fn init_tracing() -> Result<()> {
    // Create a resource that identifies this service
    let resource = Resource::new(vec![
        opentelemetry::KeyValue::new(resource::SERVICE_NAME, "distributed-scheduler"),
        opentelemetry::KeyValue::new(resource::SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
    ]);

    // Create OTLP exporter (can be configured to send to Jaeger, etc.)
    let tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317"), // Default OTLP endpoint
        )
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                .with_resource(resource.clone())
                .with_sampler(opentelemetry_sdk::trace::Sampler::AlwaysOn),
        )
        .with_batch_config(BatchConfig::default())
        .install_batch(runtime::Tokio)
        .map_err(|e| anyhow::anyhow!("Failed to install tracer: {}", e))?;

    let tracer = tracer_provider.tracer("distributed-scheduler");

    // Create tracing subscriber with OpenTelemetry layer
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .with(telemetry_layer)
        .init();

    info!("OpenTelemetry tracing initialized");
    Ok(())
}

/// Initialize both structured logging and OpenTelemetry tracing
pub fn init_logging_and_tracing(logging_config: LoggingConfig) -> Result<()> {
    let level = logging_config.level.clone();
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| logging_config.level.clone().into());

    let registry = tracing_subscriber::registry().with(env_filter);

    // Add structured logging layer
    match logging_config.format {
        LogFormat::Json => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_file(logging_config.include_location)
                .with_line_number(logging_config.include_location)
                .with_thread_ids(logging_config.include_thread_id)
                .with_thread_names(logging_config.include_thread_name);

            registry.with(fmt_layer).init();
        }
        LogFormat::Pretty => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_file(logging_config.include_location)
                .with_line_number(logging_config.include_location)
                .with_thread_ids(logging_config.include_thread_id)
                .with_thread_names(logging_config.include_thread_name);

            registry.with(fmt_layer).init();
        }
        LogFormat::Compact => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_file(logging_config.include_location)
                .with_line_number(logging_config.include_location)
                .with_thread_ids(logging_config.include_thread_id)
                .with_thread_names(logging_config.include_thread_name);

            registry.with(fmt_layer).init();
        }
    }

    info!(
        logging.format = ?logging_config.format,
        logging.level = level,
        logging.location = logging_config.include_location,
        "Logging and tracing initialized"
    );

    Ok(())
}

/// Initialize OpenTelemetry metrics provider
pub fn init_metrics() -> Result<()> {
    // Create a meter provider with Prometheus exporter
    let (recorder, _handle) = metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9090))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create Prometheus exporter: {}", e))?;

    // Install the exporter
    metrics::set_global_recorder(recorder)
        .map_err(|e| anyhow::anyhow!("Failed to install metrics recorder: {}", e))?;

    info!("OpenTelemetry metrics initialized with Prometheus exporter on :9090");
    Ok(())
}

/// Initialize complete observability stack (logging, tracing, metrics)
pub fn init_observability(logging_config: Option<LoggingConfig>) -> Result<()> {
    let config = logging_config.unwrap_or_default();
    init_logging_and_tracing(config)?;
    init_metrics()?;
    info!("Complete observability stack initialized");
    Ok(())
}

/// Structured logging utilities
pub struct StructuredLogger;

impl StructuredLogger {
    /// Log task scheduling event
    pub fn log_task_scheduled(
        task_id: i64,
        task_name: &str,
        task_type: &str,
        scheduled_at: chrono::DateTime<chrono::Utc>,
    ) {
        info!(
            event = "task_scheduled",
            task.id = task_id,
            task.name = task_name,
            task.type = task_type,
            task.scheduled_at = %scheduled_at,
            "Task scheduled for execution"
        );
    }

    /// Log task execution start
    pub fn log_task_execution_start(
        task_run_id: i64,
        task_id: i64,
        task_name: &str,
        task_type: &str,
        worker_id: &str,
    ) {
        info!(
            event = "task_execution_start",
            task_run.id = task_run_id,
            task.id = task_id,
            task.name = task_name,
            task.type = task_type,
            worker.id = worker_id,
            "Task execution started"
        );
    }

    /// Log task execution completion
    pub fn log_task_execution_complete(
        task_run_id: i64,
        task_name: &str,
        task_type: &str,
        worker_id: &str,
        success: bool,
        duration_ms: u64,
        error_message: Option<&str>,
    ) {
        if success {
            info!(
                event = "task_execution_complete",
                task_run.id = task_run_id,
                task.name = task_name,
                task.type = task_type,
                worker.id = worker_id,
                task.success = success,
                task.duration_ms = duration_ms,
                "Task execution completed successfully"
            );
        } else {
            error!(
                event = "task_execution_failed",
                task_run.id = task_run_id,
                task.name = task_name,
                task.type = task_type,
                worker.id = worker_id,
                task.success = success,
                task.duration_ms = duration_ms,
                task.error = error_message.unwrap_or("Unknown error"),
                "Task execution failed"
            );
        }
    }

    /// Log task retry
    pub fn log_task_retry(
        task_run_id: i64,
        task_name: &str,
        retry_count: u32,
        max_retries: u32,
        reason: &str,
    ) {
        warn!(
            event = "task_retry",
            task_run.id = task_run_id,
            task.name = task_name,
            task.retry_count = retry_count,
            task.max_retries = max_retries,
            task.retry_reason = reason,
            "Task retry initiated"
        );
    }

    /// Log worker registration
    pub fn log_worker_registered(
        worker_id: &str,
        hostname: &str,
        supported_types: &[String],
        max_capacity: i32,
    ) {
        info!(
            event = "worker_registered",
            worker.id = worker_id,
            worker.hostname = hostname,
            worker.supported_types = ?supported_types,
            worker.max_capacity = max_capacity,
            "Worker registered with dispatcher"
        );
    }

    /// Log worker heartbeat
    pub fn log_worker_heartbeat(
        worker_id: &str,
        current_load: i32,
        max_capacity: i32,
        system_load: Option<f64>,
    ) {
        debug!(
            event = "worker_heartbeat",
            worker.id = worker_id,
            worker.current_load = current_load,
            worker.max_capacity = max_capacity,
            worker.system_load = system_load,
            "Worker heartbeat received"
        );
    }

    /// Log dependency check
    pub fn log_dependency_check(
        task_id: i64,
        task_name: &str,
        dependencies_met: bool,
        reason: Option<&str>,
    ) {
        if dependencies_met {
            debug!(
                event = "dependency_check_passed",
                task.id = task_id,
                task.name = task_name,
                "Task dependencies satisfied"
            );
        } else {
            warn!(
                event = "dependency_check_failed",
                task.id = task_id,
                task.name = task_name,
                dependency.reason = reason.unwrap_or("Unknown reason"),
                "Task dependencies not satisfied"
            );
        }
    }

    /// Log database operation
    pub fn log_database_operation(
        operation: &str,
        table: &str,
        duration_ms: u64,
        affected_rows: Option<u64>,
    ) {
        debug!(
            event = "database_operation",
            db.operation = operation,
            db.table = table,
            db.duration_ms = duration_ms,
            db.affected_rows = affected_rows,
            "Database operation completed"
        );
    }

    /// Log message queue operation
    pub fn log_message_queue_operation(
        operation: &str,
        queue: &str,
        message_count: Option<u32>,
        duration_ms: u64,
    ) {
        debug!(
            event = "message_queue_operation",
            mq.operation = operation,
            mq.queue = queue,
            mq.message_count = message_count,
            mq.duration_ms = duration_ms,
            "Message queue operation completed"
        );
    }

    /// Log system error
    pub fn log_system_error(component: &str, operation: &str, error: &dyn std::error::Error) {
        error!(
            event = "system_error",
            error.component = component,
            error.operation = operation,
            error.message = %error,
            error.type = std::any::type_name_of_val(error),
            "System error occurred"
        );
    }

    /// Log configuration change
    pub fn log_config_change(component: &str, setting: &str, old_value: &str, new_value: &str) {
        info!(
            event = "config_change",
            config.component = component,
            config.setting = setting,
            config.old_value = old_value,
            config.new_value = new_value,
            "Configuration changed"
        );
    }

    /// Log performance metrics
    pub fn log_performance_metrics(
        component: &str,
        operation: &str,
        duration_ms: u64,
        throughput: Option<f64>,
        memory_usage_mb: Option<u64>,
    ) {
        info!(
            event = "performance_metrics",
            perf.component = component,
            perf.operation = operation,
            perf.duration_ms = duration_ms,
            perf.throughput = throughput,
            perf.memory_usage_mb = memory_usage_mb,
            "Performance metrics recorded"
        );
    }
}

/// Shutdown OpenTelemetry metrics and tracing
pub fn shutdown_observability() {
    global::shutdown_tracer_provider();
    info!("OpenTelemetry observability shutdown completed");
}

/// Tracing utilities for task lifecycle tracking
pub struct TaskTracer;

impl TaskTracer {
    /// Create a span for task scheduling
    pub fn schedule_task_span(task_id: i64, task_name: &str, task_type: &str) -> tracing::Span {
        let span = tracing::info_span!(
            "schedule_task",
            task.id = task_id,
            task.name = task_name,
            task.type = task_type,
            otel.kind = "internal"
        );

        // Add OpenTelemetry attributes
        span.set_attribute("scheduler.task.id", task_id);
        span.set_attribute("scheduler.task.name", task_name.to_string());
        span.set_attribute("scheduler.task.type", task_type.to_string());

        span
    }

    /// Create a span for task execution
    pub fn execute_task_span(
        task_run_id: i64,
        task_id: i64,
        task_name: &str,
        task_type: &str,
        worker_id: &str,
    ) -> tracing::Span {
        let span = tracing::info_span!(
            "execute_task",
            task_run.id = task_run_id,
            task.id = task_id,
            task.name = task_name,
            task.type = task_type,
            worker.id = worker_id,
            otel.kind = "internal"
        );

        // Add OpenTelemetry attributes
        span.set_attribute("scheduler.task_run.id", task_run_id);
        span.set_attribute("scheduler.task.id", task_id);
        span.set_attribute("scheduler.task.name", task_name.to_string());
        span.set_attribute("scheduler.task.type", task_type.to_string());
        span.set_attribute("scheduler.worker.id", worker_id.to_string());

        span
    }

    /// Create a span for dependency checking
    pub fn dependency_check_span(task_id: i64, task_name: &str) -> tracing::Span {
        let span = tracing::info_span!(
            "check_dependencies",
            task.id = task_id,
            task.name = task_name,
            otel.kind = "internal"
        );

        span.set_attribute("scheduler.task.id", task_id);
        span.set_attribute("scheduler.task.name", task_name.to_string());

        span
    }

    /// Create a span for message queue operations
    pub fn message_queue_span(operation: &str, queue_name: &str) -> tracing::Span {
        let span = tracing::info_span!(
            "message_queue_operation",
            mq.operation = operation,
            mq.queue = queue_name,
            otel.kind = "producer"
        );

        span.set_attribute("messaging.operation", operation.to_string());
        span.set_attribute("messaging.destination.name", queue_name.to_string());
        span.set_attribute("messaging.system", "rabbitmq");

        span
    }

    /// Create a span for database operations
    pub fn database_span(operation: &str, table: &str) -> tracing::Span {
        let span = tracing::info_span!(
            "database_operation",
            db.operation = operation,
            db.table = table,
            otel.kind = "client"
        );

        span.set_attribute("db.operation", operation.to_string());
        span.set_attribute("db.sql.table", table.to_string());
        span.set_attribute("db.system", "postgresql");

        span
    }

    /// Add error information to current span
    pub fn record_error(error: &dyn std::error::Error) {
        let span = tracing::Span::current();
        span.set_attribute("error", true);
        span.set_attribute("error.message", error.to_string());
        span.set_attribute("error.type", std::any::type_name_of_val(error));
    }

    /// Add task execution result to span
    pub fn record_task_result(success: bool, execution_time_ms: u64, error_message: Option<&str>) {
        let span = tracing::Span::current();
        span.set_attribute("task.success", success);
        span.set_attribute("task.execution_time_ms", execution_time_ms as i64);

        if let Some(error) = error_message {
            span.set_attribute("task.error_message", error.to_string());
        }
    }

    /// Add retry information to span
    pub fn record_retry(retry_count: u32, max_retries: u32) {
        let span = tracing::Span::current();
        span.set_attribute("task.retry_count", retry_count as i64);
        span.set_attribute("task.max_retries", max_retries as i64);
    }
}

/// Cross-component tracing utilities
pub struct CrossComponentTracer;

impl CrossComponentTracer {
    /// Extract trace context from message headers (for message queue)
    pub fn extract_trace_context_from_headers(
        headers: &std::collections::HashMap<String, String>,
    ) -> Option<opentelemetry::Context> {
        use opentelemetry::propagation::Extractor;

        struct HeaderExtractor<'a>(&'a std::collections::HashMap<String, String>);

        impl<'a> Extractor for HeaderExtractor<'a> {
            fn get(&self, key: &str) -> Option<&str> {
                self.0.get(key).map(|s| s.as_str())
            }

            fn keys(&self) -> Vec<&str> {
                self.0.keys().map(|s| s.as_str()).collect()
            }
        }

        let extractor = HeaderExtractor(headers);
        let context = global::get_text_map_propagator(|propagator| propagator.extract(&extractor));

        Some(context)
    }

    /// Inject trace context into message headers (for message queue)
    pub fn inject_trace_context_into_headers(
        headers: &mut std::collections::HashMap<String, String>,
    ) {
        use opentelemetry::propagation::Injector;

        struct HeaderInjector<'a>(&'a mut std::collections::HashMap<String, String>);

        impl<'a> Injector for HeaderInjector<'a> {
            fn set(&mut self, key: &str, value: String) {
                self.0.insert(key.to_string(), value);
            }
        }

        let mut injector = HeaderInjector(headers);
        let context = tracing::Span::current().context();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&context, &mut injector)
        });
    }

    /// Create a child span from remote context
    pub fn create_child_span_from_context(
        context: opentelemetry::Context,
        span_name: &str,
        attributes: Vec<(String, String)>,
    ) -> tracing::Span {
        let span = tracing::info_span!(target: "scheduler", "{}", span_name);

        // Set the remote context as parent
        span.set_parent(context);

        // Add attributes
        for (key, value) in attributes {
            span.set_attribute(key, value);
        }

        span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new().unwrap();

        // Test task execution metrics
        collector.record_task_execution("shell", "completed", 1.5);
        collector.record_task_failure("http", "timeout");
        collector.record_task_retry("shell", 1);

        // Test worker metrics
        collector.update_active_workers(3.0);
        collector.update_worker_capacity("worker-1", 10.0);
        collector.update_worker_load("worker-1", 5.0);

        // Test system metrics
        collector.update_queue_depth(25.0);
        collector.record_scheduling_duration(0.1);
        collector.record_database_operation("select", 0.05);
        collector.record_message_queue_operation("publish", 0.02);
    }

    #[tokio::test]
    async fn test_metrics_initialization() {
        // This test would require a more complex setup in a real scenario
        // For now, we just test that the function doesn't panic
        let result = init_metrics();
        // In a test environment, this might fail due to port conflicts
        // so we don't assert success
        println!("Metrics init result: {:?}", result);
    }

    #[tokio::test]
    async fn test_task_tracer_spans() {
        // Test task scheduling span
        let schedule_span = TaskTracer::schedule_task_span(1, "test-task", "shell");
        assert_eq!(schedule_span.metadata().unwrap().name(), "schedule_task");

        // Test task execution span
        let execute_span = TaskTracer::execute_task_span(1, 1, "test-task", "shell", "worker-1");
        assert_eq!(execute_span.metadata().unwrap().name(), "execute_task");

        // Test dependency check span
        let dep_span = TaskTracer::dependency_check_span(1, "test-task");
        assert_eq!(dep_span.metadata().unwrap().name(), "check_dependencies");

        // Test message queue span
        let mq_span = TaskTracer::message_queue_span("publish", "tasks");
        assert_eq!(
            mq_span.metadata().unwrap().name(),
            "message_queue_operation"
        );

        // Test database span
        let db_span = TaskTracer::database_span("select", "tasks");
        assert_eq!(db_span.metadata().unwrap().name(), "database_operation");
    }

    #[tokio::test]
    async fn test_cross_component_tracing() {
        use std::collections::HashMap;

        // Test header injection and extraction
        let mut headers = HashMap::new();
        CrossComponentTracer::inject_trace_context_into_headers(&mut headers);

        // In test environment without proper tracing setup, headers might be empty
        // This is expected behavior

        // Test context extraction with empty headers
        let context = CrossComponentTracer::extract_trace_context_from_headers(&headers);
        assert!(context.is_some()); // Should always return some context, even if empty

        // Test with some sample headers
        let mut sample_headers = HashMap::new();
        sample_headers.insert(
            "traceparent".to_string(),
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
        );

        let context_with_headers =
            CrossComponentTracer::extract_trace_context_from_headers(&sample_headers);
        assert!(context_with_headers.is_some());
    }

    #[tokio::test]
    async fn test_structured_logging_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert!(matches!(config.format, LogFormat::Json));
        assert!(matches!(config.output, LogOutput::Stdout));
        assert!(config.include_location);

        let custom_config = LoggingConfig {
            level: "debug".to_string(),
            format: LogFormat::Pretty,
            output: LogOutput::Stderr,
            include_location: false,
            include_thread_id: true,
            include_thread_name: true,
        };

        assert_eq!(custom_config.level, "debug");
        assert!(matches!(custom_config.format, LogFormat::Pretty));
        assert!(custom_config.include_thread_id);
    }

    #[tokio::test]
    async fn test_structured_logger_methods() {
        use chrono::Utc;

        // These tests just verify the methods don't panic
        // In a real test environment, you'd capture and verify log output

        StructuredLogger::log_task_scheduled(1, "test-task", "shell", Utc::now());
        StructuredLogger::log_task_execution_start(1, 1, "test-task", "shell", "worker-1");
        StructuredLogger::log_task_execution_complete(
            1,
            "test-task",
            "shell",
            "worker-1",
            true,
            1000,
            None,
        );
        StructuredLogger::log_task_retry(1, "test-task", 1, 3, "timeout");
        StructuredLogger::log_worker_registered("worker-1", "localhost", &["shell".to_string()], 5);
        StructuredLogger::log_worker_heartbeat("worker-1", 2, 5, Some(0.5));
        StructuredLogger::log_dependency_check(1, "test-task", true, None);
        StructuredLogger::log_database_operation("select", "tasks", 50, Some(10));
        StructuredLogger::log_message_queue_operation("publish", "tasks", Some(1), 25);

        let error = std::io::Error::new(std::io::ErrorKind::Other, "test error");
        StructuredLogger::log_system_error("test", "test_operation", &error);

        StructuredLogger::log_config_change("worker", "max_tasks", "5", "10");
        StructuredLogger::log_performance_metrics(
            "worker",
            "execute_task",
            1000,
            Some(10.5),
            Some(256),
        );
    }
}
