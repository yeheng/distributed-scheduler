//! Metrics collector for the distributed scheduler system
//!
//! This module provides comprehensive metrics collection and reporting
//! capabilities using OpenTelemetry and the metrics crate.

use anyhow::Result;
use metrics::{counter, gauge, histogram, Counter, Gauge, Histogram};
use tracing::{info, warn};

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