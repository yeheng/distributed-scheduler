use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::errors::Result;

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

    /// Create new histogram metric
    pub fn histogram(name: String, sum: f64, count: u64, buckets: Vec<(f64, u64)>) -> Self {
        Self {
            name,
            metric_type: MetricType::Histogram,
            value: MetricValue::Histogram { sum, count, buckets },
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
            value: MetricValue::Timer { count, sum, min, max, avg },
            labels: HashMap::new(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            description: None,
        }
    }

    /// Add label to metric
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    /// Add labels to metric
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels.extend(labels);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| crate::errors::SchedulerError::Internal(format!("Failed to serialize metric: {}", e)))
    }

    /// Convert to Prometheus format
    pub fn to_prometheus(&self) -> String {
        let labels_str = if self.labels.is_empty() {
            String::new()
        } else {
            let labels: Vec<String> = self.labels
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
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
                format!("{}{} {}\n", self.name, labels_str, if *value { 1 } else { 0 })
            }
            MetricValue::String(value) => {
                format!("{}{} \"{}\"\n", self.name, labels_str, value)
            }
            MetricValue::Histogram { sum, count, buckets: _ } => {
                let mut result = format!(
                    "{}_bucket{} le={} {}\n",
                    self.name,
                    labels_str,
                    f64::INFINITY,
                    *count
                );
                result.push_str(&format!(
                    "{}_count{} {}\n",
                    self.name,
                    labels_str,
                    count
                ));
                result.push_str(&format!(
                    "{}_sum{} {}\n",
                    self.name,
                    labels_str,
                    sum
                ));
                result
            }
            MetricValue::Timer { count, sum, min, max, avg } => {
                let mut result = format!(
                    "{}_bucket{} le={} {}\n",
                    self.name,
                    labels_str,
                    f64::INFINITY,
                    *count
                );
                result.push_str(&format!(
                    "{}_count{} {}\n",
                    self.name,
                    labels_str,
                    count
                ));
                result.push_str(&format!(
                    "{}_sum{} {}\n",
                    self.name,
                    labels_str,
                    sum
                ));
                result.push_str(&format!(
                    "{}_max{} {}\n",
                    self.name,
                    labels_str,
                    max
                ));
                result.push_str(&format!(
                    "{}_min{} {}\n",
                    self.name,
                    labels_str,
                    min
                ));
                result.push_str(&format!(
                    "{}_avg{} {}\n",
                    self.name,
                    labels_str,
                    avg
                ));
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
    labels: HashMap<String, String>,
    description: Option<String>,
}

impl Counter {
    /// Create new counter
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: Arc::new(AtomicU64::new(0)),
            labels: HashMap::new(),
            description: None,
        }
    }

    /// Add label
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
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
            .with_labels(self.labels.clone())
            .with_description(self.description.clone().unwrap_or_default())
    }
}

/// Gauge metric - Value that can go up and down
#[derive(Clone, Debug)]
pub struct Gauge {
    name: String,
    value: Arc<AtomicI64>,
    labels: HashMap<String, String>,
    description: Option<String>,
}

impl Gauge {
    /// Create new gauge
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: Arc::new(AtomicI64::new(0)),
            labels: HashMap::new(),
            description: None,
        }
    }

    /// Add label
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
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

    /// Add value to gauge
    pub fn add(&self, value: i64) {
        self.value.fetch_add(value, Ordering::Relaxed);
    }

    /// Subtract value from gauge
    pub fn subtract(&self, value: i64) {
        self.value.fetch_sub(value, Ordering::Relaxed);
    }

    /// Get current value
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Convert to metric
    pub fn to_metric(&self) -> Metric {
        Metric::gauge(self.name.clone(), self.get() as f64)
            .with_labels(self.labels.clone())
            .with_description(self.description.clone().unwrap_or_default())
    }
}

/// Histogram metric - Distribution of values
#[derive(Clone, Debug)]
pub struct Histogram {
    name: String,
    sum: Arc<AtomicU64>,
    count: Arc<AtomicU64>,
    buckets: Vec<(f64, Arc<AtomicU64>)>,
    labels: HashMap<String, String>,
    description: Option<String>,
}

impl Histogram {
    /// Create new histogram with default buckets
    pub fn new(name: String) -> Self {
        let default_buckets = vec![
            0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
        ];
        Self::with_buckets(name, default_buckets)
    }

    /// Create new histogram with custom buckets
    pub fn with_buckets(name: String, buckets: Vec<f64>) -> Self {
        let bucket_atoms: Vec<(f64, Arc<AtomicU64>)> = buckets
            .iter()
            .map(|&b| (b, Arc::new(AtomicU64::new(0))))
            .collect();

        Self {
            name,
            sum: Arc::new(AtomicU64::new(0)),
            count: Arc::new(AtomicU64::new(0)),
            buckets: bucket_atoms,
            labels: HashMap::new(),
            description: None,
        }
    }

    /// Add label
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Record value
    pub fn record(&self, value: f64) {
        self.sum.fetch_add((value * 1000.0) as u64, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        // Update buckets
        for (bucket_bound, bucket_count) in &self.buckets {
            if value <= *bucket_bound {
                bucket_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> (f64, u64, Vec<(f64, u64)>) {
        let sum = self.sum.load(Ordering::Relaxed) as f64 / 1000.0;
        let count = self.count.load(Ordering::Relaxed);
        let buckets: Vec<(f64, u64)> = self.buckets
            .iter()
            .map(|(bound, count)| (*bound, count.load(Ordering::Relaxed)))
            .collect();
        (sum, count, buckets)
    }

    /// Convert to metric
    pub fn to_metric(&self) -> Metric {
        let (sum, count, buckets) = self.get_stats();
        Metric::histogram(self.name.clone(), sum, count, buckets)
            .with_labels(self.labels.clone())
            .with_description(self.description.clone().unwrap_or_default())
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
    labels: HashMap<String, String>,
    description: Option<String>,
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
            labels: HashMap::new(),
            description: None,
        }
    }

    /// Add label
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
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
            .with_labels(self.labels.clone())
            .with_description(self.description.clone().unwrap_or_default())
    }

    /// Time operation
    pub fn time<F, T>(&self, operation: F) -> T
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

/// Metrics collector - Manages all metrics
#[derive(Debug)]
pub struct MetricsCollector {
    /// Registered metrics
    metrics: Arc<RwLock<HashMap<String, Metric>>>,
    /// Counters
    counters: Arc<RwLock<HashMap<String, Arc<Counter>>>>,
    /// Gauges
    gauges: Arc<RwLock<HashMap<String, Arc<Gauge>>>>,
    /// Histograms
    histograms: Arc<RwLock<HashMap<String, Arc<Histogram>>>>,
    /// Timers
    timers: Arc<RwLock<HashMap<String, Arc<Timer>>>>,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            timers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create counter
    pub fn counter(&self, name: String) -> Arc<Counter> {
        let counters = self.counters.read().await;
        if let Some(counter) = counters.get(&name) {
            return counter.clone();
        }
        drop(counters);
        
        let mut counters = self.counters.write().await;
        counters
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Counter::new(name)))
            .clone()
    }

    /// Create gauge
    pub fn gauge(&self, name: String) -> Arc<Gauge> {
        let gauges = self.gauges.read().await;
        if let Some(gauge) = gauges.get(&name) {
            return gauge.clone();
        }
        drop(gauges);
        
        let mut gauges = self.gauges.write().await;
        gauges
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Gauge::new(name)))
            .clone()
    }

    /// Create histogram
    pub fn histogram(&self, name: String) -> Arc<Histogram> {
        let histograms = self.histograms.read().await;
        if let Some(histogram) = histograms.get(&name) {
            return histogram.clone();
        }
        drop(histograms);
        
        let mut histograms = self.histograms.write().await;
        histograms
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Histogram::new(name)))
            .clone()
    }

    /// Create timer
    pub fn timer(&self, name: String) -> Arc<Timer> {
        let timers = self.timers.read().await;
        if let Some(timer) = timers.get(&name) {
            return timer.clone();
        }
        drop(timers);
        
        let mut timers = self.timers.write().await;
        timers
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Timer::new(name)))
            .clone()
    }

    /// Collect all metrics
    pub fn collect(&self) -> Vec<Metric> {
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

        // Collect histogram metrics
        let histograms = self.histograms.read().await;
        for histogram in histograms.values() {
            metrics.push(histogram.to_metric());
        }

        // Collect timer metrics
        let timers = self.timers.read().await;
        for timer in timers.values() {
            metrics.push(timer.to_metric());
        }

        metrics
    }

    /// Export metrics as JSON
    pub fn export_json(&self) -> Result<String> {
        let metrics = self.collect().await;
        serde_json::to_string(&metrics)
            .map_err(|e| crate::errors::SchedulerError::Internal(format!("Failed to export metrics: {}", e)))
    }

    /// Export metrics as Prometheus format
    pub fn export_prometheus(&self) -> Result<String> {
        let metrics = self.collect().await;
        let mut result = String::new();
        
        for metric in metrics {
            result.push_str(&metric.to_prometheus());
        }
        
        Ok(result)
    }

    /// Reset all metrics
    pub fn reset(&self) {
        let counters = self.counters.write().await;
        for counter in counters.values() {
            counter.value.store(0, Ordering::Relaxed);
        }

        let gauges = self.gauges.write().await;
        for gauge in gauges.values() {
            gauge.value.store(0, Ordering::Relaxed);
        }

        let histograms = self.histograms.write().await;
        for histogram in histograms.values() {
            histogram.sum.store(0, Ordering::Relaxed);
            histogram.count.store(0, Ordering::Relaxed);
            for (_, bucket_count) in &histogram.buckets {
                bucket_count.store(0, Ordering::Relaxed);
            }
        }

        let timers = self.timers.write().await;
        for timer in timers.values() {
            timer.count.store(0, Ordering::Relaxed);
            timer.sum.store(0, Ordering::Relaxed);
            timer.min.store(u64::MAX, Ordering::Relaxed);
            timer.max.store(0, Ordering::Relaxed);
        }
    }
}

/// Global metrics collector
static GLOBAL_METRICS: std::sync::OnceLock<Arc<MetricsCollector>> = std::sync::OnceLock::new();

/// Initialize global metrics collector
pub fn init_global_metrics() -> Arc<MetricsCollector> {
    let collector = Arc::new(MetricsCollector::new());
    GLOBAL_METRICS.set(collector.clone())
        .expect("Global metrics collector already initialized");
    collector
}

/// Get global metrics collector
pub fn global_metrics() -> Option<Arc<MetricsCollector>> {
    GLOBAL_METRICS.get().cloned()
}

/// Global metrics macros
#[macro_export]
macro_rules! counter {
    ($name:expr) => {
        $crate::metrics::global_metrics().map(|m| m.counter($name.to_string()))
    };
}

#[macro_export]
macro_rules! gauge {
    ($name:expr) => {
        $crate::metrics::global_metrics().map(|m| m.gauge($name.to_string()))
    };
}

#[macro_export]
macro_rules! histogram {
    ($name:expr) => {
        $crate::metrics::global_metrics().map(|m| m.histogram($name.to_string()))
    };
}

#[macro_export]
macro_rules! timer {
    ($name:expr) => {
        $crate::metrics::global_metrics().map(|m| m.timer($name.to_string()))
    };
}

#[macro_export]
macro_rules! inc_counter {
    ($name:expr) => {
        if let Some(counter) = $crate::counter!($name) {
            counter.increment();
        }
    };
}

#[macro_export]
macro_rules! inc_counter_by {
    ($name:expr, $value:expr) => {
        if let Some(counter) = $crate::counter!($name) {
            counter.increment_by($value);
        }
    };
}

#[macro_export]
macro_rules! set_gauge {
    ($name:expr, $value:expr) => {
        if let Some(gauge) = $crate::gauge!($name) {
            gauge.set($value);
        }
    };
}

#[macro_export]
macro_rules! record_histogram {
    ($name:expr, $value:expr) => {
        if let Some(histogram) = $crate::histogram!($name) {
            histogram.record($value);
        }
    };
}

#[macro_export]
macro_rules! record_timer {
    ($name:expr, $duration:expr) => {
        if let Some(timer) = $crate::timer!($name) {
            timer.record($duration);
        }
    };
}

#[macro_export]
macro_rules! time_operation {
    ($name:expr, $operation:expr) => {
        if let Some(timer) = $crate::timer!($name) {
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
        let collector = MetricsCollector::new();
        let counter = collector.counter("test_counter".to_string()).await;
        
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
        let collector = MetricsCollector::new();
        let gauge = collector.gauge("test_gauge".to_string());
        
        assert_eq!(gauge.get(), 0);
        
        gauge.set(10);
        assert_eq!(gauge.get(), 10);
        
        gauge.increment();
        assert_eq!(gauge.get(), 11);
        
        gauge.decrement();
        assert_eq!(gauge.get(), 10);
        
        gauge.add(5);
        assert_eq!(gauge.get(), 15);
        
        gauge.subtract(3);
        assert_eq!(gauge.get(), 12);
        
        let metric = gauge.to_metric();
        assert_eq!(metric.name, "test_gauge");
        assert_eq!(metric.value, MetricValue::Float(12.0));
    }

    #[tokio::test]
    async fn test_histogram() {
        let collector = MetricsCollector::new();
        let histogram = collector.histogram("test_histogram".to_string());
        
        histogram.record(0.1);
        histogram.record(0.5);
        histogram.record(1.0);
        histogram.record(2.0);
        
        let (sum, count, buckets) = histogram.get_stats();
        assert_eq!(sum, 3.6);
        assert_eq!(count, 4);
        
        // Check bucket counts
        assert!(buckets.iter().any(|(bound, count)| *bound == 0.1 && *count == 1));
        assert!(buckets.iter().any(|(bound, count)| *bound == 0.5 && *count == 1));
        assert!(buckets.iter().any(|(bound, count)| *bound == 1.0 && *count == 1));
        assert!(buckets.iter().any(|(bound, count)| *bound == 2.0 && *count == 1));
        
        let metric = histogram.to_metric();
        assert_eq!(metric.name, "test_histogram");
    }

    #[tokio::test]
    async fn test_timer() {
        let collector = MetricsCollector::new();
        let timer = collector.timer("test_timer".to_string());
        
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
        let collector = MetricsCollector::new();
        
        let counter = collector.counter("test_counter".to_string());
        let gauge = collector.gauge("test_gauge".to_string());
        let histogram = collector.histogram("test_histogram".to_string());
        let timer = collector.timer("test_timer".to_string());
        
        counter.increment();
        gauge.set(42);
        histogram.record(1.0);
        timer.record(Duration::from_millis(100));
        
        let metrics = collector.collect().await;
        assert_eq!(metrics.len(), 4);
        
        // Test JSON export
        let json = collector.export_json().await.unwrap();
        assert!(json.contains("test_counter"));
        assert!(json.contains("test_gauge"));
        assert!(json.contains("test_histogram"));
        assert!(json.contains("test_timer"));
        
        // Test Prometheus export
        let prometheus = collector.export_prometheus().await.unwrap();
        assert!(prometheus.contains("test_counter"));
        assert!(prometheus.contains("test_gauge"));
        assert!(prometheus.contains("test_histogram"));
        assert!(prometheus.contains("test_timer"));
    }

    #[tokio::test]
    async fn test_metric_labels() {
        let collector = MetricsCollector::new();
        
        let counter = collector.counter("test_counter".to_string())
            .with_label("env".to_string(), "test".to_string())
            .with_label("version".to_string(), "1.0".to_string());
        
        counter.increment();
        
        let metric = counter.to_metric();
        assert_eq!(metric.labels.get("env"), Some(&"test".to_string()));
        assert_eq!(metric.labels.get("version"), Some(&"1.0".to_string()));
    }

    #[tokio::test]
    async fn test_timer_operation() {
        let collector = MetricsCollector::new();
        let timer = collector.timer("test_timer".to_string());
        
        let result = timer.time(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            42
        }).await;
        
        assert_eq!(result, 42);
        
        let (count, sum, _, _, _) = timer.get_stats();
        assert_eq!(count, 1);
        assert!(sum >= 45.0 && sum <= 55.0); // Allow some variance
    }

    #[tokio::test]
    async fn test_global_metrics() {
        let collector = init_global_metrics();
        
        let counter = collector.counter("global_test_counter".to_string());
        counter.increment();
        
        let metrics = collector.collect().await;
        assert!(metrics.iter().any(|m| m.name == "global_test_counter"));
    }

    #[tokio::test]
    async fn test_metrics_macros() {
        let collector = init_global_metrics();
        
        inc_counter!("macro_test_counter");
        set_gauge!("macro_test_gauge", 100);
        record_histogram!("macro_test_histogram", 1.5);
        record_timer!("macro_test_timer", Duration::from_millis(200));
        
        let result = time_operation!("macro_test_operation_timer", async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            "done"
        }).await;
        
        assert_eq!(result, "done");
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        let collector = MetricsCollector::new();
        
        let counter = collector.counter("test_counter".to_string());
        let gauge = collector.gauge("test_gauge".to_string());
        
        counter.increment();
        counter.increment();
        gauge.set(100);
        
        assert_eq!(counter.get(), 2);
        assert_eq!(gauge.get(), 100);
        
        collector.reset().await;
        
        assert_eq!(counter.get(), 0);
        assert_eq!(gauge.get(), 0);
    }
}