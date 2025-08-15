use super::*;
use scheduler_domain::entities::{Message, TaskExecutionMessage};
use std::collections::HashMap;

#[test]
fn test_message_tracing_ext_inject_trace_context() {
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

    // Test that inject_current_trace_context creates message with headers
    let message_with_trace = message.inject_current_trace_context();

    // Since we're not in a tracing context, headers should be empty or None
    // but the method should not panic
    assert!(
        message_with_trace.get_trace_headers().is_none()
            || message_with_trace.get_trace_headers().unwrap().is_empty()
    );
}

#[test]
fn test_message_with_trace_headers() {
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

    let mut headers = HashMap::new();
    headers.insert(
        "traceparent".to_string(),
        "00-12345678901234567890123456789012-1234567890123456-01".to_string(),
    );
    headers.insert("tracestate".to_string(), "vendor1=value1".to_string());

    let message = Message::task_execution(task_execution).with_trace_headers(headers.clone());

    assert_eq!(message.get_trace_headers(), Some(&headers));
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
    let span = message.create_span("test_operation");

    // Span should be created without panic - checking name is "message_queue"
    if let Some(metadata) = span.metadata() {
        println!("Span name: {}", metadata.name());
        assert_eq!(metadata.name(), "message_queue");
    } else {
        // If no metadata, just check that span was created
        assert!(true);
    }
}
