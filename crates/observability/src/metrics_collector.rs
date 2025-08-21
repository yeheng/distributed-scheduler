use anyhow::Result;
use metrics::{counter, gauge, histogram, Counter, Gauge, Histogram};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Clone, Debug)]
pub struct PerformanceMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub disk_usage_mb: f64,
    pub network_io_mb: f64,
    pub timestamp: std::time::SystemTime,
}

pub struct MetricsCollector {
    // Task metrics
    task_executions_total: Counter,
    task_execution_duration: Histogram,
    task_failures_total: Counter,
    task_retries_total: Counter,
    task_queue_time: Histogram,

    // Worker metrics
    active_workers: Gauge,
    worker_task_capacity: Gauge,
    worker_current_load: Gauge,
    worker_heartbeat_count: Counter,
    worker_failures_total: Counter,

    // Queue metrics
    queue_depth: Gauge,
    queue_processing_rate: Histogram,
    queue_latency: Histogram,
    queue_backpressure_count: Counter,

    // Dispatcher metrics
    dispatcher_scheduling_duration: Histogram,
    dispatcher_tasks_scheduled_total: Counter,
    dispatcher_dependency_resolutions_total: Counter,

    // Database metrics
    database_operation_duration: Histogram,
    database_connection_pool_size: Gauge,
    database_query_count: Counter,
    database_slow_query_count: Counter,
    database_connection_failures: Counter,

    // Message queue metrics
    message_queue_operation_duration: Histogram,
    message_queue_throughput: Histogram,
    message_queue_errors_total: Counter,
    message_queue_consumer_lag: Gauge,

    // System resource metrics
    system_cpu_usage: Gauge,
    system_memory_usage: Gauge,
    system_disk_usage: Gauge,
    system_network_io: Gauge,

    // API metrics
    api_requests_total: Counter,
    api_response_duration: Histogram,
    api_error_rate: Counter,

    // Cache metrics
    cache_hit_rate: Gauge,
    cache_operation_duration: Histogram,
    cache_size: Gauge,

    // Performance history for regression detection
    performance_history: Arc<RwLock<Vec<PerformanceMetrics>>>,
}

impl MetricsCollector {
    pub fn new() -> Result<Self> {
        // Task metrics
        let task_executions_total = counter!("scheduler_task_executions_total");
        let task_execution_duration = histogram!("scheduler_task_execution_duration_seconds");
        let task_failures_total = counter!("scheduler_task_failures_total");
        let task_retries_total = counter!("scheduler_task_retries_total");
        let task_queue_time = histogram!("scheduler_task_queue_time_seconds");

        // Worker metrics
        let active_workers = gauge!("scheduler_active_workers");
        let worker_task_capacity = gauge!("scheduler_worker_task_capacity");
        let worker_current_load = gauge!("scheduler_worker_current_load");
        let worker_heartbeat_count = counter!("scheduler_worker_heartbeat_count_total");
        let worker_failures_total = counter!("scheduler_worker_failures_total");

        // Queue metrics
        let queue_depth = gauge!("scheduler_queue_depth");
        let queue_processing_rate = histogram!("scheduler_queue_processing_rate_per_second");
        let queue_latency = histogram!("scheduler_queue_latency_seconds");
        let queue_backpressure_count = counter!("scheduler_queue_backpressure_count_total");

        // Dispatcher metrics
        let dispatcher_scheduling_duration =
            histogram!("scheduler_dispatcher_scheduling_duration_seconds");
        let dispatcher_tasks_scheduled_total =
            counter!("scheduler_dispatcher_tasks_scheduled_total");
        let dispatcher_dependency_resolutions_total =
            counter!("scheduler_dispatcher_dependency_resolutions_total");

        // Database metrics
        let database_operation_duration =
            histogram!("scheduler_database_operation_duration_seconds");
        let database_connection_pool_size = gauge!("scheduler_database_connection_pool_size");
        let database_query_count = counter!("scheduler_database_query_count_total");
        let database_slow_query_count = counter!("scheduler_database_slow_query_count_total");
        let database_connection_failures = counter!("scheduler_database_connection_failures_total");

        // Message queue metrics
        let message_queue_operation_duration =
            histogram!("scheduler_message_queue_operation_duration_seconds");
        let message_queue_throughput = histogram!("scheduler_message_queue_throughput_per_second");
        let message_queue_errors_total = counter!("scheduler_message_queue_errors_total");
        let message_queue_consumer_lag = gauge!("scheduler_message_queue_consumer_lag_seconds");

        // System resource metrics
        let system_cpu_usage = gauge!("scheduler_system_cpu_usage_percent");
        let system_memory_usage = gauge!("scheduler_system_memory_usage_mb");
        let system_disk_usage = gauge!("scheduler_system_disk_usage_mb");
        let system_network_io = gauge!("scheduler_system_network_io_mb");

        // API metrics
        let api_requests_total = counter!("scheduler_api_requests_total");
        let api_response_duration = histogram!("scheduler_api_response_duration_seconds");
        let api_error_rate = counter!("scheduler_api_error_rate_total");

        // Cache metrics
        let cache_hit_rate = gauge!("scheduler_cache_hit_rate_percent");
        let cache_operation_duration = histogram!("scheduler_cache_operation_duration_seconds");
        let cache_size = gauge!("scheduler_cache_size_mb");

        Ok(Self {
            task_executions_total,
            task_execution_duration,
            task_failures_total,
            task_retries_total,
            task_queue_time,
            active_workers,
            worker_task_capacity,
            worker_current_load,
            worker_heartbeat_count,
            worker_failures_total,
            queue_depth,
            queue_processing_rate,
            queue_latency,
            queue_backpressure_count,
            dispatcher_scheduling_duration,
            dispatcher_tasks_scheduled_total,
            dispatcher_dependency_resolutions_total,
            database_operation_duration,
            database_connection_pool_size,
            database_query_count,
            database_slow_query_count,
            database_connection_failures,
            message_queue_operation_duration,
            message_queue_throughput,
            message_queue_errors_total,
            message_queue_consumer_lag,
            system_cpu_usage,
            system_memory_usage,
            system_disk_usage,
            system_network_io,
            api_requests_total,
            api_response_duration,
            api_error_rate,
            cache_hit_rate,
            cache_operation_duration,
            cache_size,
            performance_history: Arc::new(RwLock::new(Vec::new())),
        })
    }
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
    pub fn record_task_failure(&self, task_type: &str, error_type: &str) {
        self.task_failures_total.increment(1);

        warn!(
            task_type = task_type,
            error_type = error_type,
            "Task execution failed"
        );
    }
    pub fn record_task_retry(&self, task_type: &str, retry_count: u32) {
        self.task_retries_total.increment(1);

        info!(
            task_type = task_type,
            retry_count = retry_count,
            "Task retry initiated"
        );
    }
    pub fn update_active_workers(&self, count: f64) {
        self.active_workers.set(count);
    }
    pub fn update_worker_capacity(&self, worker_id: &str, capacity: f64) {
        self.worker_task_capacity.set(capacity);

        info!(
            worker_id = worker_id,
            capacity = capacity,
            "Worker capacity updated"
        );
    }
    pub fn update_worker_load(&self, worker_id: &str, current_load: f64) {
        self.worker_current_load.set(current_load);

        info!(
            worker_id = worker_id,
            current_load = current_load,
            "Worker load updated"
        );
    }
    pub fn update_queue_depth(&self, depth: f64) {
        self.queue_depth.set(depth);
    }
    pub fn record_scheduling_duration(&self, duration_seconds: f64) {
        self.dispatcher_scheduling_duration.record(duration_seconds);
    }
    pub fn record_database_operation(&self, operation: &str, duration_seconds: f64) {
        self.database_operation_duration.record(duration_seconds);

        info!(
            operation = operation,
            duration_seconds = duration_seconds,
            "Database operation completed"
        );
    }
    pub fn record_message_queue_operation(&self, operation: &str, duration_seconds: f64) {
        self.message_queue_operation_duration
            .record(duration_seconds);

        info!(
            operation = operation,
            duration_seconds = duration_seconds,
            "Message queue operation completed"
        );
    }

    // Enhanced task metrics
    pub fn record_task_queue_time(&self, queue_time_seconds: f64) {
        self.task_queue_time.record(queue_time_seconds);
    }

    // Enhanced worker metrics
    pub fn record_worker_heartbeat(&self, worker_id: &str) {
        self.worker_heartbeat_count.increment(1);
        info!(worker_id = worker_id, "Worker heartbeat recorded");
    }

    pub fn record_worker_failure(&self, worker_id: &str, error_type: &str) {
        self.worker_failures_total.increment(1);
        warn!(
            worker_id = worker_id,
            error_type = error_type,
            "Worker failure recorded"
        );
    }

    // Enhanced queue metrics
    pub fn record_queue_processing_rate(&self, rate_per_second: f64) {
        self.queue_processing_rate.record(rate_per_second);
    }

    pub fn record_queue_latency(&self, latency_seconds: f64) {
        self.queue_latency.record(latency_seconds);
    }

    pub fn record_queue_backpressure(&self) {
        self.queue_backpressure_count.increment(1);
        warn!("Queue backpressure event recorded");
    }

    // Enhanced dispatcher metrics
    pub fn record_task_scheduled(&self) {
        self.dispatcher_tasks_scheduled_total.increment(1);
    }

    pub fn record_dependency_resolution(&self, resolution_type: &str) {
        self.dispatcher_dependency_resolutions_total.increment(1);
        info!(
            resolution_type = resolution_type,
            "Dependency resolution completed"
        );
    }

    // Enhanced database metrics
    pub fn update_database_connection_pool(&self, pool_size: f64) {
        self.database_connection_pool_size.set(pool_size);
    }

    pub fn record_database_query(&self, query_type: &str, duration_seconds: f64) {
        self.database_query_count.increment(1);
        self.database_operation_duration.record(duration_seconds);

        // Consider queries over 1 second as slow
        if duration_seconds > 1.0 {
            self.database_slow_query_count.increment(1);
            warn!(
                query_type = query_type,
                duration_seconds = duration_seconds,
                "Slow database query detected"
            );
        }
    }

    pub fn record_database_connection_failure(&self, error_type: &str) {
        self.database_connection_failures.increment(1);
        error!(
            error_type = error_type,
            "Database connection failure recorded"
        );
    }

    // Enhanced message queue metrics
    pub fn record_message_queue_throughput(&self, throughput_per_second: f64) {
        self.message_queue_throughput.record(throughput_per_second);
    }

    pub fn record_message_queue_error(&self, error_type: &str) {
        self.message_queue_errors_total.increment(1);
        error!(error_type = error_type, "Message queue error recorded");
    }

    pub fn update_consumer_lag(&self, lag_seconds: f64) {
        self.message_queue_consumer_lag.set(lag_seconds);
    }

    // API metrics
    pub fn record_api_request(
        &self,
        endpoint: &str,
        method: &str,
        status_code: u16,
        duration_seconds: f64,
    ) {
        self.api_requests_total.increment(1);
        self.api_response_duration.record(duration_seconds);

        if status_code >= 400 {
            self.api_error_rate.increment(1);
        }

        info!(
            endpoint = endpoint,
            method = method,
            status_code = status_code,
            duration_seconds = duration_seconds,
            "API request processed"
        );
    }

    // Cache metrics
    pub fn update_cache_hit_rate(&self, hit_rate_percent: f64) {
        self.cache_hit_rate.set(hit_rate_percent);
    }

    pub fn record_cache_operation(&self, _operation: &str, duration_seconds: f64) {
        self.cache_operation_duration.record(duration_seconds);
    }

    pub fn update_cache_size(&self, size_mb: f64) {
        self.cache_size.set(size_mb);
    }

    // System resource metrics
    pub async fn update_system_metrics(&self, metrics: PerformanceMetrics) {
        self.system_cpu_usage.set(metrics.cpu_usage_percent);
        self.system_memory_usage.set(metrics.memory_usage_mb);
        self.system_disk_usage.set(metrics.disk_usage_mb);
        self.system_network_io.set(metrics.network_io_mb);

        // Store for regression detection
        let mut history = self.performance_history.write().await;
        history.push(metrics.clone());

        // Keep only last 1000 entries to prevent memory growth
        if history.len() > 1000 {
            history.remove(0);
        }

        info!(
            cpu_usage = metrics.cpu_usage_percent,
            memory_usage = metrics.memory_usage_mb,
            disk_usage = metrics.disk_usage_mb,
            network_io = metrics.network_io_mb,
            "System metrics updated"
        );
    }

    // Performance regression detection
    pub async fn detect_performance_regressions(&self) -> Vec<String> {
        let history = self.performance_history.read().await;
        let mut regressions = Vec::new();

        if history.len() < 10 {
            return regressions; // Not enough data
        }

        // Analyze trends in the last 100 entries
        let recent_data: Vec<_> = history.iter().rev().take(100).collect();

        // Calculate average of recent data
        let recent_avg = PerformanceMetrics {
            cpu_usage_percent: recent_data.iter().map(|m| m.cpu_usage_percent).sum::<f64>()
                / recent_data.len() as f64,
            memory_usage_mb: recent_data.iter().map(|m| m.memory_usage_mb).sum::<f64>()
                / recent_data.len() as f64,
            disk_usage_mb: recent_data.iter().map(|m| m.disk_usage_mb).sum::<f64>()
                / recent_data.len() as f64,
            network_io_mb: recent_data.iter().map(|m| m.network_io_mb).sum::<f64>()
                / recent_data.len() as f64,
            timestamp: std::time::SystemTime::now(),
        };

        // Compare with older data (100-200 entries ago)
        let older_data: Vec<_> = history.iter().rev().skip(100).take(100).collect();
        if !older_data.is_empty() {
            let older_avg = PerformanceMetrics {
                cpu_usage_percent: older_data.iter().map(|m| m.cpu_usage_percent).sum::<f64>()
                    / older_data.len() as f64,
                memory_usage_mb: older_data.iter().map(|m| m.memory_usage_mb).sum::<f64>()
                    / older_data.len() as f64,
                disk_usage_mb: older_data.iter().map(|m| m.disk_usage_mb).sum::<f64>()
                    / older_data.len() as f64,
                network_io_mb: older_data.iter().map(|m| m.network_io_mb).sum::<f64>()
                    / older_data.len() as f64,
                timestamp: std::time::SystemTime::now(),
            };

            // Check for significant degradation (>20% increase)
            if recent_avg.cpu_usage_percent > older_avg.cpu_usage_percent * 1.2 {
                regressions.push(format!(
                    "CPU usage increased by {:.1}% ({}% → {}%)",
                    (recent_avg.cpu_usage_percent - older_avg.cpu_usage_percent),
                    older_avg.cpu_usage_percent,
                    recent_avg.cpu_usage_percent
                ));
            }

            if recent_avg.memory_usage_mb > older_avg.memory_usage_mb * 1.2 {
                regressions.push(format!(
                    "Memory usage increased by {:.1}MB ({}MB → {}MB)",
                    recent_avg.memory_usage_mb - older_avg.memory_usage_mb,
                    older_avg.memory_usage_mb,
                    recent_avg.memory_usage_mb
                ));
            }

            if recent_avg.disk_usage_mb > older_avg.disk_usage_mb * 1.1 {
                regressions.push(format!(
                    "Disk usage increased by {:.1}MB ({}MB → {}MB)",
                    recent_avg.disk_usage_mb - older_avg.disk_usage_mb,
                    older_avg.disk_usage_mb,
                    recent_avg.disk_usage_mb
                ));
            }
        }

        if !regressions.is_empty() {
            warn!(regressions = ?regressions, "Performance regressions detected");
        }

        regressions
    }

    // Get current performance summary
    pub async fn get_performance_summary(&self) -> Result<String> {
        let history = self.performance_history.read().await;

        if history.is_empty() {
            return Ok("No performance data available".to_string());
        }

        let latest = &history[history.len() - 1];
        let avg_cpu =
            history.iter().map(|m| m.cpu_usage_percent).sum::<f64>() / history.len() as f64;
        let avg_memory =
            history.iter().map(|m| m.memory_usage_mb).sum::<f64>() / history.len() as f64;

        Ok(format!(
            "Performance Summary ({} data points):\n- Current CPU: {:.1}% (Avg: {:.1}%)\n- Current Memory: {:.1}MB (Avg: {:.1}MB)\n- Current Disk: {:.1}MB\n- Current Network I/O: {:.1}MB",
            history.len(),
            latest.cpu_usage_percent,
            avg_cpu,
            latest.memory_usage_mb,
            avg_memory,
            latest.disk_usage_mb,
            latest.network_io_mb
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_metrics_collector_new() {
        // Test that MetricsCollector can be created
        let result = MetricsCollector::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_performance_metrics_creation() {
        let timestamp = SystemTime::now();
        let metrics = PerformanceMetrics {
            cpu_usage_percent: 50.5,
            memory_usage_mb: 1024.0,
            disk_usage_mb: 2048.0,
            network_io_mb: 100.0,
            timestamp,
        };

        assert_eq!(metrics.cpu_usage_percent, 50.5);
        assert_eq!(metrics.memory_usage_mb, 1024.0);
        assert_eq!(metrics.disk_usage_mb, 2048.0);
        assert_eq!(metrics.network_io_mb, 100.0);
        assert_eq!(metrics.timestamp, timestamp);
    }

    #[test]
    fn test_performance_metrics_clone() {
        let timestamp = SystemTime::now();
        let metrics = PerformanceMetrics {
            cpu_usage_percent: 75.0,
            memory_usage_mb: 2048.0,
            disk_usage_mb: 4096.0,
            network_io_mb: 200.0,
            timestamp,
        };

        let cloned_metrics = metrics.clone();
        assert_eq!(cloned_metrics.cpu_usage_percent, metrics.cpu_usage_percent);
        assert_eq!(cloned_metrics.memory_usage_mb, metrics.memory_usage_mb);
        assert_eq!(cloned_metrics.disk_usage_mb, metrics.disk_usage_mb);
        assert_eq!(cloned_metrics.network_io_mb, metrics.network_io_mb);
        assert_eq!(cloned_metrics.timestamp, metrics.timestamp);
    }

    #[test]
    fn test_performance_metrics_debug() {
        let timestamp = SystemTime::now();
        let metrics = PerformanceMetrics {
            cpu_usage_percent: 25.0,
            memory_usage_mb: 512.0,
            disk_usage_mb: 1024.0,
            network_io_mb: 50.0,
            timestamp,
        };

        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("PerformanceMetrics"));
        assert!(debug_str.contains("cpu_usage_percent"));
        assert!(debug_str.contains("memory_usage_mb"));
    }

    #[tokio::test]
    async fn test_record_task_execution() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording successful task execution
        collector.record_task_execution("shell", "completed", 1.5);

        // Test recording failed task execution
        collector.record_task_execution("http", "failed", 2.0);

        // Test with different task types
        collector.record_task_execution("database", "completed", 0.5);
    }

    #[tokio::test]
    async fn test_record_task_failure() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording different types of task failures
        collector.record_task_failure("shell", "timeout");
        collector.record_task_failure("http", "connection_error");
        collector.record_task_failure("database", "query_error");
    }

    #[tokio::test]
    async fn test_record_task_retry() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording task retries with different counts
        collector.record_task_retry("shell", 1);
        collector.record_task_retry("http", 5);
        collector.record_task_retry("database", 10);
    }

    #[tokio::test]
    async fn test_update_active_workers() {
        let collector = MetricsCollector::new().unwrap();

        // Test updating active workers count
        collector.update_active_workers(5.0);
        collector.update_active_workers(10.0);
        collector.update_active_workers(0.0);
    }

    #[tokio::test]
    async fn test_update_worker_capacity() {
        let collector = MetricsCollector::new().unwrap();

        // Test updating worker capacity
        collector.update_worker_capacity("worker-1", 10.0);
        collector.update_worker_capacity("worker-2", 20.0);
        collector.update_worker_capacity("worker-3", 5.0);
    }

    #[tokio::test]
    async fn test_update_worker_load() {
        let collector = MetricsCollector::new().unwrap();

        // Test updating worker load
        collector.update_worker_load("worker-1", 3.0);
        collector.update_worker_load("worker-2", 8.0);
        collector.update_worker_load("worker-3", 1.0);
    }

    #[tokio::test]
    async fn test_update_queue_depth() {
        let collector = MetricsCollector::new().unwrap();

        // Test updating queue depth
        collector.update_queue_depth(100.0);
        collector.update_queue_depth(500.0);
        collector.update_queue_depth(0.0);
    }

    #[tokio::test]
    async fn test_record_scheduling_duration() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording scheduling duration
        collector.record_scheduling_duration(0.1);
        collector.record_scheduling_duration(1.0);
        collector.record_scheduling_duration(5.0);
    }

    #[tokio::test]
    async fn test_record_database_operation() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording database operations
        collector.record_database_operation("SELECT", 0.05);
        collector.record_database_operation("INSERT", 0.1);
        collector.record_database_operation("UPDATE", 0.2);
        collector.record_database_operation("DELETE", 0.03);
    }

    #[tokio::test]
    async fn test_record_message_queue_operation() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording message queue operations
        collector.record_message_queue_operation("publish", 0.01);
        collector.record_message_queue_operation("consume", 0.02);
        collector.record_message_queue_operation("ack", 0.005);
    }

    #[tokio::test]
    async fn test_record_task_queue_time() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording task queue time
        collector.record_task_queue_time(0.5);
        collector.record_task_queue_time(2.0);
        collector.record_task_queue_time(10.0);
    }

    #[tokio::test]
    async fn test_record_worker_heartbeat() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording worker heartbeats
        collector.record_worker_heartbeat("worker-1");
        collector.record_worker_heartbeat("worker-2");
        collector.record_worker_heartbeat("worker-3");
    }

    #[tokio::test]
    async fn test_record_worker_failure() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording worker failures
        collector.record_worker_failure("worker-1", "connection_lost");
        collector.record_worker_failure("worker-2", "timeout");
        collector.record_worker_failure("worker-3", "crash");
    }

    #[tokio::test]
    async fn test_record_queue_processing_rate() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording queue processing rate
        collector.record_queue_processing_rate(10.5);
        collector.record_queue_processing_rate(25.0);
        collector.record_queue_processing_rate(100.0);
    }

    #[tokio::test]
    async fn test_record_queue_latency() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording queue latency
        collector.record_queue_latency(0.1);
        collector.record_queue_latency(0.5);
        collector.record_queue_latency(2.0);
    }

    #[tokio::test]
    async fn test_record_queue_backpressure() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording queue backpressure
        collector.record_queue_backpressure();
        collector.record_queue_backpressure();
        collector.record_queue_backpressure();
    }

    #[tokio::test]
    async fn test_record_task_scheduled() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording task scheduling
        collector.record_task_scheduled();
        collector.record_task_scheduled();
        collector.record_task_scheduled();
    }

    #[tokio::test]
    async fn test_record_dependency_resolution() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording dependency resolution
        collector.record_dependency_resolution("success");
        collector.record_dependency_resolution("failed");
        collector.record_dependency_resolution("timeout");
    }

    #[tokio::test]
    async fn test_update_database_connection_pool() {
        let collector = MetricsCollector::new().unwrap();

        // Test updating database connection pool
        collector.update_database_connection_pool(5.0);
        collector.update_database_connection_pool(10.0);
        collector.update_database_connection_pool(20.0);
    }

    #[tokio::test]
    async fn test_record_database_query() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording database queries
        collector.record_database_query("SELECT", 0.05);
        collector.record_database_query("INSERT", 0.1);
        collector.record_database_query("UPDATE", 1.5); // Should trigger slow query warning
        collector.record_database_query("DELETE", 0.02);
    }

    #[tokio::test]
    async fn test_record_database_connection_failure() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording database connection failures
        collector.record_database_connection_failure("connection_refused");
        collector.record_database_connection_failure("timeout");
        collector.record_database_connection_failure("authentication_error");
    }

    #[tokio::test]
    async fn test_record_message_queue_throughput() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording message queue throughput
        collector.record_message_queue_throughput(50.0);
        collector.record_message_queue_throughput(100.0);
        collector.record_message_queue_throughput(200.0);
    }

    #[tokio::test]
    async fn test_record_message_queue_error() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording message queue errors
        collector.record_message_queue_error("connection_lost");
        collector.record_message_queue_error("queue_full");
        collector.record_message_queue_error("serialization_error");
    }

    #[tokio::test]
    async fn test_update_consumer_lag() {
        let collector = MetricsCollector::new().unwrap();

        // Test updating consumer lag
        collector.update_consumer_lag(0.0);
        collector.update_consumer_lag(5.0);
        collector.update_consumer_lag(30.0);
    }

    #[tokio::test]
    async fn test_record_api_request() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording API requests
        collector.record_api_request("/api/tasks", "GET", 200, 0.1);
        collector.record_api_request("/api/workers", "POST", 201, 0.2);
        collector.record_api_request("/api/health", "GET", 404, 0.05); // Should record as error
        collector.record_api_request("/api/system", "GET", 500, 1.0); // Should record as error
    }

    #[tokio::test]
    async fn test_update_cache_hit_rate() {
        let collector = MetricsCollector::new().unwrap();

        // Test updating cache hit rate
        collector.update_cache_hit_rate(95.5);
        collector.update_cache_hit_rate(80.0);
        collector.update_cache_hit_rate(50.0);
    }

    #[tokio::test]
    async fn test_record_cache_operation() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording cache operations
        collector.record_cache_operation("get", 0.001);
        collector.record_cache_operation("set", 0.002);
        collector.record_cache_operation("delete", 0.001);
    }

    #[tokio::test]
    async fn test_update_cache_size() {
        let collector = MetricsCollector::new().unwrap();

        // Test updating cache size
        collector.update_cache_size(10.0);
        collector.update_cache_size(100.0);
        collector.update_cache_size(1000.0);
    }

    #[tokio::test]
    async fn test_update_system_metrics() {
        let collector = MetricsCollector::new().unwrap();

        let timestamp = SystemTime::now();
        let metrics = PerformanceMetrics {
            cpu_usage_percent: 45.5,
            memory_usage_mb: 2048.0,
            disk_usage_mb: 4096.0,
            network_io_mb: 150.0,
            timestamp,
        };

        // Test updating system metrics
        collector.update_system_metrics(metrics).await;
    }

    #[tokio::test]
    async fn test_detect_performance_regressions() {
        let collector = MetricsCollector::new().unwrap();

        // Test with insufficient data
        let regressions = collector.detect_performance_regressions().await;
        assert!(regressions.is_empty());

        // Add some test data
        let now = SystemTime::now();
        for i in 0..150 {
            let timestamp = now - std::time::Duration::from_secs(i);
            let metrics = PerformanceMetrics {
                cpu_usage_percent: 50.0 + (i as f64 * 0.1),
                memory_usage_mb: 1000.0 + (i as f64 * 10.0),
                disk_usage_mb: 2000.0 + (i as f64 * 5.0),
                network_io_mb: 100.0 + (i as f64 * 2.0),
                timestamp,
            };
            collector.update_system_metrics(metrics).await;
        }

        // Test regression detection
        let regressions = collector.detect_performance_regressions().await;
        // Should detect some regressions due to the increasing values
        println!("Detected regressions: {:?}", regressions);
    }

    #[tokio::test]
    async fn test_get_performance_summary() {
        let collector = MetricsCollector::new().unwrap();

        // Test with no data
        let summary = collector.get_performance_summary().await;
        assert!(summary.is_ok());
        assert!(summary.unwrap().contains("No performance data available"));

        // Add some test data
        let now = SystemTime::now();
        for i in 0..10 {
            let timestamp = now - std::time::Duration::from_secs(i);
            let metrics = PerformanceMetrics {
                cpu_usage_percent: 50.0 + (i as f64),
                memory_usage_mb: 1000.0 + (i as f64 * 100.0),
                disk_usage_mb: 2000.0 + (i as f64 * 50.0),
                network_io_mb: 100.0 + (i as f64 * 10.0),
                timestamp,
            };
            collector.update_system_metrics(metrics).await;
        }

        // Test performance summary
        let summary = collector.get_performance_summary().await;
        assert!(summary.is_ok());
        let summary_text = summary.unwrap();
        assert!(summary_text.contains("Performance Summary"));
        assert!(summary_text.contains("data points"));
        assert!(summary_text.contains("Current CPU"));
        assert!(summary_text.contains("Current Memory"));
    }

    #[tokio::test]
    async fn test_performance_history_limit() {
        let collector = MetricsCollector::new().unwrap();

        // Add more than 1000 entries to test the limit
        let now = SystemTime::now();
        for i in 0..1100 {
            let timestamp = now - std::time::Duration::from_secs(i);
            let metrics = PerformanceMetrics {
                cpu_usage_percent: 50.0,
                memory_usage_mb: 1000.0,
                disk_usage_mb: 2000.0,
                network_io_mb: 100.0,
                timestamp,
            };
            collector.update_system_metrics(metrics).await;
        }

        // The history should be limited to 1000 entries
        let summary = collector.get_performance_summary().await;
        assert!(summary.is_ok());
        let summary_text = summary.unwrap();
        assert!(summary_text.contains("1000 data points"));
    }

    #[tokio::test]
    async fn test_edge_cases_and_extreme_values() {
        let collector = MetricsCollector::new().unwrap();

        // Test with extreme values
        collector.record_task_execution("test", "completed", 0.0);
        collector.record_task_execution("test", "completed", 999999.0);
        collector.update_active_workers(-1.0);
        collector.update_active_workers(999999.0);
        collector.record_database_query("test", 0.0);
        collector.record_database_query("test", 999999.0);
        collector.record_api_request("/test", "GET", 999, 0.0);
        collector.record_api_request("/test", "GET", 0, 999999.0);
    }

    #[tokio::test]
    async fn test_concurrent_metric_updates() {
        let collector = MetricsCollector::new().unwrap();
        let collector = std::sync::Arc::new(collector);

        // Test concurrent metric updates
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let collector = collector.clone();
                tokio::spawn(async move {
                    collector.record_task_execution(&format!("task_{}", i), "completed", 1.0);
                    collector.update_active_workers(i as f64);
                    collector.record_database_operation("SELECT", 0.1);
                })
            })
            .collect();

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_metric_recording_with_various_data_types() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording with various string types and lengths
        let long_task_type =
            "very-long-task-type-with-detailed-classification-and-category-1234567890";
        let long_worker_id =
            "very-long-worker-id-with-many-characters-and-unique-identifier-1234567890";
        let long_error_type =
            "very-long-error-type-with-detailed-description-of-what-went-wrong-1234567890";

        collector.record_task_execution(long_task_type, "completed", 1.0);
        collector.record_task_failure("shell", long_error_type);
        collector.update_worker_capacity(long_worker_id, 10.0);
        collector.update_worker_load(long_worker_id, 5.0);
        collector.record_worker_heartbeat(long_worker_id);
        collector.record_worker_failure(long_worker_id, long_error_type);
    }

    #[tokio::test]
    async fn test_metric_recording_with_special_characters() {
        let collector = MetricsCollector::new().unwrap();

        // Test recording with special characters
        let special_task_type = "task@#$%&*()";
        let special_worker_id = "worker-123.456.789_Special@Chars";
        let special_error_type = "error with \"special\" characters & symbols";

        collector.record_task_execution(special_task_type, "completed", 1.0);
        collector.record_task_failure("shell", special_error_type);
        collector.update_worker_capacity(special_worker_id, 10.0);
        collector.update_worker_load(special_worker_id, 5.0);
    }
}
