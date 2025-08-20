use super::test_utils::TestApp;
use serde_json::{json, Value};

#[tokio::test]
async fn test_login_with_valid_credentials() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    let login_data = json!({
        "username": "admin",
        "password": "admin123"
    });

    let response = client
        .post(&format!("{}/api/auth/login", app.address))
        .json(&login_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let data = &response_body["data"];
    assert!(data["access_token"].is_string());
    assert_eq!(data["token_type"], "Bearer");
    assert!(data["expires_in"].is_number());

    app.cleanup().await;
}

#[tokio::test]
async fn test_login_with_invalid_credentials() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    let login_data = json!({
        "username": "admin",
        "password": "wrong_password"
    });

    let response = client
        .post(&format!("{}/api/auth/login", app.address))
        .json(&login_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 401);

    app.cleanup().await;
}

#[tokio::test]
async fn test_refresh_token_invalid() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    let refresh_data = json!({
        "refresh_token": "invalid_refresh_token"
    });

    let response = client
        .post(&format!("{}/api/auth/refresh", app.address))
        .json(&refresh_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 401);

    app.cleanup().await;
}

#[tokio::test]
async fn test_validate_token_invalid() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/api/auth/validate", app.address))
        .header("Authorization", "Bearer invalid_token")
        .send()
        .await
        .expect("Failed to execute request");

    // When auth is disabled, invalid tokens are ignored and default user is used
    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let data = &response_body["data"];
    assert_eq!(data["valid"], true);
    assert_eq!(data["user_id"], "test-user");

    app.cleanup().await;
}

#[tokio::test]
async fn test_validate_token_no_auth() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/api/auth/validate", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    // When auth is disabled, this returns 200 with default user
    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let data = &response_body["data"];
    assert_eq!(data["valid"], true);
    assert_eq!(data["user_id"], "test-user");

    app.cleanup().await;
}

#[tokio::test]
async fn test_create_api_key_without_auth() {
    let app = TestApp::spawn().await;
    let client = reqwest::Client::new();

    let api_key_data = json!({
        "name": "test-api-key",
        "permissions": ["TaskRead"]
    });

    let response = client
        .post(&format!("{}/api/auth/api-keys", app.address))
        .json(&api_key_data)
        .send()
        .await
        .expect("Failed to execute request");

    // When auth is disabled, this works with default admin user
    assert_eq!(response.status(), 200);

    let response_body: Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());

    let data = &response_body["data"];
    assert!(data["api_key"].is_string());
    assert_eq!(data["name"], "test-api-key");

    app.cleanup().await;
}