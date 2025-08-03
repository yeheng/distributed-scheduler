//! Structured logging utilities
//!
//! This module provides comprehensive structured logging capabilities
//! for various events and operations in the distributed scheduler system.

use tracing::{debug, error, info, warn};

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
