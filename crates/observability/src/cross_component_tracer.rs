use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use scheduler_domain::entities::Message;

pub struct CrossComponentTracer;

impl CrossComponentTracer {
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
    
    pub fn create_child_span_from_context(
        context: opentelemetry::Context,
        span_name: &str,
        attributes: Vec<(String, String)>,
    ) -> tracing::Span {
        let span = tracing::info_span!(target: "scheduler", "span", name = span_name);
        span.set_parent(context);
        for (key, value) in attributes {
            span.set_attribute(key, value);
        }

        span
    }
    
    /// Extract tracing context from message and create a child span
    pub fn create_span_from_message(message: &Message, operation: &str) -> tracing::Span {
        let span_name = format!("message_queue.{}", operation);
        
        // Add message-specific attributes
        let mut attributes = vec![
            ("message.id".to_string(), message.id.clone()),
            ("message.type".to_string(), format!("{:?}", message.message_type)),
            ("message.retry_count".to_string(), message.retry_count.to_string()),
        ];
        
        if let Some(correlation_id) = &message.correlation_id {
            attributes.push(("message.correlation_id".to_string(), correlation_id.clone()));
        }
        
        // Extract trace context from message headers
        if let Some(headers) = message.get_trace_headers() {
            if let Some(context) = Self::extract_trace_context_from_headers(headers) {
                return Self::create_child_span_from_context(context, &span_name, attributes);
            }
        }
        
        // Create root span if no context found
        let span = tracing::info_span!(target: "scheduler", "message_queue", operation = operation);
        for (key, value) in attributes {
            span.set_attribute(key, value);
        }
        span
    }
    
    /// Create a message with tracing context injected
    pub fn inject_trace_context_into_message(mut message: Message) -> Message {
        let mut headers = std::collections::HashMap::new();
        Self::inject_trace_context_into_headers(&mut headers);
        
        if !headers.is_empty() {
            message = message.with_trace_headers(headers);
        }
        
        message
    }
    
    /// Create instrumented span for service operations
    pub fn instrument_service_call(
        service_name: &str,
        operation: &str,
        attributes: Vec<(String, String)>,
    ) -> tracing::Span {
        let span = tracing::info_span!(target: "scheduler", "service", service = service_name, operation = operation);
        
        for (key, value) in attributes {
            span.set_attribute(key, value);
        }
        
        span
    }
}
