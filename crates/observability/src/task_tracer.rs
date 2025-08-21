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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_task_span() {
        let span = TaskTracer::schedule_task_span(1, "test_task", "shell");

        // Test that span is created without panic
        // In a real test environment, we would check the span attributes
        // but for now we just ensure it doesn't panic
        let _span = span;
    }

    #[test]
    fn test_execute_task_span() {
        let span = TaskTracer::execute_task_span(1, 1, "test_task", "shell", "worker-1");

        // Test that span is created without panic
        let _span = span;
    }

    #[test]
    fn test_dependency_check_span() {
        let span = TaskTracer::dependency_check_span(1, "test_task");

        // Test that span is created without panic
        let _span = span;
    }

    #[test]
    fn test_message_queue_span() {
        let span = TaskTracer::message_queue_span("publish", "task_queue");

        // Test that span is created without panic
        let _span = span;
    }

    #[test]
    fn test_database_span() {
        let span = TaskTracer::database_span("SELECT", "tasks");

        // Test that span is created without panic
        let _span = span;
    }

    #[test]
    fn test_record_error() {
        // Test that recording an error doesn't panic
        let test_error = std::io::Error::new(std::io::ErrorKind::Other, "Test error");
        TaskTracer::record_error(&test_error);
    }

    #[test]
    fn test_record_task_result() {
        // Test recording successful task result
        TaskTracer::record_task_result(true, 100, None);

        // Test recording failed task result
        TaskTracer::record_task_result(false, 200, Some("Task failed"));

        // Test recording with empty error message
        TaskTracer::record_task_result(false, 150, Some(""));
    }

    #[test]
    fn test_record_retry() {
        // Test recording retry with different scenarios
        TaskTracer::record_retry(0, 3);
        TaskTracer::record_retry(1, 3);
        TaskTracer::record_retry(3, 3);
        TaskTracer::record_retry(5, 3); // Exceeds max retries
    }

    #[test]
    fn test_span_creation_with_edge_cases() {
        // Test span creation with various edge cases
        let span1 = TaskTracer::schedule_task_span(0, "", ""); // Empty strings
        let span2 = TaskTracer::execute_task_span(-1, -1, "task", "type", "worker"); // Negative IDs
        let span3 = TaskTracer::message_queue_span("", ""); // Empty queue and operation
        let span4 = TaskTracer::database_span("", ""); // Empty operation and table

        // Just ensure they don't panic
        let _ = (span1, span2, span3, span4);
    }

    #[test]
    fn test_span_creation_with_long_strings() {
        let long_task_name =
            "very-long-task-name-with-many-characters-and-descriptive-text-1234567890";
        let long_task_type =
            "very-long-task-type-with-detailed-classification-and-category-1234567890";
        let long_worker_id =
            "very-long-worker-id-with-many-characters-and-unique-identifier-1234567890";
        let long_queue_name =
            "very-long-queue-name-with-detailed-purpose-and-routing-information-1234567890";

        let span1 = TaskTracer::schedule_task_span(1, long_task_name, long_task_type);
        let span2 =
            TaskTracer::execute_task_span(1, 1, long_task_name, long_task_type, long_worker_id);
        let span3 = TaskTracer::message_queue_span("publish", long_queue_name);

        // Just ensure they don't panic
        let _ = (span1, span2, span3);
    }

    #[test]
    fn test_span_creation_with_special_characters() {
        let special_task_name = "Task with \"special\" characters & symbols @#$%";
        let special_worker_id = "Worker-ID_123.456.789_Special@Chars";
        let special_queue = "queue.special#$%&*()";

        let span1 = TaskTracer::schedule_task_span(1, special_task_name, "shell");
        let span2 =
            TaskTracer::execute_task_span(1, 1, special_task_name, "shell", special_worker_id);
        let span3 = TaskTracer::message_queue_span("publish", special_queue);

        // Just ensure they don't panic
        let _ = (span1, span2, span3);
    }

    #[test]
    fn test_record_error_with_different_error_types() {
        // Test recording different types of errors
        let io_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let custom_error = Box::<dyn std::error::Error + Send + Sync>::from(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Custom error",
        ));

        TaskTracer::record_error(&io_error);
        TaskTracer::record_error(&json_error);
        TaskTracer::record_error(custom_error.as_ref());
    }

    #[test]
    fn test_record_task_result_with_extreme_values() {
        // Test recording task results with extreme values
        TaskTracer::record_task_result(true, 0, None); // Zero duration
        TaskTracer::record_task_result(true, 999999, None); // Very long duration
        TaskTracer::record_task_result(false, 1, Some("")); // Empty error message
        TaskTracer::record_task_result(false, 100, Some("x".repeat(1000).as_str()));
        // Very long error message
    }

    #[test]
    fn test_record_retry_with_extreme_values() {
        // Test recording retries with extreme values
        TaskTracer::record_retry(0, 0); // Zero retry count, zero max retries
        TaskTracer::record_retry(1000, 1); // Many retries, low max
        TaskTracer::record_retry(1, 1000); // Few retries, high max
        TaskTracer::record_retry(u32::MAX, u32::MAX); // Maximum values
    }

    #[test]
    fn test_multiple_span_creations() {
        // Test creating multiple spans in sequence
        for i in 0..10 {
            let span = TaskTracer::schedule_task_span(i, &format!("task_{}", i), "shell");
            let _ = span; // Just to avoid unused variable warning
        }

        // Test creating multiple execution spans
        for i in 0..5 {
            let span =
                TaskTracer::execute_task_span(i, i, "task", "type", &format!("worker_{}", i));
            let _ = span;
        }
    }

    #[test]
    fn test_concurrent_span_creation() {
        // Test creating spans from multiple threads (though they'll be dropped immediately)
        let handles: Vec<_> = (0..10)
            .map(|i| {
                std::thread::spawn(move || {
                    TaskTracer::schedule_task_span(i, &format!("concurrent_task_{}", i), "shell");
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_database_span_with_various_operations() {
        let operations = vec![
            "SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "DROP", "ALTER", "TRUNCATE",
        ];
        let tables = vec![
            "tasks",
            "task_runs",
            "workers",
            "messages",
            "config",
            "metadata",
        ];

        for operation in operations {
            for table in &tables {
                let span = TaskTracer::database_span(operation, table);
                let _ = span;
            }
        }
    }

    #[test]
    fn test_message_queue_span_with_various_operations() {
        let operations = vec!["publish", "consume", "ack", "nack", "reject", "requeue"];
        let queues = vec![
            "task_queue",
            "result_queue",
            "heartbeat_queue",
            "control_queue",
        ];

        for operation in operations {
            for queue in &queues {
                let span = TaskTracer::message_queue_span(operation, queue);
                let _ = span;
            }
        }
    }
}
