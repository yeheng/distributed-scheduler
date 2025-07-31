use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::SchedulerResult;

/// Metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    /// Counter - Monotonically increasing value
    Counter,
    /// Gauge - Value that can go up and down
    Gauge,
    /// Histogram - Distribution of values
    Histogram,
    /// Timer - Duration measurements
    Timer,
}

/// Metric value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetricValue {
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// String value
    String(String),
    /// Histogram buckets
    Histogram {
        /// Sum of all values
        sum: f64,
        /// Count of values
        count: u64,
        /// Bucket boundaries and counts
        buckets: Vec<(f64, u64)>,
    },
    /// Timer statistics
    Timer {
        /// Count of measurements
        count: u64,
        /// Sum of all durations in milliseconds
        sum: f64,
        /// Minimum duration in milliseconds
        min: f64,
        /// Maximum duration in milliseconds
        max: f64,
        /// Average duration in milliseconds
        avg: f64,
    },
}

/// Metric data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    /// Metric name
    pub name: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Metric value
    pub value: MetricValue,
    /// Labels/dimensions
    pub labels: HashMap<String, String>,
    /// Timestamp
    pub timestamp: u64,
    /// Description
    pub description: Option<String>,
}

impl Metric {
    /// Create new counter metric
    pub fn counter(name: String, value: i64) -> Self {
        Self {
            name,
            metric_type: MetricType::Counter,
            value: MetricValue::Integer(value),
            labels: HashMap::new(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            description: None,
        }
    }

    /// Create new gauge metric
    pub fn gauge(name: String, value: f64) -> Self {
        Self {
            name,
            metric_type: MetricType::Gauge,
            value: MetricValue::Float(value),
            labels: HashMap::new(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            description: None,
        }
    }

    /// Create new timer metric
    pub fn timer(name: String, count: u64, sum: f64, min: f64, max: f64, avg: f64) -> Self {
        Self {
            name,
            metric_type: MetricType::Timer,
            value: MetricValue::Timer {
                count,
                sum,
                min,
                max,
                avg,
            },
            labels: HashMap::new(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            description: None,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> SchedulerResult<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::errors::SchedulerError::Internal(format!("Failed to serialize metric: {e}"))
        })
    }

    /// Convert to Prometheus format
    pub fn to_prometheus(&self) -> String {
        let labels_str = if self.labels.is_empty() {
            String::new()
        } else {
            let labels: Vec<String> = self
                .labels
                .iter()
                .map(|(k, v)| format!("{k}=\"{v}\""))
                .collect();
            format!("{{{}}}", labels.join(","))
        };

        match &self.value {
            MetricValue::Integer(value) => {
                format!("{}{} {}\n", self.name, labels_str, value)
            }
            MetricValue::Float(value) => {
                format!("{}{} {}\n", self.name, labels_str, value)
            }
            MetricValue::Boolean(value) => {
                format!(
                    "{}{} {}\n",
                    self.name,
                    labels_str,
                    if *value { 1 } else { 0 }
                )
            }
            MetricValue::String(value) => {
                format!("{}{} \"{}\"\n", self.name, labels_str, value)
            }
            MetricValue::Histogram {
                sum,
                count,
                buckets: _,
            } => {
                let mut result = format!(
                    "{}_bucket{} le={} {}\n",
                    self.name,
                    labels_str,
                    f64::INFINITY,
                    *count
                );
                result.push_str(&format!("{}_count{} {}\n", self.name, labels_str, count));
                result.push_str(&format!("{}_sum{} {}\n", self.name, labels_str, sum));
                result
            }
            MetricValue::Timer {
                count,
                sum,
                min,
                max,
                avg,
            } => {
                let mut result = format!(
                    "{}_bucket{} le={} {}\n",
                    self.name,
                    labels_str,
                    f64::INFINITY,
                    *count
                );
                result.push_str(&format!("{}_count{} {}\n", self.name, labels_str, count));
                result.push_str(&format!("{}_sum{} {}\n", self.name, labels_str, sum));
                result.push_str(&format!("{}_max{} {}\n", self.name, labels_str, max));
                result.push_str(&format!("{}_min{} {}\n", self.name, labels_str, min));
                result.push_str(&format!("{}_avg{} {}\n", self.name, labels_str, avg));
                result
            }
        }
    }
}

/// Counter metric - Monotonically increasing
#[derive(Clone, Debug)]
pub struct Counter {
    name: String,
    value: Arc<AtomicU64>,
    _labels: HashMap<String, String>,
    _description: Option<String>,
}

impl Counter {
    /// Create new counter
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: Arc::new(AtomicU64::new(0)),
            _labels: HashMap::new(),
            _description: None,
        }
    }

    /// Increment counter
    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment counter by value
    pub fn increment_by(&self, value: u64) {
        self.value.fetch_add(value, Ordering::Relaxed);
    }

    /// Get current value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Convert to metric
    pub fn to_metric(&self) -> Metric {
        Metric::counter(self.name.clone(), self.get() as i64)
    }
}

/// Gauge metric - Value that can go up and down
#[derive(Clone, Debug)]
pub struct Gauge {
    name: String,
    value: Arc<AtomicI64>,
    _labels: HashMap<String, String>,
    _description: Option<String>,
}

impl Gauge {
    /// Create new gauge
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: Arc::new(AtomicI64::new(0)),
            _labels: HashMap::new(),
            _description: None,
        }
    }

    /// Set gauge value
    pub fn set(&self, value: i64) {
        self.value.store(value, Ordering::Relaxed);
    }

    /// Increment gauge
    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement gauge
    pub fn decrement(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current value
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Convert to metric
    pub fn to_metric(&self) -> Metric {
        Metric::gauge(self.name.clone(), self.get() as f64)
    }
}

/// Timer metric - Duration measurements
#[derive(Clone, Debug)]
pub struct Timer {
    name: String,
    count: Arc<AtomicU64>,
    sum: Arc<AtomicU64>,
    min: Arc<AtomicU64>,
    max: Arc<AtomicU64>,
    _labels: HashMap<String, String>,
    _description: Option<String>,
}

impl Timer {
    /// Create new timer
    pub fn new(name: String) -> Self {
        Self {
            name,
            count: Arc::new(AtomicU64::new(0)),
            sum: Arc::new(AtomicU64::new(0)),
            min: Arc::new(AtomicU64::new(u64::MAX)),
            max: Arc::new(AtomicU64::new(0)),
            _labels: HashMap::new(),
            _description: None,
        }
    }

    /// Record duration
    pub fn record(&self, duration: Duration) {
        let millis = duration.as_millis() as u64;
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(millis, Ordering::Relaxed);
        self.min.fetch_min(millis, Ordering::Relaxed);
        self.max.fetch_max(millis, Ordering::Relaxed);
    }

    /// Get statistics
    pub fn get_stats(&self) -> (u64, f64, f64, f64, f64) {
        let count = self.count.load(Ordering::Relaxed);
        let sum = self.sum.load(Ordering::Relaxed) as f64;
        let min = self.min.load(Ordering::Relaxed) as f64;
        let max = self.max.load(Ordering::Relaxed) as f64;
        let avg = if count > 0 { sum / count as f64 } else { 0.0 };
        (count, sum, min, max, avg)
    }

    /// Convert to metric
    pub fn to_metric(&self) -> Metric {
        let (count, sum, min, max, avg) = self.get_stats();
        Metric::timer(self.name.clone(), count, sum, min, max, avg)
    }

    /// Time operation
    pub async fn time<F, T>(&self, operation: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = operation.await;
        let duration = start.elapsed();
        self.record(duration);
        result
    }
}

/// Simple metrics collector - Thread-safe and easy to use
#[derive(Debug)]
pub struct SimpleMetricsCollector {
    counters: Arc<RwLock<HashMap<String, Arc<Counter>>>>,
    gauges: Arc<RwLock<HashMap<String, Arc<Gauge>>>>,
    timers: Arc<RwLock<HashMap<String, Arc<Timer>>>>,
}

impl Default for SimpleMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleMetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            timers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create or get counter
    pub async fn counter(&self, name: &str) -> Arc<Counter> {
        let mut counters = self.counters.write().await;
        counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Counter::new(name.to_string())))
            .clone()
    }

    /// Create or get gauge
    pub async fn gauge(&self, name: &str) -> Arc<Gauge> {
        let mut gauges = self.gauges.write().await;
        gauges
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Gauge::new(name.to_string())))
            .clone()
    }

    /// Create or get timer
    pub async fn timer(&self, name: &str) -> Arc<Timer> {
        let mut timers = self.timers.write().await;
        timers
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Timer::new(name.to_string())))
            .clone()
    }

    /// Collect all metrics
    pub async fn collect(&self) -> Vec<Metric> {
        let mut metrics = Vec::new();

        // Collect counter metrics
        let counters = self.counters.read().await;
        for counter in counters.values() {
            metrics.push(counter.to_metric());
        }

        // Collect gauge metrics
        let gauges = self.gauges.read().await;
        for gauge in gauges.values() {
            metrics.push(gauge.to_metric());
        }

        // Collect timer metrics
        let timers = self.timers.read().await;
        for timer in timers.values() {
            metrics.push(timer.to_metric());
        }

        metrics
    }

    /// Export metrics as JSON
    pub async fn export_json(&self) -> SchedulerResult<String> {
        let metrics = self.collect().await;
        serde_json::to_string(&metrics).map_err(|e| {
            crate::errors::SchedulerError::Internal(format!("Failed to export metrics: {e}"))
        })
    }

    /// Export metrics as Prometheus format
    pub async fn export_prometheus(&self) -> SchedulerResult<String> {
        let metrics = self.collect().await;
        let mut result = String::new();

        for metric in metrics {
            result.push_str(&metric.to_prometheus());
        }

        Ok(result)
    }
}

/// Global metrics collector
static GLOBAL_METRICS: std::sync::OnceLock<Arc<SimpleMetricsCollector>> =
    std::sync::OnceLock::new();

/// Initialize global metrics collector
pub fn init_global_metrics() -> Arc<SimpleMetricsCollector> {
    let collector = Arc::new(SimpleMetricsCollector::new());
    GLOBAL_METRICS
        .set(collector.clone())
        .expect("Global metrics collector already initialized");
    collector
}

/// Get global metrics collector
pub fn global_metrics() -> Option<Arc<SimpleMetricsCollector>> {
    GLOBAL_METRICS.get().cloned()
}

/// Simple metrics macros
#[macro_export]
macro_rules! inc_counter {
    ($name:expr) => {
        if let Some(metrics) = $crate::metrics_simple::global_metrics() {
            let counter = metrics.counter($name).await;
            counter.increment();
        }
    };
}

#[macro_export]
macro_rules! set_gauge {
    ($name:expr, $value:expr) => {
        if let Some(metrics) = $crate::metrics_simple::global_metrics() {
            let gauge = metrics.gauge($name).await;
            gauge.set($value);
        }
    };
}

#[macro_export]
macro_rules! record_timer {
    ($name:expr, $duration:expr) => {
        if let Some(metrics) = $crate::metrics_simple::global_metrics() {
            let timer = metrics.timer($name).await;
            timer.record($duration);
        }
    };
}

#[macro_export]
macro_rules! time_operation {
    ($name:expr, $operation:expr) => {
        if let Some(metrics) = $crate::metrics_simple::global_metrics() {
            let timer = metrics.timer($name).await;
            timer.time($operation).await
        } else {
            $operation.await
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_counter() {
        let collector = SimpleMetricsCollector::new();
        let counter = collector.counter("test_counter").await;

        assert_eq!(counter.get(), 0);

        counter.increment();
        assert_eq!(counter.get(), 1);

        counter.increment_by(5);
        assert_eq!(counter.get(), 6);

        let metric = counter.to_metric();
        assert_eq!(metric.name, "test_counter");
        assert_eq!(metric.value, MetricValue::Integer(6));
    }

    #[tokio::test]
    async fn test_gauge() {
        let collector = SimpleMetricsCollector::new();
        let gauge = collector.gauge("test_gauge").await;

        assert_eq!(gauge.get(), 0);

        gauge.set(10);
        assert_eq!(gauge.get(), 10);

        gauge.increment();
        assert_eq!(gauge.get(), 11);

        gauge.decrement();
        assert_eq!(gauge.get(), 10);

        let metric = gauge.to_metric();
        assert_eq!(metric.name, "test_gauge");
        assert_eq!(metric.value, MetricValue::Float(10.0));
    }

    #[tokio::test]
    async fn test_timer() {
        let collector = SimpleMetricsCollector::new();
        let timer = collector.timer("test_timer").await;

        timer.record(Duration::from_millis(100));
        timer.record(Duration::from_millis(200));
        timer.record(Duration::from_millis(50));

        let (count, sum, min, max, avg) = timer.get_stats();
        assert_eq!(count, 3);
        assert_eq!(sum, 350.0);
        assert_eq!(min, 50.0);
        assert_eq!(max, 200.0);
        assert_eq!(avg, 350.0 / 3.0);

        let metric = timer.to_metric();
        assert_eq!(metric.name, "test_timer");
    }

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = SimpleMetricsCollector::new();

        let counter = collector.counter("test_counter").await;
        let gauge = collector.gauge("test_gauge").await;
        let timer = collector.timer("test_timer").await;

        counter.increment();
        gauge.set(42);
        timer.record(Duration::from_millis(100));

        let metrics = collector.collect().await;
        assert_eq!(metrics.len(), 3);

        // Test JSON export
        let json = collector.export_json().await.unwrap();
        assert!(json.contains("test_counter"));
        assert!(json.contains("test_gauge"));
        assert!(json.contains("test_timer"));

        // Test Prometheus export
        let prometheus = collector.export_prometheus().await.unwrap();
        assert!(prometheus.contains("test_counter"));
        assert!(prometheus.contains("test_gauge"));
        assert!(prometheus.contains("test_timer"));
    }

    #[tokio::test]
    async fn test_global_metrics() {
        // Note: This test may fail if run multiple times due to global state
        // In real usage, global metrics should only be initialized once
        if GLOBAL_METRICS.get().is_none() {
            let collector = init_global_metrics();

            let counter = collector.counter("global_test_counter").await;
            counter.increment();

            let metrics = collector.collect().await;
            assert!(metrics.iter().any(|m| m.name == "global_test_counter"));
        }
    }

    #[tokio::test]
    async fn test_macros() {
        // Use existing global metrics or create a local collector for testing
        if let Some(collector) = global_metrics() {
            inc_counter!("macro_test_counter");
            set_gauge!("macro_test_gauge", 100);
            record_timer!("macro_test_timer", Duration::from_millis(200));

            let result = time_operation!("macro_test_operation_timer", async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                "done".to_string()
            });

            assert_eq!(result, "done".to_string());

            let metrics = collector.collect().await;
            assert!(metrics.iter().any(|m| m.name == "macro_test_counter"));
            assert!(metrics.iter().any(|m| m.name == "macro_test_gauge"));
            assert!(metrics.iter().any(|m| m.name == "macro_test_timer"));
            assert!(metrics
                .iter()
                .any(|m| m.name == "macro_test_operation_timer"));
        } else {
            // If no global metrics, test the macros directly
            let collector = Arc::new(SimpleMetricsCollector::new());

            // Simulate macro behavior
            let counter = collector.counter("macro_test_counter").await;
            counter.increment();

            let gauge = collector.gauge("macro_test_gauge").await;
            gauge.set(100);

            let timer = collector.timer("macro_test_timer").await;
            timer.record(Duration::from_millis(200));

            let timer_op = collector.timer("macro_test_operation_timer").await;
            let result = timer_op
                .time(async {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    "done".to_string()
                })
                .await;

            assert_eq!(result, "done".to_string());

            let metrics = collector.collect().await;
            assert!(metrics.iter().any(|m| m.name == "macro_test_counter"));
            assert!(metrics.iter().any(|m| m.name == "macro_test_gauge"));
            assert!(metrics.iter().any(|m| m.name == "macro_test_timer"));
            assert!(metrics
                .iter()
                .any(|m| m.name == "macro_test_operation_timer"));
        }
    }
}
