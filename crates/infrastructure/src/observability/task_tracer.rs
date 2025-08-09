
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub struct TaskTracer;

impl TaskTracer {
    pub fn schedule_task_span(task_id: i64, task_name: &str, task_type: &str) -> tracing::Span {
        let span = tracing::info_span!(
            "schedule_task",
            task.id = task_id,
            task.name = task_name,
            task.type = task_type,
            otel.kind = "internal"
        );
        span.set_attribute("scheduler.task.id", task_id);
        span.set_attribute("scheduler.task.name", task_name.to_string());
        span.set_attribute("scheduler.task.type", task_type.to_string());

        span
    }
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
        span.set_attribute("scheduler.task_run.id", task_run_id);
        span.set_attribute("scheduler.task.id", task_id);
        span.set_attribute("scheduler.task.name", task_name.to_string());
        span.set_attribute("scheduler.task.type", task_type.to_string());
        span.set_attribute("scheduler.worker.id", worker_id.to_string());

        span
    }
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
    pub fn record_error(error: &dyn std::error::Error) {
        let span = tracing::Span::current();
        span.set_attribute("error", true);
        span.set_attribute("error.message", error.to_string());
        span.set_attribute("error.type", std::any::type_name_of_val(error));
    }
    pub fn record_task_result(success: bool, execution_time_ms: u64, error_message: Option<&str>) {
        let span = tracing::Span::current();
        span.set_attribute("task.success", success);
        span.set_attribute("task.execution_time_ms", execution_time_ms as i64);

        if let Some(error) = error_message {
            span.set_attribute("task.error_message", error.to_string());
        }
    }
    pub fn record_retry(retry_count: u32, max_retries: u32) {
        let span = tracing::Span::current();
        span.set_attribute("task.retry_count", retry_count as i64);
        span.set_attribute("task.max_retries", max_retries as i64);
    }
}
