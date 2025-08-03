//! Cross-component tracing utilities
//!
//! This module provides utilities for distributed tracing across different
//! components of the distributed scheduler system, including context propagation
//! and span creation from remote contexts.

use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Cross-component tracing utilities
pub struct CrossComponentTracer;

impl CrossComponentTracer {
    /// Extract trace context from message headers (for message queue)
    pub fn extract_trace_context_from_headers(
        headers: &std::collections::HashMap<String, String>,
    ) -> Option<opentelemetry::Context> {
        struct HeaderExtractor<'a>(&'a std::collections::HashMap<String, String>);

        impl<'a> Extractor for HeaderExtractor<'a> {
            fn get(&self, key: &str) -> Option<&str> {
                self.0.get(key).map(|s| s.as_str())
            }

            fn keys(&self) -> Vec<&str> {
                self.0.keys().map(|s| s.as_str()).collect()
            }
        }

        let extractor = HeaderExtractor(headers);
        let context = global::get_text_map_propagator(|propagator| propagator.extract(&extractor));

        Some(context)
    }

    /// Inject trace context into message headers (for message queue)
    pub fn inject_trace_context_into_headers(
        headers: &mut std::collections::HashMap<String, String>,
    ) {
        struct HeaderInjector<'a>(&'a mut std::collections::HashMap<String, String>);

        impl<'a> Injector for HeaderInjector<'a> {
            fn set(&mut self, key: &str, value: String) {
                self.0.insert(key.to_string(), value);
            }
        }

        let mut injector = HeaderInjector(headers);
        let context = tracing::Span::current().context();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&context, &mut injector)
        });
    }

    /// Create a child span from remote context
    pub fn create_child_span_from_context(
        context: opentelemetry::Context,
        span_name: &str,
        attributes: Vec<(String, String)>,
    ) -> tracing::Span {
        let span = tracing::info_span!(target: "scheduler", "{}", span_name);

        // Set the remote context as parent
        span.set_parent(context);

        // Add attributes
        for (key, value) in attributes {
            span.set_attribute(key, value);
        }

        span
    }
}
