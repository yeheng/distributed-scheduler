/// Core metrics types and definitions
pub mod types;
/// Metrics collector implementations
pub mod collector;
/// Metrics registry and management
pub mod registry;
/// Metrics exporter functionality
pub mod exporter;
/// Timer and histogram utilities
pub mod timer;
/// Metrics aggregation functions
pub mod aggregator;

// Re-export key types for convenience
pub use types::{MetricType, MetricValue, Metric, MetricSnapshot};
pub use collector::{MetricsCollector, Counter, Gauge, Histogram, Timer};
pub use registry::{MetricsRegistry, RegistryConfig};
pub use exporter::{MetricsExporter, ExportFormat, PrometheusExporter};
pub use timer::{TimerBuilder, Stopwatch};
pub use aggregator::{MetricsAggregator, AggregationFunction};