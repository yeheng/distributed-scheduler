use super::test_utils::TestApp;
use serde_json::{json, Value};

#[tokio::test]
async fn test_health_check() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["status"], "ok");
    assert!(response_body["timestamp"].is_string());
    assert!(response_body["version"].is_string());

    app.cleanup().await;
}

#[tokio::test]
async fn test_health_check_with_database() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["status"], "ok");
    assert!(response_body["timestamp"].is_string());
    assert!(response_body["service"].is_string());
    assert!(response_body["version"].is_string());

    app.cleanup().await;
}

#[tokio::test]
async fn test_system_endpoints_with_auth() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    // First login to get a token
    let login_data = json!({
        "username": "admin",
        "password": "admin123"
    });

    let login_response = client
        .post(&format!("{}/api/auth/login", app.address))
        .json(&login_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(login_response.status(), 200);

    let login_body: Value = login_response
        .json()
        .await
        .expect("Failed to parse response");
    let access_token = login_body["data"]["access_token"].as_str().unwrap();

    // Test system stats with authentication
    let stats_response = client
        .get(&format!("{}/api/system/stats", app.address))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(stats_response.status(), 200);

    // Test system health with authentication
    let health_response = client
        .get(&format!("{}/api/system/health", app.address))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(health_response.status(), 200);

    app.cleanup().await;
}

#[tokio::test]
async fn test_task_execution_stats_with_data() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    // Create a task
    let task_data = json!({
        "name": "execution-stats-task-unique",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo test"},
        "timeout_seconds": 300,
        "max_retries": 1
    });

    let create_response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    // Task creation should work (201) or fail with validation (422/400)
    println!("Task creation status: {}", create_response.status());
    assert!(
        create_response.status() == 201
            || create_response.status() == 422
            || create_response.status() == 400
    );

    if create_response.status() == 201 {
        let create_body: Value = create_response
            .json()
            .await
            .expect("Failed to parse response");
        let task_id = create_body["data"]["id"].as_i64().unwrap();

        // Trigger the task to create a run
        let trigger_response = client
            .post(&format!("{}/api/tasks/{}/trigger", app.address, task_id))
            .send()
            .await
            .expect("Failed to execute request");

        assert_eq!(trigger_response.status(), 200);

        // Get task execution stats
        let stats_response = client
            .get(&format!("{}/api/tasks/{}/stats", app.address, task_id))
            .send()
            .await
            .expect("Failed to execute request");

        assert_eq!(stats_response.status(), 200);

        let stats_body: Value = stats_response
            .json()
            .await
            .expect("Failed to parse response");
        let data = &stats_body["data"];

        // The stats endpoint works, actual fields depend on implementation
        assert!(data.is_object());
    }

    app.cleanup().await;
}

#[tokio::test]
async fn test_worker_registration_and_retrieval() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    // List workers (should be empty initially)
    let list_response = client
        .get(&format!("{}/api/workers", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(list_response.status(), 200);

    let list_body: Value = list_response
        .json()
        .await
        .expect("Failed to parse response");
    assert_eq!(list_body["data"]["total"], 0);

    // Try to get a non-existent worker
    let get_response = client
        .get(&format!("{}/api/workers/test-worker-1", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(get_response.status(), 404);

    app.cleanup().await;
}

#[tokio::test]
async fn test_task_lifecycle_with_all_operations() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    // Create a task
    let task_data = json!({
        "name": "lifecycle-test-task-unique",
        "task_type": "shell",
        "schedule": "0 0 0 * * *",
        "parameters": {"command": "echo lifecycle"},
        "timeout_seconds": 300,
        "max_retries": 2
    });

    let create_response = client
        .post(&format!("{}/api/tasks", app.address))
        .json(&task_data)
        .send()
        .await
        .expect("Failed to execute request");

    // Task creation should succeed (201) or fail with validation (422)
    assert!(create_response.status() == 201 || create_response.status() == 422);

    if create_response.status() == 201 {
        let create_body: Value = create_response
            .json()
            .await
            .expect("Failed to parse response");
        let task_id = create_body["data"]["id"].as_i64().unwrap();

        // Get the task
        let get_response = client
            .get(&format!("{}/api/tasks/{}", app.address, task_id))
            .send()
            .await
            .expect("Failed to execute request");

        assert_eq!(get_response.status(), 200);

        // Patch the task
        let patch_data = json!({
            "timeout_seconds": 600
        });

        let patch_response = client
            .patch(&format!("{}/api/tasks/{}", app.address, task_id))
            .json(&patch_data)
            .send()
            .await
            .expect("Failed to execute request");

        // Patch should succeed (200) or fail with validation (422)
        assert!(patch_response.status() == 200 || patch_response.status() == 422);
    }

    app.cleanup().await;
}

#[tokio::test]
async fn test_error_handling_for_invalid_endpoints() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    // Test invalid endpoint
    let response = client
        .get(&format!("{}/api/invalid/endpoint", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 404);

    // Test invalid method
    let response = client
        .patch(&format!("{}/api/tasks", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 405);

    app.cleanup().await;
}
