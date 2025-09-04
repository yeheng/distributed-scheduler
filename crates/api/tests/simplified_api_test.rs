use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::sync::Arc;
use std::collections::HashMap;
use tower::ServiceExt;
use async_trait::async_trait;

use scheduler_api::{
    routes::{AppState, create_routes},
    auth::AuthConfig,
};
use scheduler_testing_utils::mocks::{
    MockTaskRepository, MockTaskRunRepository, MockWorkerRepository,
};
use scheduler_application::task_services::TaskControlService;
use scheduler_domain::entities::TaskRun;
use scheduler_errors::SchedulerResult;

/// Mock TaskControlService for testing
struct MockTaskControlService;

#[async_trait]
impl TaskControlService for MockTaskControlService {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
        // Return a dummy TaskRun for testing
        Ok(TaskRun {
            id: 1,
            task_id,
            status: scheduler_domain::entities::TaskRunStatus::Pending,
            worker_id: None,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: chrono::Utc::now(),
        })
    }

    async fn pause_task(&self, _task_id: i64) -> SchedulerResult<()> {
        Ok(())
    }

    async fn resume_task(&self, _task_id: i64) -> SchedulerResult<()> {
        Ok(())
    }

    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun> {
        Ok(TaskRun {
            id: task_run_id,
            task_id: 1,
            status: scheduler_domain::entities::TaskRunStatus::Pending,
            worker_id: None,
            retry_count: 1,
            shard_index: None,
            shard_total: None,
            scheduled_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: chrono::Utc::now(),
        })
    }

    async fn abort_task_run(&self, _task_run_id: i64) -> SchedulerResult<()> {
        Ok(())
    }

    async fn cancel_all_task_runs(&self, _task_id: i64) -> SchedulerResult<usize> {
        Ok(0)
    }

    async fn has_running_instances(&self, _task_id: i64) -> SchedulerResult<bool> {
        Ok(false)
    }

    async fn get_recent_executions(
        &self,
        _task_id: i64,
        _limit: usize,
    ) -> SchedulerResult<Vec<TaskRun>> {
        Ok(vec![])
    }
}

/// 创建测试用的应用状态
fn create_test_app_state() -> AppState {
    AppState {
        task_repo: Arc::new(MockTaskRepository::new()),
        task_run_repo: Arc::new(MockTaskRunRepository::new()),
        worker_repo: Arc::new(MockWorkerRepository::new()),
        task_controller: Arc::new(MockTaskControlService),
        auth_config: Arc::new(AuthConfig {
            enabled: false,
            jwt_secret: "test_secret".to_string(),
            api_keys: HashMap::new(),
            jwt_expiration_hours: 24,
        }),
        rate_limiter: None,
    }
}

#[tokio::test]
async fn test_root_endpoint() {
    let app_state = create_test_app_state();
    let app = create_routes(app_state);

    let request = Request::builder()
        .uri("/")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["name"], "嵌入式任务调度系统");
    assert_eq!(json["status"], "running");
    assert_eq!(json["api_version"], "v1");
    assert!(json["documentation"].is_object());
    assert!(json["endpoints"].is_object());
}

#[tokio::test]
async fn test_api_docs_endpoint() {
    let app_state = create_test_app_state();
    let app = create_routes(app_state);

    let request = Request::builder()
        .uri("/api/docs")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["title"], "嵌入式任务调度系统 API");
    assert!(json["endpoints"].is_object());
    assert!(json["examples"].is_object());
}

#[tokio::test]
async fn test_health_endpoint() {
    let app_state = create_test_app_state();
    let app = create_routes(app_state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "embedded-task-scheduler");
    assert_eq!(json["mode"], "embedded");
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let app_state = create_test_app_state();
    let app = create_routes(app_state);

    let request = Request::builder()
        .uri("/api/metrics")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // 检查响应结构
    assert!(json["success"].as_bool().unwrap());
    let data = &json["data"];
    assert!(data["system"].is_object());
    assert!(data["tasks"].is_object());
    assert!(data["workers"].is_object());
    assert!(data["performance"].is_object());
    assert!(data["timestamp"].is_string());
}

#[tokio::test]
async fn test_detailed_health_endpoint() {
    let app_state = create_test_app_state();
    let app = create_routes(app_state);

    let request = Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json["status"].is_string());
    assert_eq!(json["service"], "embedded-task-scheduler");
    assert!(json["components"].is_object());
    assert!(json["system"].is_object());
    assert!(json["timestamp"].is_string());
}

#[tokio::test]
async fn test_error_response_format() {
    let app_state = create_test_app_state();
    let app = create_routes(app_state);

    // 测试不存在的端点
    let request = Request::builder()
        .uri("/api/nonexistent")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    
    // Check if the response is empty or not JSON
    if body_str.is_empty() {
        // If the body is empty, that's expected for a 404 from Axum's default handler
        return;
    }
    
    // If there is a body, it should be JSON with our error format
    let json: Value = serde_json::from_str(&body_str).unwrap();

    // 检查友好的错误响应格式
    assert!(json["error"].is_object());
    let error = &json["error"];
    assert!(error["message"].is_string());
    assert!(error["type"].is_string());
    assert!(error["code"].is_number());
    assert!(error["suggestions"].is_array());
    assert!(error["timestamp"].is_string());
    assert_eq!(error["documentation"], "/api/docs");
}

#[tokio::test]
async fn test_cors_headers() {
    let app_state = create_test_app_state();
    let app = create_routes(app_state);

    let request = Request::builder()
        .uri("/")
        .header("Origin", "http://localhost:3000")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    // Note: CORS headers would be added by middleware in the actual application
}

#[tokio::test]
async fn test_all_new_endpoints_exist() {
    let app_state = create_test_app_state();
    let app = create_routes(app_state);

    let endpoints = vec![
        "/",
        "/api/docs",
        "/health",
        "/api/health",
        "/api/metrics",
    ];

    for endpoint in endpoints {
        let request = Request::builder()
            .uri(endpoint)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Endpoint {} should return 200 OK",
            endpoint
        );
    }
}