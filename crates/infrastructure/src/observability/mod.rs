//! Observability module
//!
//! This module provides comprehensive observability features including:
//! - Metrics collection and reporting
//! - Structured logging
//! - Distributed tracing
//! - Telemetry setup and configuration

pub mod cross_component_tracer;
pub mod metrics_collector;
pub mod structured_logger;
pub mod task_tracer;
pub mod telemetry_setup;

// Re-export main types for convenience
pub use cross_component_tracer::CrossComponentTracer;
pub use metrics_collector::MetricsCollector;
pub use structured_logger::{LoggingConfig, StructuredLogger};
pub use task_tracer::TaskTracer;
pub use telemetry_setup::{
    init_logging_and_tracing, init_metrics, init_observability, init_structured_logging,
    init_tracing, shutdown_observability,
};
