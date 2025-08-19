use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::metrics_collector::MetricsCollector;
use crate::alerting::AlertManager;
use crate::performance_testing::PerformanceRegressionTester;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub system_metrics: SystemMetrics,
    pub performance_metrics: PerformanceMetricsData,
    pub alerts: Vec<AlertSummary>,
    pub benchmarks: Vec<BenchmarkSummary>,
    pub health_status: HealthStatus,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub memory_usage_percent: f64,
    pub disk_usage_mb: f64,
    pub disk_usage_percent: f64,
    pub network_io_mb: f64,
    pub uptime_seconds: u64,
    pub active_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetricsData {
    pub task_executions_total: u64,
    pub task_execution_duration_avg_ms: f64,
    pub task_failures_total: u64,
    pub task_failure_rate_percent: f64,
    pub active_workers: u32,
    pub worker_capacity_utilization_percent: f64,
    pub queue_depth: u32,
    pub queue_processing_rate_per_second: f64,
    pub database_operation_duration_avg_ms: f64,
    pub database_connection_pool_size: u32,
    pub message_queue_throughput_per_second: f64,
    pub api_requests_total: u64,
    pub api_response_duration_avg_ms: f64,
    pub api_error_rate_percent: f64,
    pub cache_hit_rate_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertSummary {
    pub rule_name: String,
    pub metric_name: String,
    pub severity: String,
    pub current_value: f64,
    pub threshold: f64,
    pub message: String,
    pub triggered_at: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub name: String,
    pub latest_score: f64,
    pub average_score: f64,
    pub trend_direction: String,
    pub last_run: std::time::SystemTime,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub overall: HealthLevel,
    pub components: HashMap<String, HealthLevel>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthLevel {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

pub struct PerformanceDashboard {
    metrics_collector: Arc<MetricsCollector>,
    alert_manager: Arc<AlertManager>,
    regression_tester: Arc<PerformanceRegressionTester>,
    cache: Arc<RwLock<HashMap<String, DashboardData>>>,
    cache_ttl_seconds: u64,
}

impl PerformanceDashboard {
    pub fn new(
        metrics_collector: Arc<MetricsCollector>,
        alert_manager: Arc<AlertManager>,
        regression_tester: Arc<PerformanceRegressionTester>,
    ) -> Self {
        Self {
            metrics_collector,
            alert_manager,
            regression_tester,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl_seconds: 30, // Cache data for 30 seconds
        }
    }

    pub async fn get_dashboard_data(&self) -> Result<DashboardData> {
        // Check cache first
        let cache_key = "dashboard_data".to_string();
        let cache = self.cache.read().await;
        
        if let Some(cached_data) = cache.get(&cache_key) {
            let cache_age = cached_data.timestamp.elapsed().unwrap_or_default().as_secs();
            if cache_age < self.cache_ttl_seconds {
                return Ok(cached_data.clone());
            }
        }
        drop(cache);

        // Generate fresh data
        let dashboard_data = self.generate_dashboard_data().await?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(cache_key, dashboard_data.clone());

        Ok(dashboard_data)
    }

    async fn generate_dashboard_data(&self) -> Result<DashboardData> {
        let system_metrics = self.collect_system_metrics().await?;
        let performance_metrics = self.collect_performance_metrics().await?;
        let alerts = self.collect_alert_data().await?;
        let benchmarks = self.collect_benchmark_data().await?;
        let health_status = self.calculate_health_status(&system_metrics, &performance_metrics, &alerts).await?;

        Ok(DashboardData {
            system_metrics,
            performance_metrics,
            alerts,
            benchmarks,
            health_status,
            timestamp: std::time::SystemTime::now(),
        })
    }

    async fn collect_system_metrics(&self) -> Result<SystemMetrics> {
        // Placeholder for actual system metrics collection
        // In a real implementation, this would use sysinfo or similar
        Ok(SystemMetrics {
            cpu_usage_percent: 45.0,
            memory_usage_mb: 2048.0,
            memory_usage_percent: 60.0,
            disk_usage_mb: 10240.0,
            disk_usage_percent: 40.0,
            network_io_mb: 15.0,
            uptime_seconds: 86400,
            active_connections: 25,
        })
    }

    async fn collect_performance_metrics(&self) -> Result<PerformanceMetricsData> {
        // Placeholder for actual metrics collection
        // In a real implementation, this would query the metrics exporter
        Ok(PerformanceMetricsData {
            task_executions_total: 1250,
            task_execution_duration_avg_ms: 45.0,
            task_failures_total: 25,
            task_failure_rate_percent: 2.0,
            active_workers: 8,
            worker_capacity_utilization_percent: 65.0,
            queue_depth: 150,
            queue_processing_rate_per_second: 85.0,
            database_operation_duration_avg_ms: 25.0,
            database_connection_pool_size: 10,
            message_queue_throughput_per_second: 450.0,
            api_requests_total: 3200,
            api_response_duration_avg_ms: 120.0,
            api_error_rate_percent: 1.5,
            cache_hit_rate_percent: 92.0,
        })
    }

    async fn collect_alert_data(&self) -> Result<Vec<AlertSummary>> {
        let alerts = self.alert_manager.get_active_alerts().await;
        
        let mut alert_summaries = Vec::new();
        for alert in alerts {
            alert_summaries.push(AlertSummary {
                rule_name: alert.rule_name,
                metric_name: alert.metric_name,
                severity: format!("{:?}", alert.severity),
                current_value: alert.current_value,
                threshold: alert.threshold,
                message: alert.message,
                triggered_at: alert.timestamp,
            });
        }

        Ok(alert_summaries)
    }

    async fn collect_benchmark_data(&self) -> Result<Vec<BenchmarkSummary>> {
        let mut benchmark_summaries = Vec::new();
        
        // Get benchmark names
        let benchmarks = self.regression_tester.get_benchmarks().await;
        
        for benchmark in benchmarks.iter() {
            if let Ok(Some(trend)) = self.regression_tester.get_performance_trends(&benchmark.name).await {
                benchmark_summaries.push(BenchmarkSummary {
                    name: benchmark.name.clone(),
                    latest_score: trend.latest_result.overall_score,
                    average_score: trend.average_score,
                    trend_direction: format!("{:?}", trend.trend_direction),
                    last_run: trend.latest_result.timestamp,
                    passed: trend.latest_result.passed,
                });
            }
        }

        Ok(benchmark_summaries)
    }

    async fn calculate_health_status(
        &self,
        system_metrics: &SystemMetrics,
        performance_metrics: &PerformanceMetricsData,
        alerts: &[AlertSummary],
    ) -> Result<HealthStatus> {
        let mut components = HashMap::new();
        let mut issues = Vec::new();

        // System health
        let system_health = if system_metrics.cpu_usage_percent > 90.0 || system_metrics.memory_usage_percent > 90.0 {
            components.insert("system".to_string(), HealthLevel::Critical);
            issues.push("High system resource usage detected".to_string());
            HealthLevel::Critical
        } else if system_metrics.cpu_usage_percent > 75.0 || system_metrics.memory_usage_percent > 75.0 {
            components.insert("system".to_string(), HealthLevel::Warning);
            issues.push("Elevated system resource usage".to_string());
            HealthLevel::Warning
        } else {
            components.insert("system".to_string(), HealthLevel::Healthy);
            HealthLevel::Healthy
        };

        // Performance health
        let performance_health = if performance_metrics.task_failure_rate_percent > 10.0 || performance_metrics.api_error_rate_percent > 5.0 {
            components.insert("performance".to_string(), HealthLevel::Critical);
            issues.push("High error rates detected".to_string());
            HealthLevel::Critical
        } else if performance_metrics.task_failure_rate_percent > 5.0 || performance_metrics.api_error_rate_percent > 2.0 {
            components.insert("performance".to_string(), HealthLevel::Warning);
            issues.push("Elevated error rates".to_string());
            HealthLevel::Warning
        } else {
            components.insert("performance".to_string(), HealthLevel::Healthy);
            HealthLevel::Healthy
        };

        // Queue health
        let queue_health = if performance_metrics.queue_depth > 1000 {
            components.insert("queue".to_string(), HealthLevel::Critical);
            issues.push("High queue depth detected".to_string());
            HealthLevel::Critical
        } else if performance_metrics.queue_depth > 500 {
            components.insert("queue".to_string(), HealthLevel::Warning);
            issues.push("Elevated queue depth".to_string());
            HealthLevel::Warning
        } else {
            components.insert("queue".to_string(), HealthLevel::Healthy);
            HealthLevel::Healthy
        };

        // Database health
        let database_health = if performance_metrics.database_operation_duration_avg_ms > 500.0 {
            components.insert("database".to_string(), HealthLevel::Critical);
            issues.push("Slow database operations detected".to_string());
            HealthLevel::Critical
        } else if performance_metrics.database_operation_duration_avg_ms > 200.0 {
            components.insert("database".to_string(), HealthLevel::Warning);
            issues.push("Slow database queries".to_string());
            HealthLevel::Warning
        } else {
            components.insert("database".to_string(), HealthLevel::Healthy);
            HealthLevel::Healthy
        };

        // Cache health
        let cache_health = if performance_metrics.cache_hit_rate_percent < 70.0 {
            components.insert("cache".to_string(), HealthLevel::Critical);
            issues.push("Low cache hit rate detected".to_string());
            HealthLevel::Critical
        } else if performance_metrics.cache_hit_rate_percent < 85.0 {
            components.insert("cache".to_string(), HealthLevel::Warning);
            issues.push("Suboptimal cache hit rate".to_string());
            HealthLevel::Warning
        } else {
            components.insert("cache".to_string(), HealthLevel::Healthy);
            HealthLevel::Healthy
        };

        // Alert-based health impact
        let critical_alerts = alerts.iter().filter(|a| a.severity == "Critical").count() as u32;
        let warning_alerts = alerts.iter().filter(|a| a.severity == "Warning").count() as u32;

        // Calculate overall health
        let overall = if critical_alerts > 0 || components.values().any(|h| *h == HealthLevel::Critical) {
            HealthLevel::Critical
        } else if warning_alerts > 2 || components.values().any(|h| *h == HealthLevel::Warning) {
            HealthLevel::Warning
        } else {
            HealthLevel::Healthy
        };

        Ok(HealthStatus {
            overall,
            components,
            issues,
        })
    }

    pub async fn get_metrics_json(&self) -> Result<String> {
        let dashboard_data = self.get_dashboard_data().await?;
        Ok(serde_json::to_string_pretty(&dashboard_data)?)
    }

    pub async fn get_system_metrics_summary(&self) -> Result<String> {
        let dashboard_data = self.get_dashboard_data().await?;
        
        Ok(format!(
            "System Metrics Summary:\n\
            - CPU Usage: {:.1}%\n\
            - Memory Usage: {:.1}MB ({:.1}%)\n\
            - Disk Usage: {:.1}MB ({:.1}%)\n\
            - Network I/O: {:.1}MB\n\
            - Uptime: {} seconds\n\
            - Active Connections: {}\n\
            - Overall Health: {:?}",
            dashboard_data.system_metrics.cpu_usage_percent,
            dashboard_data.system_metrics.memory_usage_mb,
            dashboard_data.system_metrics.memory_usage_percent,
            dashboard_data.system_metrics.disk_usage_mb,
            dashboard_data.system_metrics.disk_usage_percent,
            dashboard_data.system_metrics.network_io_mb,
            dashboard_data.system_metrics.uptime_seconds,
            dashboard_data.system_metrics.active_connections,
            dashboard_data.health_status.overall
        ))
    }

    pub async fn get_performance_summary(&self) -> Result<String> {
        let dashboard_data = self.get_dashboard_data().await?;
        
        Ok(format!(
            "Performance Metrics Summary:\n\
            - Task Executions: {}\n\
            - Task Failure Rate: {:.1}%\n\
            - Active Workers: {}\n\
            - Queue Depth: {}\n\
            - Queue Processing Rate: {:.1}/sec\n\
            - Database Operation Duration: {:.1}ms\n\
            - API Response Duration: {:.1}ms\n\
            - API Error Rate: {:.1}%\n\
            - Cache Hit Rate: {:.1}%",
            dashboard_data.performance_metrics.task_executions_total,
            dashboard_data.performance_metrics.task_failure_rate_percent,
            dashboard_data.performance_metrics.active_workers,
            dashboard_data.performance_metrics.queue_depth,
            dashboard_data.performance_metrics.queue_processing_rate_per_second,
            dashboard_data.performance_metrics.database_operation_duration_avg_ms,
            dashboard_data.performance_metrics.api_response_duration_avg_ms,
            dashboard_data.performance_metrics.api_error_rate_percent,
            dashboard_data.performance_metrics.cache_hit_rate_percent
        ))
    }

    pub async fn get_alert_summary(&self) -> Result<String> {
        let dashboard_data = self.get_dashboard_data().await?;
        
        let critical_count = dashboard_data.alerts.iter().filter(|a| a.severity == "Critical").count();
        let warning_count = dashboard_data.alerts.iter().filter(|a| a.severity == "Warning").count();
        
        if dashboard_data.alerts.is_empty() {
            return Ok("No active alerts".to_string());
        }

        let mut summary = format!("Active Alerts ({} total):\n", dashboard_data.alerts.len());
        summary.push_str(&format!("- Critical: {}\n", critical_count));
        summary.push_str(&format!("- Warning: {}\n\n", warning_count));

        for alert in &dashboard_data.alerts {
            summary.push_str(&format!(
                "- {}: {} ({}) - Current: {}, Threshold: {}\n",
                alert.rule_name, alert.message, alert.severity, alert.current_value, alert.threshold
            ));
        }

        Ok(summary)
    }

    pub async fn get_benchmark_summary(&self) -> Result<String> {
        let dashboard_data = self.get_dashboard_data().await?;
        
        if dashboard_data.benchmarks.is_empty() {
            return Ok("No benchmark data available".to_string());
        }

        let mut summary = format!("Benchmark Summary ({} benchmarks):\n", dashboard_data.benchmarks.len());
        
        for benchmark in &dashboard_data.benchmarks {
            let status = if benchmark.passed { "✓" } else { "✗" };
            summary.push_str(&format!(
                "- {} {} (Score: {:.1}, Avg: {:.1}, Trend: {})\n",
                status, benchmark.name, benchmark.latest_score, benchmark.average_score, benchmark.trend_direction
            ));
        }

        Ok(summary)
    }
}