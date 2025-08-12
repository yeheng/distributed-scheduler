use axum::{
    body::Body,
    http::Request,
    http::StatusCode,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use scheduler_api::{
    auth::{AuthConfig, Permission, AuthenticatedUser, AuthType},
    create_simple_app,
};
use scheduler_domain::{TaskExecutionStats, WorkerLoadStats};

#[tokio::test]
async fn test_login_with_valid_credentials() {
    let task_repo = std::sync::Arc::new(MockTaskRepository) as std::sync::Arc<dyn scheduler_domain::repositories::TaskRepository>;
    let task_run_repo = std::sync::Arc::new(MockTaskRunRepository) as std::sync::Arc<dyn scheduler_domain::repositories::TaskRunRepository>;
    let worker_repo = std::sync::Arc::new(MockWorkerRepository) as std::sync::Arc<dyn scheduler_domain::repositories::WorkerRepository>;
    let task_controller = std::sync::Arc::new(MockTaskController) as std::sync::Arc<dyn scheduler_core::traits::TaskControlService>;

    let app = create_simple_app(task_repo, task_run_repo, worker_repo, task_controller);

    let login_request = json!({
        "username": "admin",
        "password": "admin123"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&login_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(response_json["data"]["access_token"].is_string());
    assert_eq!(response_json["data"]["token_type"], "Bearer");
    assert!(response_json["data"]["expires_in"].is_number());
}

#[tokio::test]
async fn test_login_with_invalid_credentials() {
    let task_repo = std::sync::Arc::new(MockTaskRepository) as std::sync::Arc<dyn scheduler_domain::repositories::TaskRepository>;
    let task_run_repo = std::sync::Arc::new(MockTaskRunRepository) as std::sync::Arc<dyn scheduler_domain::repositories::TaskRunRepository>;
    let worker_repo = std::sync::Arc::new(MockWorkerRepository) as std::sync::Arc<dyn scheduler_domain::repositories::WorkerRepository>;
    let task_controller = std::sync::Arc::new(MockTaskController) as std::sync::Arc<dyn scheduler_core::traits::TaskControlService>;

    let app = create_simple_app(task_repo, task_run_repo, worker_repo, task_controller);

    let login_request = json!({
        "username": "admin",
        "password": "wrong_password"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&login_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_endpoint_without_auth() {
    let task_repo = std::sync::Arc::new(MockTaskRepository) as std::sync::Arc<dyn scheduler_domain::repositories::TaskRepository>;
    let task_run_repo = std::sync::Arc::new(MockTaskRunRepository) as std::sync::Arc<dyn scheduler_domain::repositories::TaskRunRepository>;
    let worker_repo = std::sync::Arc::new(MockWorkerRepository) as std::sync::Arc<dyn scheduler_domain::repositories::WorkerRepository>;
    let task_controller = std::sync::Arc::new(MockTaskController) as std::sync::Arc<dyn scheduler_core::traits::TaskControlService>;

    // Create app with authentication enabled
    let auth_config = AuthConfig {
        enabled: true,
        jwt_secret: "test-secret-key".to_string(),
        api_keys: std::collections::HashMap::new(),
        jwt_expiration_hours: 24,
    };
    
    let state = scheduler_api::routes::AppState {
        task_repo,
        task_run_repo,
        worker_repo,
        task_controller,
        auth_config: std::sync::Arc::new(auth_config),
        rate_limiter: None,
    };
    
    let app = scheduler_api::routes::create_routes(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_jwt_token_validation() {
    let auth_config = AuthConfig {
        enabled: true,
        jwt_secret: "test-secret-key".to_string(),
        api_keys: std::collections::HashMap::new(),
        jwt_expiration_hours: 24,
    };

    let jwt_service = scheduler_api::auth::JwtService::new(&auth_config.jwt_secret, auth_config.jwt_expiration_hours);
    
    let permissions = vec![Permission::TaskRead, Permission::TaskWrite];
    let token = jwt_service.generate_token("test-user", &permissions).unwrap();
    
    let claims = jwt_service.validate_token(&token).unwrap();
    assert_eq!(claims.user_id, "test-user");
    assert_eq!(claims.permissions.len(), 2);
}

#[tokio::test]
async fn test_user_permissions() {
    let user = AuthenticatedUser {
        user_id: "test-user".to_string(),
        permissions: vec![Permission::TaskRead, Permission::TaskWrite],
        auth_type: AuthType::ApiKey("test-key".to_string()),
    };

    assert!(user.has_permission(Permission::TaskRead));
    assert!(user.has_permission(Permission::TaskWrite));
    assert!(!user.has_permission(Permission::TaskDelete));
    assert!(!user.has_permission(Permission::Admin));

    assert!(user.require_permission(Permission::TaskRead).is_ok());
    assert!(user.require_permission(Permission::TaskDelete).is_err());
}

#[tokio::test]
async fn test_admin_permissions() {
    let admin_user = AuthenticatedUser {
        user_id: "admin".to_string(),
        permissions: vec![Permission::Admin],
        auth_type: AuthType::ApiKey("admin-key".to_string()),
    };

    assert!(admin_user.has_permission(Permission::TaskRead));
    assert!(admin_user.has_permission(Permission::TaskWrite));
    assert!(admin_user.has_permission(Permission::TaskDelete));
    assert!(admin_user.has_permission(Permission::SystemWrite));
}

#[tokio::test]
async fn test_api_key_validation() {
    use sha2::{Digest, Sha256};
    use base64::{engine::general_purpose, Engine as _};
    
    let mut api_keys = std::collections::HashMap::new();
    
    // Create a proper hash for the test key using the same method as ApiKeyService
    let test_key = "test-key";
    let mut hasher = Sha256::new();
    hasher.update(test_key.as_bytes());
    let key_hash = general_purpose::STANDARD.encode(hasher.finalize());
    
    api_keys.insert(
        key_hash.clone(),
        scheduler_api::auth::ApiKeyInfo {
            name: "test-key".to_string(),
            permissions: vec![Permission::TaskRead],
            is_active: true,
        },
    );

    let auth_config = AuthConfig {
        enabled: true,
        jwt_secret: "test-secret".to_string(),
        api_keys,
        jwt_expiration_hours: 24,
    };

    let api_service = scheduler_api::auth::ApiKeyService::new(auth_config.api_keys);
    
    // Test with valid API key
    let result = api_service.validate_api_key(test_key);
    assert!(result.is_ok());
    
    // Test with invalid API key
    let result = api_service.validate_api_key("invalid-key");
    assert!(result.is_err());
}

// Mock implementations for testing
struct MockTaskRepository;
struct MockTaskRunRepository;
struct MockWorkerRepository;
struct MockTaskController;

#[async_trait::async_trait]
impl scheduler_domain::repositories::TaskRepository for MockTaskRepository {
    async fn create(&self, _task: &scheduler_domain::entities::Task) -> scheduler_core::SchedulerResult<scheduler_domain::entities::Task> {
        unimplemented!()
    }

    async fn get_by_id(&self, _id: i64) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::Task>> {
        unimplemented!()
    }

    async fn get_by_name(&self, _name: &str) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::Task>> {
        unimplemented!()
    }

    async fn update(&self, _task: &scheduler_domain::entities::Task) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn delete(&self, _id: i64) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn list(&self, _filter: &scheduler_domain::entities::TaskFilter) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
        Ok(vec![])
    }

    async fn get_active_tasks(&self) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
        unimplemented!()
    }

    async fn get_schedulable_tasks(&self, _current_time: chrono::DateTime<chrono::Utc>) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
        unimplemented!()
    }

    async fn check_dependencies(&self, _task_id: i64) -> scheduler_core::SchedulerResult<bool> {
        unimplemented!()
    }

    async fn get_dependencies(&self, _task_id: i64) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
        unimplemented!()
    }

    async fn batch_update_status(&self, _task_ids: &[i64], _status: scheduler_domain::entities::TaskStatus) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl scheduler_domain::repositories::TaskRunRepository for MockTaskRunRepository {
    async fn create(&self, _task_run: &scheduler_domain::entities::TaskRun) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
        unimplemented!()
    }

    async fn get_by_id(&self, _id: i64) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::TaskRun>> {
        unimplemented!()
    }

    async fn update(&self, _task_run: &scheduler_domain::entities::TaskRun) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn delete(&self, _id: i64) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn get_by_task_id(&self, _task_id: i64) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        unimplemented!()
    }

    async fn get_by_worker_id(&self, _worker_id: &str) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        unimplemented!()
    }

    async fn get_by_status(&self, _status: scheduler_domain::entities::TaskRunStatus) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        Ok(vec![])
    }

    async fn get_pending_runs(&self, _limit: Option<i64>) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        unimplemented!()
    }

    async fn get_running_runs(&self) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        unimplemented!()
    }

    async fn get_timeout_runs(&self, _timeout_seconds: i64) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        unimplemented!()
    }

    async fn update_status(&self, _id: i64, _status: scheduler_domain::entities::TaskRunStatus, _worker_id: Option<&str>) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn update_result(&self, _id: i64, _result: Option<&str>, _error_message: Option<&str>) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn get_recent_runs(&self, _task_id: i64, _limit: i64) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        unimplemented!()
    }

    async fn get_execution_stats(&self, _task_id: i64, _days: i32) -> scheduler_core::SchedulerResult<TaskExecutionStats> {
        unimplemented!()
    }

    async fn cleanup_old_runs(&self, _days: i32) -> scheduler_core::SchedulerResult<u64> {
        unimplemented!()
    }

    async fn batch_update_status(&self, _run_ids: &[i64], _status: scheduler_domain::entities::TaskRunStatus) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl scheduler_domain::repositories::WorkerRepository for MockWorkerRepository {
    async fn register(&self, _worker: &scheduler_domain::entities::WorkerInfo) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn unregister(&self, _worker_id: &str) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn get_by_id(&self, _worker_id: &str) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::WorkerInfo>> {
        unimplemented!()
    }

    async fn update(&self, _worker: &scheduler_domain::entities::WorkerInfo) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn list(&self) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
        Ok(vec![])
    }

    async fn get_alive_workers(&self) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
        unimplemented!()
    }

    async fn get_workers_by_task_type(&self, _task_type: &str) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
        unimplemented!()
    }

    async fn update_heartbeat(&self, _worker_id: &str, _heartbeat_time: chrono::DateTime<chrono::Utc>, _current_task_count: i32) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn update_status(&self, _worker_id: &str, _status: scheduler_domain::entities::WorkerStatus) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn get_timeout_workers(&self, _timeout_seconds: i64) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
        unimplemented!()
    }

    async fn cleanup_offline_workers(&self, _timeout_seconds: i64) -> scheduler_core::SchedulerResult<u64> {
        unimplemented!()
    }

    async fn get_worker_load_stats(&self) -> scheduler_core::SchedulerResult<Vec<WorkerLoadStats>> {
        unimplemented!()
    }

    async fn batch_update_status(&self, _worker_ids: &[String], _status: scheduler_domain::entities::WorkerStatus) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl scheduler_core::traits::TaskControlService for MockTaskController {
    async fn trigger_task(&self, _task_id: i64) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
        unimplemented!()
    }

    async fn pause_task(&self, _task_id: i64) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn resume_task(&self, _task_id: i64) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn restart_task_run(&self, _task_run_id: i64) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
        unimplemented!()
    }

    async fn abort_task_run(&self, _task_run_id: i64) -> scheduler_core::SchedulerResult<()> {
        unimplemented!()
    }

    async fn cancel_all_task_runs(&self, _task_id: i64) -> scheduler_core::SchedulerResult<usize> {
        unimplemented!()
    }

    async fn has_running_instances(&self, _task_id: i64) -> scheduler_core::SchedulerResult<bool> {
        unimplemented!()
    }

    async fn get_recent_executions(&self, _task_id: i64, _limit: usize) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        unimplemented!()
    }
}