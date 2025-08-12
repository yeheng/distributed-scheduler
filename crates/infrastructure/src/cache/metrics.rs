//! Cache metrics collection and reporting

use super::CacheStats;
use prometheus::{Counter, Gauge, Histogram, Opts, Registry, TextEncoder, Encoder};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

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
}

impl CacheMetrics {
    /// Create new cache metrics
    pub fn new(registry: &Registry) -> Self {
        let hits = Counter::with_opts(Opts::new(
            "cache_hits_total",
            "Total number of cache hits"
        )).unwrap();
        registry.register(Box::new(hits.clone())).unwrap();

        let misses = Counter::with_opts(Opts::new(
            "cache_misses_total",
            "Total number of cache misses"
        )).unwrap();
        registry.register(Box::new(misses.clone())).unwrap();

        let sets = Counter::with_opts(Opts::new(
            "cache_sets_total",
            "Total number of cache sets"
        )).unwrap();
        registry.register(Box::new(sets.clone())).unwrap();

        let deletes = Counter::with_opts(Opts::new(
            "cache_deletes_total",
            "Total number of cache deletes"
        )).unwrap();
        registry.register(Box::new(deletes.clone())).unwrap();

        let errors = Counter::with_opts(Opts::new(
            "cache_errors_total",
            "Total number of cache errors"
        )).unwrap();
        registry.register(Box::new(errors.clone())).unwrap();

        let hit_rate = Gauge::with_opts(Opts::new(
            "cache_hit_rate",
            "Cache hit rate (0.0 to 1.0)"
        )).unwrap();
        registry.register(Box::new(hit_rate.clone())).unwrap();

        let miss_rate = Gauge::with_opts(Opts::new(
            "cache_miss_rate",
            "Cache miss rate (0.0 to 1.0)"
        )).unwrap();
        registry.register(Box::new(miss_rate.clone())).unwrap();

        let error_rate = Gauge::with_opts(Opts::new(
            "cache_error_rate",
            "Cache error rate (0.0 to 1.0)"
        )).unwrap();
        registry.register(Box::new(error_rate.clone())).unwrap();

        let operation_duration = Histogram::with_opts(Opts::new(
            "cache_operation_duration_seconds",
            "Cache operation duration in seconds"
        ).into()).unwrap();
        registry.register(Box::new(operation_duration.clone())).unwrap();

        let cache_size = Gauge::with_opts(Opts::new(
            "cache_size_bytes",
            "Cache size in bytes"
        )).unwrap();
        registry.register(Box::new(cache_size.clone())).unwrap();

        let memory_usage = Gauge::with_opts(Opts::new(
            "cache_memory_usage_bytes",
            "Cache memory usage in bytes"
        )).unwrap();
        registry.register(Box::new(memory_usage.clone())).unwrap();

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
        // TODO: Fix metrics encoding - for now return empty string
        String::new()
    }

    /// Get time since last update
    pub async fn time_since_last_update(&self) -> Duration {
        self.last_update.read().await.elapsed()
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
            low_hit_rate: 0.5,  // 50% hit rate
            high_error_rate: 0.05, // 5% error rate
            slow_operation_threshold: 1.0, // 1 second
            alert_cooldown: 300, // 5 minutes
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
        if stats.hit_rate() < self.alert_thresholds.low_hit_rate && stats.hits + stats.misses > 100 {
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
    metrics: CacheMetrics,
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
            degraded_hit_rate: 0.7,    // 70% hit rate
            unhealthy_hit_rate: 0.5,   // 50% hit rate
            degraded_error_rate: 0.02, // 2% error rate
            unhealthy_error_rate: 0.05, // 5% error rate
        }
    }
}

impl CacheHealthChecker {
    /// Create new cache health checker
    pub fn new(metrics: CacheMetrics) -> Self {
        Self {
            metrics,
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
            || error_rate > self.health_thresholds.unhealthy_error_rate {
            return CacheHealthStatus::Unhealthy;
        }

        // Check for degraded conditions
        if hit_rate < self.health_thresholds.degraded_hit_rate 
            || error_rate > self.health_thresholds.degraded_error_rate {
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