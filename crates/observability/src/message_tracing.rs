use crate::CrossComponentTracer;
use scheduler_domain::entities::Message;

/// Extension trait to add tracing capabilities to messages
pub trait MessageTracingExt {
    /// Inject current tracing context into the message
    fn inject_current_trace_context(self) -> Self;

    /// Create an instrumented span from this message
    fn create_span(&self, operation: &str) -> tracing::Span;
}

impl MessageTracingExt for Message {
    fn inject_current_trace_context(mut self) -> Self {
        let mut headers = std::collections::HashMap::new();
        CrossComponentTracer::inject_trace_context_into_headers(&mut headers);
        if !headers.is_empty() {
            self = self.with_trace_headers(headers);
        }
        self
    }

    fn create_span(&self, operation: &str) -> tracing::Span {
        CrossComponentTracer::create_span_from_message(self, operation)
    }
}

#[cfg(test)]
mod tests;
