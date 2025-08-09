use super::test_utils::TestApp;
use serde_json::{json, Value};

#[tokio::test]
async fn test_create_task_success() {
    tracing_subscriber::fmt::init();
    let app = TestApp::spawn().await;

    let task_data = json!({
        "name": "test-task",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo hello"},
        "timeout_seconds": 300,
        "max_retries": 3
    });
    let client = reqwest::Client::new();
    let health_response = client
        .get(&format!("{}/health", app.address))
        .send()
        .await
        .expect("Failed to execute health check");

    eprintln!("Health check status: {}", health_response.status());

    let response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    let status = response.status();
    if status != 201 {
        let error_body = response
            .text()
            .await
            .expect("Failed to read error response");
        eprintln!(
            "Request data: {}",
            serde_json::to_string_pretty(&task_data).unwrap()
        );
        eprintln!("Response status: {}", status);
        eprintln!("Response body: {}", error_body);
        panic!("Expected status 201, got {}. Error: {}", status, error_body);
    }

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let task = &response_body["data"];
    assert_eq!(task["name"], "test-task");
    assert_eq!(task["task_type"], "shell");
    assert_eq!(task["schedule"], "0 0 0 * * *");
    assert_eq!(task["timeout_seconds"], 300);
    assert_eq!(task["max_retries"], 3);
    assert_eq!(task["status"], "ACTIVE");

    app.cleanup().await;
}

#[tokio::test]
async fn test_create_task_invalid_cron() {
    let app = TestApp::spawn().await;

    let task_data = json!({
        "name": "test-task",
        "task_type": "shell",
        "schedule": "invalid-cron",
        "parameters": {"command": "echo hello"}
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 400);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("无效的CRON表达式"));

    app.cleanup().await;
}

#[tokio::test]
async fn test_create_task_duplicate_name() {
    let app = TestApp::spawn().await;

    let task_data = json!({
        "name": "duplicate-task",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo hello"}
    });

    let client = reqwest::Client::new();
    let response1 = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");
    assert_eq!(response1.status(), 201);
    let response2 = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");
    assert_eq!(response2.status(), 409);

    let response_body: Value = response2.json().await.expect("Failed to parse response");
    assert!(response_body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("已存在"));

    app.cleanup().await;
}

#[tokio::test]
async fn test_list_tasks() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();
    for i in 1..=3 {
        let task_data = json!({
            "name": format!("task-{}", i),
            "task_type": "shell",
            "schedule": "0 0 0 * * *",
            "parameters": {"command": format!("echo {}", i)}
        });

        let response = client
            .post(&format!("{}/api/tasks", app.address))
            .json(&task_data)
            .send()
            .await
            .expect("Failed to execute request");
        assert_eq!(response.status(), 201);
    }
    let response = client
        .get(&format!("{}/api/tasks", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let data = &response_body["data"];
    assert_eq!(data["total"], 3);
    assert_eq!(data["items"].as_array().unwrap().len(), 3);

    app.cleanup().await;
}

#[tokio::test]
async fn test_list_tasks_with_filters() {
    let app = TestApp::spawn().await;

    let client = reqwest::Client::new();
    let shell_task = json!({
        "name": "shell-task",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo shell"}
    });

    let http_task = json!({
        "name": "http-task",
        "task_type": "http",
        "schedule": "0 0 0 * * *",
        "parameters": {"url": "http://example.com"}
    });

    client
        .post(&format!("{}/api/tasks", app.address))
        .json(&shell_task)
        .send()
        .await
        .unwrap();
    client
        .post(&format!("{}/api/tasks", app.address))
        .json(&http_task)
        .send()
        .await
        .unwrap();
    let response = client
        .get(&format!("{}/api/tasks?task_type=shell", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    let data = &response_body["data"];
    assert_eq!(data["total"], 1);
    assert_eq!(data["items"][0]["task_type"], "shell");

    app.cleanup().await;
}

#[tokio::test]
async fn test_get_task() {
    let app = TestApp::spawn().await;
    let task_data = json!({
        "name": "get-test-task",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo test"}
    });

    let client = reqwest::Client::new();
    let create_response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    let create_body: Value = create_response
        .json()
        .await
        .expect("Failed to parse response");
    let task_id = create_body["data"]["id"].as_i64().unwrap();
    let response = client
        .get(&format!("{}/api/tasks/{}", app.address, task_id))
        .send()
        .await
        .expect("Failed to execute request");

    let status = response.status();
    if status != 200 {
        let error_body = response
            .text()
            .await
            .expect("Failed to read error response");
        eprintln!("Get task failed. Status: {}, Error: {}", status, error_body);
        panic!("Expected status 200, got {}", status);
    }

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let task = &response_body["data"];
    assert_eq!(task["id"], task_id);
    assert_eq!(task["name"], "get-test-task");
    assert!(task["recent_runs"].is_array());
    assert!(task["execution_stats"].is_object());

    app.cleanup().await;
}

#[tokio::test]
async fn test_get_task_not_found() {
    let app = TestApp::spawn().await;

    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/api/tasks/999", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 404);

    app.cleanup().await;
}

#[tokio::test]
async fn test_update_task() {
    let app = TestApp::spawn().await;
    let task_data = json!({
        "name": "update-test-task",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo test"}
    });

    let client = reqwest::Client::new();
    let create_response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    let create_body: Value = create_response
        .json()
        .await
        .expect("Failed to parse response");
    let task_id = create_body["data"]["id"].as_i64().unwrap();
    let update_data = json!({
        "name": "updated-task-name",
        "schedule": "0 0 1 * * *",
        "timeout_seconds": 600
    });

    let response = client
        .put(&format!("{}/api/tasks/{}", app.address, task_id))
        .json(&update_data)
        .send()
        .await
        .expect("Failed to execute request");

    let status = response.status();
    if status != 200 {
        let error_body = response
            .text()
            .await
            .expect("Failed to read error response");
        eprintln!(
            "Update task failed. Status: {}, Error: {}",
            status, error_body
        );
        panic!("Expected status 200, got {}", status);
    }

    let response_body: Value = response.json().await.expect("Failed to parse response");
    let task = &response_body["data"];
    assert_eq!(task["name"], "updated-task-name");
    assert_eq!(task["schedule"], "0 0 1 * * *");
    assert_eq!(task["timeout_seconds"], 600);

    app.cleanup().await;
}

#[tokio::test]
async fn test_delete_task() {
    let app = TestApp::spawn().await;
    let task_data = json!({
        "name": "delete-test-task",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo test"}
    });

    let client = reqwest::Client::new();
    let create_response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    let create_body: Value = create_response
        .json()
        .await
        .expect("Failed to parse response");
    let task_id = create_body["data"]["id"].as_i64().unwrap();
    let response = client
        .delete(&format!("{}/api/tasks/{}", app.address, task_id))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["data"]["message"]
        .as_str()
        .unwrap()
        .contains("删除"));
    let get_response = client
        .get(&format!("{}/api/tasks/{}", app.address, task_id))
        .send()
        .await
        .expect("Failed to execute request");

    let get_body: Value = get_response.json().await.expect("Failed to parse response");
    assert_eq!(get_body["data"]["status"], "INACTIVE");

    app.cleanup().await;
}

#[tokio::test]
async fn test_trigger_task() {
    let app = TestApp::spawn().await;
    let task_data = json!({
        "name": "trigger-test-task",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo test"}
    });

    let client = reqwest::Client::new();
    let create_response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    let create_body: Value = create_response
        .json()
        .await
        .expect("Failed to parse response");
    let task_id = create_body["data"]["id"].as_i64().unwrap();
    let response = client
        .post(&format!("{}/api/tasks/{}/trigger", app.address, task_id))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let task_run = &response_body["data"];
    assert_eq!(task_run["task_id"], task_id);
    assert!(task_run["id"].is_number());

    app.cleanup().await;
}

#[tokio::test]
async fn test_get_task_runs() {
    let app = TestApp::spawn().await;
    let task_data = json!({
        "name": "runs-test-task",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo test"}
    });

    let client = reqwest::Client::new();
    let create_response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    let create_body: Value = create_response
        .json()
        .await
        .expect("Failed to parse response");
    let task_id = create_body["data"]["id"].as_i64().unwrap();
    let response = client
        .get(&format!("{}/api/tasks/{}/runs", app.address, task_id))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let data = &response_body["data"];
    assert_eq!(data["total"], 0);
    assert_eq!(data["items"].as_array().unwrap().len(), 0);

    app.cleanup().await;
}

#[tokio::test]
async fn test_get_task_run() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/api/task-runs/999", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 404);

    app.cleanup().await;
}

#[tokio::test]
async fn test_pagination() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();
    for i in 1..=25 {
        let task_data = json!({
            "name": format!("pagination-task-{:02}", i),
            "task_type": "shell",
            "schedule": "0 0 0 * * *",
            "parameters": {"command": format!("echo {}", i)}
        });

        client
            .post(&format!("{}/api/tasks", app.address))
            .json(&task_data)
            .send()
            .await
            .unwrap();
    }
    let response = client
        .get(&format!("{}/api/tasks?page=1&page_size=10", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    let data = &response_body["data"];
    assert_eq!(data["total"], 25);
    assert_eq!(data["page"], 1);
    assert_eq!(data["page_size"], 10);
    assert_eq!(data["total_pages"], 3);
    assert_eq!(data["items"].as_array().unwrap().len(), 10);
    let response = client
        .get(&format!("{}/api/tasks?page=2&page_size=10", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    let response_body: Value = response.json().await.expect("Failed to parse response");
    let data = &response_body["data"];
    assert_eq!(data["page"], 2);
    assert_eq!(data["items"].as_array().unwrap().len(), 10);

    app.cleanup().await;
}
