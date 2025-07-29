#[cfg(test)]
use chrono::Utc;
use scheduler_core::{TaskExecutor as _, TaskRun, TaskRunStatus};

use crate::{HttpExecutor, ShellExecutor};

fn create_test_task_run_for_type(params: serde_json::Value, task_type: &str) -> TaskRun {
    let task_info = serde_json::json!({
        "parameters": params,
        "task_type": task_type
    });

    TaskRun {
        id: 1,
        task_id: 1,
        status: TaskRunStatus::Running,
        worker_id: Some("test-worker".to_string()),
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        result: Some(task_info.to_string()),
        error_message: None,
        created_at: Utc::now(),
    }
}

fn create_test_task_run(params: serde_json::Value) -> TaskRun {
    create_test_task_run_for_type(params, "shell")
}

#[tokio::test]
async fn test_shell_executor_echo() {
    let executor = ShellExecutor::new();

    let params = serde_json::json!({
        "command": "echo",
        "args": ["Hello, World!"]
    });

    let task_run = create_test_task_run(params);
    let result = executor.execute(&task_run).await.unwrap();

    assert!(result.success);
    assert_eq!(result.output, Some("Hello, World!".to_string()));
    assert!(result.error_message.is_none());
    assert_eq!(result.exit_code, Some(0));
}

#[tokio::test]
async fn test_shell_executor_invalid_command() {
    let executor = ShellExecutor::new();

    let params = serde_json::json!({
        "command": "nonexistent_command_12345"
    });

    let task_run = create_test_task_run(params);
    let result = executor.execute(&task_run).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_shell_executor_supports_task_type() {
    let executor = ShellExecutor::new();

    assert!(executor.supports_task_type("shell"));
    assert!(!executor.supports_task_type("http"));
    assert!(!executor.supports_task_type("other"));
}

#[tokio::test]
async fn test_http_executor_supports_task_type() {
    let executor = HttpExecutor::new();

    assert!(executor.supports_task_type("http"));
    assert!(!executor.supports_task_type("shell"));
    assert!(!executor.supports_task_type("other"));
}

#[tokio::test]
async fn test_http_executor_get_request() {
    let executor = HttpExecutor::new();

    let params = serde_json::json!({
        "url": "https://httpbin.org/get",
        "method": "GET"
    });

    let task_run = create_test_task_run_for_type(params, "http");
    let result = executor.execute(&task_run).await.unwrap();

    assert!(result.success);
    assert!(result.output.is_some());
    assert!(result.error_message.is_none());
    assert_eq!(result.exit_code, Some(200));
}

#[tokio::test]
async fn test_executor_names() {
    let shell_executor = ShellExecutor::new();
    let http_executor = HttpExecutor::new();

    assert_eq!(shell_executor.name(), "shell");
    assert_eq!(http_executor.name(), "http");
}
