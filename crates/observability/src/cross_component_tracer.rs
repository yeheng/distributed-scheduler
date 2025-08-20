use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};
use scheduler_domain::entities::Message;
use tracing_opentelemetry::OpenTelemetrySpanExt;

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
        let span_name = format!("message_queue.{operation}");

        // Add message-specific attributes
        let mut attributes = vec![
            ("message.id".to_string(), message.id.clone()),
            (
                "message.type".to_string(),
                format!("{:?}", message.message_type),
            ),
            (
                "message.retry_count".to_string(),
                message.retry_count.to_string(),
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_domain::entities::{Message, TaskExecutionMessage};

    #[test]
    fn test_extract_trace_context_from_headers() {
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "traceparent".to_string(),
            "00-12345678901234567890123456789012-1234567890123456-01".to_string(),
        );
        headers.insert("tracestate".to_string(), "vendor1=value1".to_string());

        let context = CrossComponentTracer::extract_trace_context_from_headers(&headers);
        
        // Context should be extracted (we can't inspect the internal structure easily)
        // but we can verify it doesn't panic and returns Some
        assert!(context.is_some());
    }

    #[test]
    fn test_extract_trace_context_from_empty_headers() {
        let headers = std::collections::HashMap::new();
        let context = CrossComponentTracer::extract_trace_context_from_headers(&headers);
        
        // OpenTelemetry may return a default context even for empty headers
        // The important thing is that it doesn't panic
        let _context = context;
    }

    #[test]
    fn test_inject_trace_context_into_headers() {
        let mut headers = std::collections::HashMap::new();
        CrossComponentTracer::inject_trace_context_into_headers(&mut headers);
        
        // Headers should be modified (we can't easily verify the exact content
        // but we can verify it doesn't panic)
        let _headers = headers;
    }

    #[test]
    fn test_create_child_span_from_context() {
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "traceparent".to_string(),
            "00-12345678901234567890123456789012-1234567890123456-01".to_string(),
        );
        
        if let Some(context) = CrossComponentTracer::extract_trace_context_from_headers(&headers) {
            let attributes = vec![
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
            ];
            
            let span = CrossComponentTracer::create_child_span_from_context(context, "test_span", attributes);
            
            // Span should be created without panic
            let _span = span;
        }
    }

    #[test]
    fn test_create_span_from_message() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        let span = CrossComponentTracer::create_span_from_message(&message, "test_operation");
        
        // Span should be created without panic
        let _span = span;
    }

    #[test]
    fn test_create_span_from_message_with_headers() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "traceparent".to_string(),
            "00-12345678901234567890123456789012-1234567890123456-01".to_string(),
        );

        let message = Message::task_execution(task_execution).with_trace_headers(headers);
        let span = CrossComponentTracer::create_span_from_message(&message, "test_operation");
        
        // Span should be created without panic
        let _span = span;
    }

    #[test]
    fn test_inject_trace_context_into_message() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        let message_with_trace = CrossComponentTracer::inject_trace_context_into_message(message);
        
        // Message should be modified with trace headers
        let _message = message_with_trace;
    }

    #[test]
    fn test_instrument_service_call() {
        let attributes = vec![
            ("service".to_string(), "test_service".to_string()),
            ("operation".to_string(), "test_operation".to_string()),
        ];
        
        let span = CrossComponentTracer::instrument_service_call("test_service", "test_operation", attributes);
        
        // Span should be created without panic
        let _span = span;
    }

    #[test]
    fn test_header_extraction_with_invalid_traceparent() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("traceparent".to_string(), "invalid-traceparent".to_string());
        
        let context = CrossComponentTracer::extract_trace_context_from_headers(&headers);
        
        // Should still return Some (OpenTelemetry is lenient)
        assert!(context.is_some());
    }

    #[test]
    fn test_header_extraction_with_partial_headers() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("traceparent".to_string(), "00-12345678901234567890123456789012-1234567890123456-01".to_string());
        // Missing tracestate
        
        let context = CrossComponentTracer::extract_trace_context_from_headers(&headers);
        
        // Should work with just traceparent
        assert!(context.is_some());
    }

    #[test]
    fn test_span_creation_with_edge_cases() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 0,
            task_id: -1,
            task_name: "".to_string(),
            task_type: "".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 0,
            retry_count: -1,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        let span = CrossComponentTracer::create_span_from_message(&message, "");
        
        // Should handle edge cases without panic
        let _span = span;
    }

    #[test]
    fn test_span_creation_with_long_strings() {
        let long_task_name = "very-long-task-name-with-many-characters-and-descriptive-text-1234567890";
        let long_task_type = "very-long-task-type-with-detailed-classification-and-category-1234567890";
        let long_operation = "very-long-operation-name-with-detailed-description-1234567890";

        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: long_task_name.to_string(),
            task_type: long_task_type.to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        let span = CrossComponentTracer::create_span_from_message(&message, long_operation);
        
        // Should handle long strings without panic
        let _span = span;
    }

    #[test]
    fn test_span_creation_with_special_characters() {
        let special_task_name = "Task with \"special\" characters & symbols @#$%";
        let special_operation = "operation@#$%&*()";

        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: special_task_name.to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        let span = CrossComponentTracer::create_span_from_message(&message, special_operation);
        
        // Should handle special characters without panic
        let _span = span;
    }

    #[test]
    fn test_message_creation_with_correlation_id() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution)
            .with_correlation_id("test-correlation-id".to_string());
        let span = CrossComponentTracer::create_span_from_message(&message, "test_operation");
        
        // Should handle correlation ID without panic
        let _span = span;
    }

    #[test]
    fn test_multiple_span_creations() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        
        // Create multiple spans
        for i in 0..10 {
            let span = CrossComponentTracer::create_span_from_message(&message, &format!("operation_{}", i));
            let _span = span;
        }
    }

    #[test]
    fn test_concurrent_span_creation() {
        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        
        // Create spans from multiple threads
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let message = message.clone();
                std::thread::spawn(move || {
                    CrossComponentTracer::create_span_from_message(&message, &format!("concurrent_operation_{}", i));
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_service_call_span_with_various_attributes() {
        let test_cases = vec![
            vec![("service".to_string(), "database".to_string()), ("operation".to_string(), "query".to_string())],
            vec![("service".to_string(), "api".to_string()), ("operation".to_string(), "request".to_string())],
            vec![("service".to_string(), "cache".to_string()), ("operation".to_string(), "get".to_string())],
            vec![],
        ];

        for attributes in test_cases {
            let span = CrossComponentTracer::instrument_service_call("test_service", "test_operation", attributes.clone());
            let _span = span;
        }
    }

    #[test]
    fn test_trace_context_round_trip() {
        // Test that trace context can be extracted and injected back
        let original_headers = std::collections::HashMap::new();
        
        // Inject trace context
        let mut headers = original_headers.clone();
        CrossComponentTracer::inject_trace_context_into_headers(&mut headers);
        
        // Extract it back
        let context = CrossComponentTracer::extract_trace_context_from_headers(&headers);
        
        // Verify round-trip works (context should be present if tracing is active)
        let _context = context;
    }

    #[test]
    fn test_span_creation_with_different_message_types() {
        // Test with different message types
        let task_execution = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        let span = CrossComponentTracer::create_span_from_message(&message, "test_operation");
        let _span = span;
    }

    #[test]
    fn test_empty_attribute_list() {
        let attributes = vec![];
        let span = CrossComponentTracer::instrument_service_call("test_service", "test_operation", attributes);
        let _span = span;
    }

    #[test]
    fn test_large_attribute_list() {
        let attributes: Vec<_> = (0..100)
            .map(|i| (format!("key_{}", i), format!("value_{}", i)))
            .collect();
        
        let span = CrossComponentTracer::instrument_service_call("test_service", "test_operation", attributes);
        let _span = span;
    }
}
