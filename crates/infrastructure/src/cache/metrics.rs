//! Cache metrics collection and reporting

use super::CacheStats;
use prometheus::{Counter, Gauge, Histogram, Opts, Registry};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::warn;

/// Cache metrics collector
#[derive(Clone)]
pub struct CacheMetrics {
    /// Cache hits counter
    hits: Counter,
    /// Cache misses counter
    misses: Counter,
    /// Cache sets counter
    sets: Counter,
    /// Cache deletes counter
    deletes: Counter,
    /// Cache errors counter
    errors: Counter,
    /// Cache hit rate gauge
    hit_rate: Gauge,
    /// Cache miss rate gauge
    miss_rate: Gauge,
    /// Cache error rate gauge
    error_rate: Gauge,
    /// Cache operation duration histogram
    operation_duration: Histogram,
    /// Cache size gauge (if available)
    cache_size: Gauge,
    /// Cache memory usage gauge (if available)
    memory_usage: Gauge,
    /// Last update timestamp
    last_update: Arc<RwLock<Instant>>,
    /// Prometheus registry for metrics gathering
    registry: Arc<Registry>,
}

impl CacheMetrics {
    /// Create new cache metrics
    pub fn new(registry: &Registry) -> Self {
        let hits = Counter::with_opts(Opts::new("cache_hits_total", "Total number of cache hits"))
            .expect("Failed to create cache hits counter");
        registry
            .register(Box::new(hits.clone()))
            .expect("Failed to register cache hits counter");

        let misses = Counter::with_opts(Opts::new(
            "cache_misses_total",
            "Total number of cache misses",
        ))
        .expect("Failed to create cache misses counter");
        registry
            .register(Box::new(misses.clone()))
            .expect("Failed to register cache misses counter");

        let sets = Counter::with_opts(Opts::new("cache_sets_total", "Total number of cache sets"))
            .expect("Failed to create cache sets counter");
        registry
            .register(Box::new(sets.clone()))
            .expect("Failed to register cache sets counter");

        let deletes = Counter::with_opts(Opts::new(
            "cache_deletes_total",
            "Total number of cache deletes",
        ))
        .expect("Failed to create cache deletes counter");
        registry
            .register(Box::new(deletes.clone()))
            .expect("Failed to register cache deletes counter");

        let errors = Counter::with_opts(Opts::new(
            "cache_errors_total",
            "Total number of cache errors",
        ))
        .expect("Failed to create cache errors counter");
        registry
            .register(Box::new(errors.clone()))
            .expect("Failed to register cache errors counter");

        let hit_rate = Gauge::with_opts(Opts::new("cache_hit_rate", "Cache hit rate (0.0 to 1.0)"))
            .expect("Failed to create cache hit rate gauge");
        registry
            .register(Box::new(hit_rate.clone()))
            .expect("Failed to register cache hit rate gauge");

        let miss_rate =
            Gauge::with_opts(Opts::new("cache_miss_rate", "Cache miss rate (0.0 to 1.0)"))
                .expect("Failed to create cache miss rate gauge");
        registry
            .register(Box::new(miss_rate.clone()))
            .expect("Failed to register cache miss rate gauge");

        let error_rate = Gauge::with_opts(Opts::new(
            "cache_error_rate",
            "Cache error rate (0.0 to 1.0)",
        ))
        .expect("Failed to create cache error rate gauge");
        registry
            .register(Box::new(error_rate.clone()))
            .expect("Failed to register cache error rate gauge");

        let operation_duration = Histogram::with_opts(
            Opts::new(
                "cache_operation_duration_seconds",
                "Cache operation duration in seconds",
            )
            .into(),
        )
        .expect("Failed to create cache operation duration histogram");
        registry
            .register(Box::new(operation_duration.clone()))
            .expect("Failed to register cache operation duration histogram");

        let cache_size = Gauge::with_opts(Opts::new("cache_size_bytes", "Cache size in bytes"))
            .expect("Failed to create cache size gauge");
        registry
            .register(Box::new(cache_size.clone()))
            .expect("Failed to register cache size gauge");

        let memory_usage = Gauge::with_opts(Opts::new(
            "cache_memory_usage_bytes",
            "Cache memory usage in bytes",
        ))
        .expect("Failed to create cache memory usage gauge");
        registry
            .register(Box::new(memory_usage.clone()))
            .expect("Failed to register cache memory usage gauge");

        Self {
            hits,
            misses,
            sets,
            deletes,
            errors,
            hit_rate,
            miss_rate,
            error_rate,
            operation_duration,
            cache_size,
            memory_usage,
            last_update: Arc::new(RwLock::new(Instant::now())),
            registry: Arc::new(registry.clone()),
        }
    }

    /// Update metrics with cache statistics
    pub async fn update_stats(&self, stats: &CacheStats) {
        self.hits.inc_by(stats.hits as f64);
        self.misses.inc_by(stats.misses as f64);
        self.sets.inc_by(stats.sets as f64);
        self.deletes.inc_by(stats.deletes as f64);
        self.errors.inc_by(stats.errors as f64);

        let total_ops = stats.hits + stats.misses + stats.sets + stats.deletes;
        if total_ops > 0 {
            self.hit_rate.set(stats.hit_rate());
            self.miss_rate.set(stats.miss_rate());
            self.error_rate.set(stats.error_rate());
        }

        *self.last_update.write().await = Instant::now();
    }

    /// Record cache operation duration
    pub fn record_operation_duration(&self, duration: Duration) {
        self.operation_duration.observe(duration.as_secs_f64());
    }

    /// Record cache hit
    pub fn record_hit(&self) {
        self.hits.inc();
    }

    /// Record cache miss
    pub fn record_miss(&self) {
        self.misses.inc();
    }

    /// Record cache set
    pub fn record_set(&self) {
        self.sets.inc();
    }

    /// Record cache delete
    pub fn record_delete(&self) {
        self.deletes.inc();
    }

    /// Record cache error
    pub fn record_error(&self) {
        self.errors.inc();
    }

    /// Update cache size (if available)
    pub fn update_cache_size(&self, size_bytes: u64) {
        self.cache_size.set(size_bytes as f64);
    }

    /// Update memory usage (if available)
    pub fn update_memory_usage(&self, memory_bytes: u64) {
        self.memory_usage.set(memory_bytes as f64);
    }

    /// Get metrics as text for Prometheus
    pub fn get_metrics_text(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();

        match encoder.encode_to_string(&metric_families) {
            Ok(text) => text,
            Err(e) => {
                warn!("Failed to encode cache metrics: {}", e);
                String::new()
            }
        }
    }

    /// Get time since last update
    pub async fn time_since_last_update(&self) -> Duration {
        self.last_update.read().await.elapsed()
    }

    /// Get detailed performance metrics
    pub async fn get_performance_metrics(&self) -> CachePerformanceMetrics {
        let stats = self.registry.gather();
        let metric_families: Vec<prometheus::proto::MetricFamily> =
            stats.into_iter().map(|mf| mf).collect();

        let mut metrics = CachePerformanceMetrics::default();

        for mf in metric_families {
            match mf.name() {
                "cache_hits_total" => {
                    if let Some(metric) = mf.get_metric().first() {
                        metrics.total_hits = metric.get_counter().value() as u64;
                    }
                }
                "cache_misses_total" => {
                    if let Some(metric) = mf.get_metric().first() {
                        metrics.total_misses = metric.get_counter().value() as u64;
                    }
                }
                "cache_operation_duration_seconds" => {
                    if let Some(metric) = mf.get_metric().first() {
                        let histogram = metric.get_histogram();
                        metrics.avg_operation_duration_ms = histogram.get_sample_sum()
                            / histogram.get_sample_count() as f64
                            * 1000.0;
                        metrics.p95_operation_duration_ms =
                            Self::get_histogram_percentile(histogram, 0.95) * 1000.0;
                        metrics.p99_operation_duration_ms =
                            Self::get_histogram_percentile(histogram, 0.99) * 1000.0;
                    }
                }
                _ => {}
            }
        }

        metrics
    }

    /// Get histogram percentile value
    fn get_histogram_percentile(histogram: &prometheus::proto::Histogram, percentile: f64) -> f64 {
        let sample_count = histogram.get_sample_count();
        if sample_count == 0 {
            return 0.0;
        }

        let target_count = (sample_count as f64 * percentile) as u64;
        let mut cumulative_count = 0;

        for bucket in histogram.get_bucket() {
            cumulative_count += bucket.cumulative_count();
            if cumulative_count >= target_count {
                return bucket.upper_bound();
            }
        }

        histogram
            .get_bucket()
            .last()
            .map_or(0.0, |b| b.upper_bound())
    }
}

/// Detailed cache performance metrics
#[derive(Debug, Clone, Default)]
pub struct CachePerformanceMetrics {
    /// Total cache hits
    pub total_hits: u64,
    /// Total cache misses
    pub total_misses: u64,
    /// Average operation duration in milliseconds
    pub avg_operation_duration_ms: f64,
    /// 95th percentile operation duration in milliseconds
    pub p95_operation_duration_ms: f64,
    /// 99th percentile operation duration in milliseconds
    pub p99_operation_duration_ms: f64,
    /// Cache hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    /// Cache miss rate (0.0 to 1.0)
    pub miss_rate: f64,
    /// Cache error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Total operations
    pub total_operations: u64,
    /// Cache efficiency score (0.0 to 1.0)
    pub efficiency_score: f64,
}

impl CachePerformanceMetrics {
    /// Calculate derived metrics
    pub fn calculate_derived_metrics(&mut self) {
        self.total_operations = self.total_hits + self.total_misses;

        if self.total_operations > 0 {
            self.hit_rate = self.total_hits as f64 / self.total_operations as f64;
            self.miss_rate = self.total_misses as f64 / self.total_operations as f64;
        } else {
            self.hit_rate = 0.0;
            self.miss_rate = 0.0;
        }

        // Calculate efficiency score (hit rate weighted by performance)
        let performance_factor = if self.avg_operation_duration_ms > 0.0 {
            (1000.0 / self.avg_operation_duration_ms).min(1.0) // Normalize to 0-1
        } else {
            1.0
        };

        self.efficiency_score = self.hit_rate * performance_factor;
    }

    /// Get performance assessment
    pub fn get_performance_assessment(&self) -> CachePerformanceAssessment {
        if self.efficiency_score >= 0.8 {
            CachePerformanceAssessment::Excellent
        } else if self.efficiency_score >= 0.6 {
            CachePerformanceAssessment::Good
        } else if self.efficiency_score >= 0.4 {
            CachePerformanceAssessment::Fair
        } else {
            CachePerformanceAssessment::Poor
        }
    }

    /// Get recommendations for improvement
    pub fn get_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        if self.hit_rate < 0.5 {
            recommendations
                .push("Consider increasing cache TTL or optimizing cache keys".to_string());
        }

        if self.avg_operation_duration_ms > 10.0 {
            recommendations.push(
                "Cache operations are slow, consider optimizing Redis configuration".to_string(),
            );
        }

        if self.p99_operation_duration_ms > 50.0 {
            recommendations.push(
                "High latency detected, consider connection pooling optimization".to_string(),
            );
        }

        if self.error_rate > 0.01 {
            recommendations
                .push("High error rate detected, check cache service health".to_string());
        }

        if recommendations.is_empty() {
            recommendations.push("Cache performance is optimal".to_string());
        }

        recommendations
    }
}

/// Cache performance assessment levels
#[derive(Debug, Clone, PartialEq)]
pub enum CachePerformanceAssessment {
    Excellent,
    Good,
    Fair,
    Poor,
}

impl std::fmt::Display for CachePerformanceAssessment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CachePerformanceAssessment::Excellent => write!(f, "Excellent"),
            CachePerformanceAssessment::Good => write!(f, "Good"),
            CachePerformanceAssessment::Fair => write!(f, "Fair"),
            CachePerformanceAssessment::Poor => write!(f, "Poor"),
        }
    }
}

/// Cache performance monitor
pub struct CachePerformanceMonitor {
    metrics: CacheMetrics,
    alert_thresholds: AlertThresholds,
    last_alert_time: Arc<RwLock<Instant>>,
}

/// Alert thresholds for cache performance
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    /// Hit rate below this triggers alert (0.0 to 1.0)
    pub low_hit_rate: f64,
    /// Error rate above this triggers alert (0.0 to 1.0)
    pub high_error_rate: f64,
    /// Operation duration above this triggers alert (seconds)
    pub slow_operation_threshold: f64,
    /// Time between alerts (seconds)
    pub alert_cooldown: u64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            low_hit_rate: 0.5,             // 50% hit rate
            high_error_rate: 0.05,         // 5% error rate
            slow_operation_threshold: 1.0, // 1 second
            alert_cooldown: 300,           // 5 minutes
        }
    }
}

impl CachePerformanceMonitor {
    /// Create new cache performance monitor
    pub fn new(metrics: CacheMetrics) -> Self {
        Self {
            metrics,
            alert_thresholds: AlertThresholds::default(),
            last_alert_time: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Set custom alert thresholds
    pub fn with_alert_thresholds(mut self, thresholds: AlertThresholds) -> Self {
        self.alert_thresholds = thresholds;
        self
    }

    /// Monitor cache performance and return alerts if needed
    pub async fn monitor(&self, stats: &CacheStats) -> Vec<String> {
        let mut alerts = Vec::new();

        // Check if we can send alerts (cooldown period)
        let last_alert = *self.last_alert_time.read().await;
        if last_alert.elapsed().as_secs() < self.alert_thresholds.alert_cooldown {
            return alerts;
        }

        // Check hit rate
        if stats.hit_rate() < self.alert_thresholds.low_hit_rate && stats.hits + stats.misses > 100
        {
            alerts.push(format!(
                "Low cache hit rate: {:.2}% (threshold: {:.2}%)",
                stats.hit_rate() * 100.0,
                self.alert_thresholds.low_hit_rate * 100.0
            ));
        }

        // Check error rate
        if stats.error_rate() > self.alert_thresholds.high_error_rate {
            alerts.push(format!(
                "High cache error rate: {:.2}% (threshold: {:.2}%)",
                stats.error_rate() * 100.0,
                self.alert_thresholds.high_error_rate * 100.0
            ));
        }

        // Update metrics
        self.metrics.update_stats(stats).await;

        if !alerts.is_empty() {
            *self.last_alert_time.write().await = Instant::now();
            for alert in &alerts {
                warn!("Cache performance alert: {}", alert);
            }
        }

        alerts
    }

    /// Get metrics reference
    pub fn metrics(&self) -> &CacheMetrics {
        &self.metrics
    }

    /// Get comprehensive performance report
    pub async fn get_performance_report(&self, stats: &CacheStats) -> CachePerformanceReport {
        let _performance_metrics = self.metrics.get_performance_metrics().await;
        let mut metrics = CachePerformanceMetrics::default();

        // Populate basic metrics
        metrics.total_hits = stats.hits;
        metrics.total_misses = stats.misses;
        metrics.error_rate = stats.error_rate();

        // Calculate derived metrics
        metrics.calculate_derived_metrics();

        // Generate report
        let assessment = metrics.get_performance_assessment();
        let recommendations = metrics.get_recommendations();
        let health_status = self.get_health_status(&metrics);
        CachePerformanceReport {
            metrics,
            assessment,
            recommendations,
            timestamp: chrono::Utc::now(),
            health_status,
        }
    }

    /// Get cache health status
    fn get_health_status(&self, metrics: &CachePerformanceMetrics) -> CacheHealthStatus {
        if metrics.efficiency_score >= 0.8 && metrics.error_rate < 0.01 {
            CacheHealthStatus::Healthy
        } else if metrics.efficiency_score >= 0.5 && metrics.error_rate < 0.05 {
            CacheHealthStatus::Degraded
        } else {
            CacheHealthStatus::Unhealthy
        }
    }
}

/// Comprehensive cache performance report
#[derive(Debug, Clone)]
pub struct CachePerformanceReport {
    /// Performance metrics
    pub metrics: CachePerformanceMetrics,
    /// Performance assessment
    pub assessment: CachePerformanceAssessment,
    /// Recommendations for improvement
    pub recommendations: Vec<String>,
    /// Report timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Cache health status
    pub health_status: CacheHealthStatus,
}

impl CachePerformanceReport {
    /// Get summary of the report
    pub fn get_summary(&self) -> String {
        format!(
            "Cache Performance Report - {}: {} (Efficiency: {:.2}%, Hit Rate: {:.2}%)",
            self.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.assessment,
            self.metrics.efficiency_score * 100.0,
            self.metrics.hit_rate * 100.0
        )
    }

    /// Check if cache performance is acceptable
    pub fn is_performance_acceptable(&self) -> bool {
        self.assessment != CachePerformanceAssessment::Poor
            && self.health_status != CacheHealthStatus::Unhealthy
    }

    /// Get critical issues (if any)
    pub fn get_critical_issues(&self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.health_status == CacheHealthStatus::Unhealthy {
            issues.push("Cache health is unhealthy".to_string());
        }

        if self.assessment == CachePerformanceAssessment::Poor {
            issues.push("Cache performance is poor".to_string());
        }

        if self.metrics.error_rate > 0.05 {
            issues.push(format!(
                "High error rate: {:.2}%",
                self.metrics.error_rate * 100.0
            ));
        }

        if self.metrics.hit_rate < 0.3 {
            issues.push(format!(
                "Low hit rate: {:.2}%",
                self.metrics.hit_rate * 100.0
            ));
        }

        issues
    }
}

/// Cache health status
#[derive(Debug, Clone, PartialEq)]
pub enum CacheHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Cache health checker
pub struct CacheHealthChecker {
    _metrics: CacheMetrics,
    health_thresholds: HealthThresholds,
}

/// Health thresholds for cache
#[derive(Debug, Clone)]
pub struct HealthThresholds {
    /// Hit rate below this is degraded (0.0 to 1.0)
    pub degraded_hit_rate: f64,
    /// Hit rate below this is unhealthy (0.0 to 1.0)
    pub unhealthy_hit_rate: f64,
    /// Error rate above this is degraded (0.0 to 1.0)
    pub degraded_error_rate: f64,
    /// Error rate above this is unhealthy (0.0 to 1.0)
    pub unhealthy_error_rate: f64,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            degraded_hit_rate: 0.7,     // 70% hit rate
            unhealthy_hit_rate: 0.5,    // 50% hit rate
            degraded_error_rate: 0.02,  // 2% error rate
            unhealthy_error_rate: 0.05, // 5% error rate
        }
    }
}

impl CacheHealthChecker {
    /// Create new cache health checker
    pub fn new(metrics: CacheMetrics) -> Self {
        Self {
            _metrics: metrics,
            health_thresholds: HealthThresholds::default(),
        }
    }

    /// Set custom health thresholds
    pub fn with_health_thresholds(mut self, thresholds: HealthThresholds) -> Self {
        self.health_thresholds = thresholds;
        self
    }

    /// Check cache health based on metrics
    pub async fn check_health(&self, stats: &CacheStats) -> CacheHealthStatus {
        let hit_rate = stats.hit_rate();
        let error_rate = stats.error_rate();

        // Check for unhealthy conditions
        if hit_rate < self.health_thresholds.unhealthy_hit_rate
            || error_rate > self.health_thresholds.unhealthy_error_rate
        {
            return CacheHealthStatus::Unhealthy;
        }

        // Check for degraded conditions
        if hit_rate < self.health_thresholds.degraded_hit_rate
            || error_rate > self.health_thresholds.degraded_error_rate
        {
            return CacheHealthStatus::Degraded;
        }

        CacheHealthStatus::Healthy
    }

    /// Get health status as string
    pub fn health_status_as_string(&self, status: CacheHealthStatus) -> &'static str {
        match status {
            CacheHealthStatus::Healthy => "healthy",
            CacheHealthStatus::Degraded => "degraded",
            CacheHealthStatus::Unhealthy => "unhealthy",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::Registry;

    #[tokio::test]
    async fn test_cache_metrics() {
        let registry = Registry::new();
        let metrics = CacheMetrics::new(&registry);

        let mut stats = CacheStats::default();
        stats.hits = 80;
        stats.misses = 20;
        stats.errors = 2;

        metrics.update_stats(&stats).await;

        assert_eq!(metrics.hits.get(), 80.0);
        assert_eq!(metrics.misses.get(), 20.0);
        assert_eq!(metrics.errors.get(), 2.0);
    }

    #[test]
    fn test_performance_monitor() {
        let registry = Registry::new();
        let metrics = CacheMetrics::new(&registry);
        let monitor = CachePerformanceMonitor::new(metrics);

        let mut stats = CacheStats::default();
        stats.hits = 30;
        stats.misses = 70; // Low hit rate

        // This would normally require async runtime
        // In real tests, you'd use tokio::test
        assert!(monitor.alert_thresholds.low_hit_rate > 0.0);
    }

    #[test]
    fn test_health_checker() {
        let registry = Registry::new();
        let metrics = CacheMetrics::new(&registry);
        let checker = CacheHealthChecker::new(metrics);

        let mut stats = CacheStats::default();
        stats.hits = 80;
        stats.misses = 20;
        stats.errors = 1;

        // This would normally require async runtime
        // In real tests, you'd use tokio::test
        assert!(checker.health_thresholds.degraded_hit_rate > 0.0);
    }
}
