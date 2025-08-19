use anyhow::Result;
use metrics::{counter, gauge, histogram, Counter, Gauge, Histogram};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

#[derive(Clone)]
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
        let dispatcher_tasks_scheduled_total = counter!("scheduler_dispatcher_tasks_scheduled_total");
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
        info!(resolution_type = resolution_type, "Dependency resolution completed");
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
        error!(error_type = error_type, "Database connection failure recorded");
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
    pub fn record_api_request(&self, endpoint: &str, method: &str, status_code: u16, duration_seconds: f64) {
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
            cpu_usage_percent: recent_data.iter().map(|m| m.cpu_usage_percent).sum::<f64>() / recent_data.len() as f64,
            memory_usage_mb: recent_data.iter().map(|m| m.memory_usage_mb).sum::<f64>() / recent_data.len() as f64,
            disk_usage_mb: recent_data.iter().map(|m| m.disk_usage_mb).sum::<f64>() / recent_data.len() as f64,
            network_io_mb: recent_data.iter().map(|m| m.network_io_mb).sum::<f64>() / recent_data.len() as f64,
            timestamp: std::time::SystemTime::now(),
        };

        // Compare with older data (100-200 entries ago)
        let older_data: Vec<_> = history.iter().rev().skip(100).take(100).collect();
        if !older_data.is_empty() {
            let older_avg = PerformanceMetrics {
                cpu_usage_percent: older_data.iter().map(|m| m.cpu_usage_percent).sum::<f64>() / older_data.len() as f64,
                memory_usage_mb: older_data.iter().map(|m| m.memory_usage_mb).sum::<f64>() / older_data.len() as f64,
                disk_usage_mb: older_data.iter().map(|m| m.disk_usage_mb).sum::<f64>() / older_data.len() as f64,
                network_io_mb: older_data.iter().map(|m| m.network_io_mb).sum::<f64>() / older_data.len() as f64,
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
        let avg_cpu = history.iter().map(|m| m.cpu_usage_percent).sum::<f64>() / history.len() as f64;
        let avg_memory = history.iter().map(|m| m.memory_usage_mb).sum::<f64>() / history.len() as f64;

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
