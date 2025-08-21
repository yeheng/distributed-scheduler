use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
    pub output: LogOutput,
    pub include_location: bool,
    pub include_thread_id: bool,
    pub include_thread_name: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

pub struct StructuredLogger;

impl StructuredLogger {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert_eq!(config.format, LogFormat::Json);
        assert_eq!(config.output, LogOutput::Stdout);
        assert!(config.include_location);
        assert!(!config.include_thread_id);
        assert!(!config.include_thread_name);
    }

    #[test]
    fn test_logging_config_creation() {
        let config = LoggingConfig {
            level: "debug".to_string(),
            format: LogFormat::Pretty,
            output: LogOutput::Stderr,
            include_location: false,
            include_thread_id: true,
            include_thread_name: true,
        };

        assert_eq!(config.level, "debug");
        assert_eq!(config.format, LogFormat::Pretty);
        assert_eq!(config.output, LogOutput::Stderr);
        assert!(!config.include_location);
        assert!(config.include_thread_id);
        assert!(config.include_thread_name);
    }

    #[test]
    fn test_log_format_variants() {
        // Test that all LogFormat variants can be created
        let formats = vec![LogFormat::Json, LogFormat::Pretty, LogFormat::Compact];
        for format in formats {
            // This test just ensures the variants exist and can be created
            let _ = format;
        }
    }

    #[test]
    fn test_log_output_variants() {
        // Test that all LogOutput variants can be created
        let outputs = vec![
            LogOutput::Stdout,
            LogOutput::Stderr,
            LogOutput::File("/tmp/test.log".to_string()),
        ];
        for output in outputs {
            // This test just ensures the variants exist and can be created
            let _ = output;
        }
    }

    #[test]
    fn test_logging_config_clone() {
        let config = LoggingConfig {
            level: "warn".to_string(),
            format: LogFormat::Compact,
            output: LogOutput::File("/tmp/test.log".to_string()),
            include_location: true,
            include_thread_id: false,
            include_thread_name: false,
        };

        let cloned_config = config.clone();
        assert_eq!(cloned_config.level, config.level);
        assert_eq!(cloned_config.format, config.format);
        assert_eq!(cloned_config.output, config.output);
        assert_eq!(cloned_config.include_location, config.include_location);
        assert_eq!(cloned_config.include_thread_id, config.include_thread_id);
        assert_eq!(
            cloned_config.include_thread_name,
            config.include_thread_name
        );
    }

    #[test]
    fn test_logging_config_debug() {
        let config = LoggingConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("LoggingConfig"));
        assert!(debug_str.contains("level"));
        assert!(debug_str.contains("format"));
        assert!(debug_str.contains("output"));
    }

    #[test]
    fn test_log_output_file_path() {
        let file_path = "/var/log/scheduler.log".to_string();
        let output = LogOutput::File(file_path.clone());

        match output {
            LogOutput::File(path) => assert_eq!(path, file_path),
            LogOutput::Stdout => panic!("Expected File output, got Stdout"),
            LogOutput::Stderr => panic!("Expected File output, got Stderr"),
        }
    }

    #[test]
    fn test_structured_logger_static_methods_exist() {
        // Test that all static methods exist and can be called (they just log)
        let timestamp = Utc::now();

        // These methods should not panic when called
        StructuredLogger::log_task_scheduled(1, "test_task", "shell", timestamp);
        StructuredLogger::log_task_execution_start(1, 1, "test_task", "shell", "worker-1");
        StructuredLogger::log_task_execution_complete(
            1,
            "test_task",
            "shell",
            "worker-1",
            true,
            100,
            None,
        );
        StructuredLogger::log_task_retry(1, "test_task", 1, 3, "timeout");
        StructuredLogger::log_worker_registered("worker-1", "hostname", &["shell".to_string()], 5);
        StructuredLogger::log_worker_heartbeat("worker-1", 2, 5, Some(0.8));
        StructuredLogger::log_dependency_check(1, "test_task", true, Some("dependencies met"));
        StructuredLogger::log_database_operation("SELECT", "tasks", 50, Some(10));
        StructuredLogger::log_message_queue_operation("publish", "task_queue", Some(5), 10);
        StructuredLogger::log_system_error(
            "database",
            "query",
            &std::io::Error::new(std::io::ErrorKind::Other, "test error"),
        );
        StructuredLogger::log_config_change("logging", "level", "info", "debug");
        StructuredLogger::log_performance_metrics(
            "scheduler",
            "task_scheduling",
            100,
            Some(10.5),
            Some(512),
        );
    }

    #[test]
    fn test_log_task_execution_with_error() {
        let timestamp = Utc::now();
        let error_message = Some("Task execution failed due to timeout");

        // This should not panic and should handle the error message properly
        StructuredLogger::log_task_execution_complete(
            1,
            "test_task",
            "shell",
            "worker-1",
            false,
            200,
            error_message,
        );
    }

    #[test]
    fn test_log_with_empty_options() {
        let timestamp = Utc::now();

        // Test with empty optional parameters
        StructuredLogger::log_task_execution_complete(
            1,
            "test_task",
            "shell",
            "worker-1",
            true,
            50,
            None,
        );
        StructuredLogger::log_worker_heartbeat("worker-1", 0, 5, None);
        StructuredLogger::log_dependency_check(1, "test_task", false, None);
        StructuredLogger::log_database_operation("INSERT", "tasks", 25, None);
        StructuredLogger::log_message_queue_operation("consume", "task_queue", None, 15);
        StructuredLogger::log_performance_metrics("api", "request", 30, None, None);
    }

    #[test]
    fn test_log_with_long_strings() {
        let timestamp = Utc::now();
        let long_worker_id = "very-long-worker-id-with-many-characters-1234567890";
        let long_task_name =
            "very-long-task-name-with-many-characters-and-descriptive-text-1234567890";
        let long_error_message = "very-long-error-message-with-detailed-description-of-what-went-wrong-and-why-it-failed-1234567890";

        // Test that long strings are handled properly
        StructuredLogger::log_task_execution_start(1, 1, long_task_name, "shell", long_worker_id);
        StructuredLogger::log_task_execution_complete(
            1,
            long_task_name,
            "shell",
            long_worker_id,
            false,
            500,
            Some(long_error_message),
        );
    }

    #[test]
    fn test_log_with_special_characters() {
        let timestamp = Utc::now();
        let special_task_name = "Task with \"special\" characters & symbols @#$%";
        let special_worker_id = "Worker-ID_123.456.789";

        // Test that special characters are handled properly
        StructuredLogger::log_task_scheduled(1, special_task_name, "shell", timestamp);
        StructuredLogger::log_worker_registered(
            special_worker_id,
            "hostname.local",
            &["shell".to_string()],
            5,
        );
    }

    #[test]
    fn test_log_with_numeric_values() {
        let timestamp = Utc::now();

        // Test with various numeric values including edge cases
        StructuredLogger::log_task_execution_complete(
            1,
            "test_task",
            "shell",
            "worker-1",
            true,
            0,
            None,
        );
        StructuredLogger::log_task_execution_complete(
            2,
            "test_task",
            "shell",
            "worker-1",
            false,
            999999,
            Some("timeout"),
        );
        StructuredLogger::log_worker_heartbeat("worker-1", 0, 10, Some(0.0));
        StructuredLogger::log_worker_heartbeat("worker-1", 100, 100, Some(100.0));
        StructuredLogger::log_database_operation("SELECT", "tasks", 0, Some(0));
        StructuredLogger::log_database_operation("INSERT", "tasks", 10000, Some(1000000));
    }

    #[test]
    fn test_log_task_retry_edge_cases() {
        // Test retry logging with edge cases
        StructuredLogger::log_task_retry(1, "test_task", 0, 0, "immediate retry");
        StructuredLogger::log_task_retry(2, "test_task", 1, 1, "max retries reached");
        StructuredLogger::log_task_retry(3, "test_task", 10, 10, "multiple retries");
        StructuredLogger::log_task_retry(4, "test_task", 100, 5, "exceeded max retries");
    }

    #[test]
    fn test_log_config_change_scenarios() {
        // Test various configuration change scenarios
        StructuredLogger::log_config_change("logging", "level", "debug", "info");
        StructuredLogger::log_config_change("database", "host", "localhost", "db.example.com");
        StructuredLogger::log_config_change("queue", "timeout", "30", "60");
        StructuredLogger::log_config_change("worker", "max_tasks", "10", "20");
    }

    #[test]
    fn test_log_system_error_scenarios() {
        // Test various system error scenarios
        let io_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();

        StructuredLogger::log_system_error("database", "connect", &io_error);
        StructuredLogger::log_system_error("message_queue", "parse", &json_error);
        StructuredLogger::log_system_error(
            "worker",
            "register",
            &std::io::Error::new(std::io::ErrorKind::Other, "Registration failed"),
        );
    }

    #[test]
    fn test_log_performance_metrics_scenarios() {
        // Test various performance metrics scenarios
        StructuredLogger::log_performance_metrics(
            "scheduler",
            "task_scheduling",
            10,
            Some(100.5),
            Some(1024),
        );
        StructuredLogger::log_performance_metrics("database", "query", 500, Some(0.1), None);
        StructuredLogger::log_performance_metrics("api", "request", 50, Some(50.0), Some(256));
        StructuredLogger::log_performance_metrics("worker", "heartbeat", 1, None, Some(128));
    }
}
