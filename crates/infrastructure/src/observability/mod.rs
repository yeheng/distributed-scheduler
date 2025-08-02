//! Observability module
//!
//! This module provides comprehensive observability features including:
//! - Metrics collection and reporting
//! - Structured logging
//! - Distributed tracing
//! - Telemetry setup and configuration

pub mod metrics_collector;
pub mod structured_logger;
pub mod task_tracer;
pub mod cross_component_tracer;
pub mod telemetry_setup;

// Re-export main types for convenience
pub use metrics_collector::MetricsCollector;
pub use structured_logger::{StructuredLogger, LoggingConfig};
pub use task_tracer::TaskTracer;
pub use cross_component_tracer::CrossComponentTracer;
pub use telemetry_setup::{
    init_structured_logging,
    init_tracing,
    init_logging_and_tracing,
    init_metrics,
    init_observability,
    shutdown_observability,
};