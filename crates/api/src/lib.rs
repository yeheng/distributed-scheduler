pub mod auth;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod response;
pub mod routes;
pub mod types;
pub mod validation;

use axum::Router;
use scheduler_application::task_services::TaskControlService;
use std::sync::Arc;
use tower::ServiceBuilder;

use middleware::{
    cors_layer, create_rate_limiting_layer, request_logging, trace_layer, RateLimitConfig,
};
use routes::{create_routes, AppState};
use scheduler_config::models::api_observability::ApiConfig;
use scheduler_config::models::api_observability::AuthConfig;
use scheduler_domain::repositories::*;

pub fn create_app(
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    worker_repo: Arc<dyn WorkerRepository>,
    task_controller: Arc<dyn TaskControlService>,
    authentication_service: Arc<scheduler_application::AuthenticationService>,
    api_config: ApiConfig,
) -> Router {
    let auth_config = convert_auth_config(&api_config.auth);

    // Create rate limiter if enabled
    let rate_limiter = if api_config.rate_limiting.enabled {
        let rate_limit_config = RateLimitConfig {
            max_requests: api_config.rate_limiting.max_requests_per_minute,
            window_duration: std::time::Duration::from_secs(60),
            refill_rate: api_config.rate_limiting.refill_rate_per_second,
            burst_size: api_config.rate_limiting.burst_size,
        };
        Some(create_rate_limiting_layer(rate_limit_config))
    } else {
        None
    };

    let state = AppState {
        task_repo,
        task_run_repo,
        worker_repo,
        task_controller,
        authentication_service,
        auth_config,
        rate_limiter,
    };

    create_routes(state).layer(
        ServiceBuilder::new()
            .layer(trace_layer())
            .layer(cors_layer())
            .layer(axum::middleware::from_fn(request_logging)),
    )
}

pub fn create_simple_app(
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    worker_repo: Arc<dyn WorkerRepository>,
    task_controller: Arc<dyn TaskControlService>,
    authentication_service: Arc<scheduler_application::AuthenticationService>,
) -> Router {
    let default_api_config = ApiConfig {
        enabled: true,
        bind_address: "0.0.0.0:8080".to_string(),
        cors_enabled: true,
        cors_origins: vec!["*".to_string()],
        request_timeout_seconds: 30,
        max_request_size_mb: 10,
        auth: AuthConfig::default(),
        rate_limiting: scheduler_config::models::api_observability::RateLimitingConfig::default(),
    };

    create_app(
        task_repo,
        task_run_repo,
        worker_repo,
        task_controller,
        authentication_service,
        default_api_config,
    )
}

pub fn create_test_app(
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    worker_repo: Arc<dyn WorkerRepository>,
    task_controller: Arc<dyn TaskControlService>,
    authentication_service: Arc<scheduler_application::AuthenticationService>,
) -> Router {
    let test_api_config = ApiConfig {
        enabled: true,
        bind_address: "0.0.0.0:8080".to_string(),
        cors_enabled: true,
        cors_origins: vec!["*".to_string()],
        request_timeout_seconds: 30,
        max_request_size_mb: 10,
        auth: AuthConfig {
            enabled: false, // Disable auth for tests
            jwt_secret: "test-secret".to_string(),
            api_keys: std::collections::HashMap::new(),
            jwt_expiration_hours: 24,
        },
        rate_limiting: scheduler_config::models::api_observability::RateLimitingConfig::default(),
    };

    create_app(
        task_repo,
        task_run_repo,
        worker_repo,
        task_controller,
        authentication_service,
        test_api_config,
    )
}
fn convert_auth_config(core_config: &AuthConfig) -> Arc<auth::AuthConfig> {
    let mut auth_api_keys = std::collections::HashMap::new();

    for (hash, key_config) in &core_config.api_keys {
        let permissions: Vec<auth::Permission> = key_config
            .permissions
            .iter()
            .filter_map(|p| match p.as_str() {
                "TaskRead" => Some(auth::Permission::TaskRead),
                "TaskWrite" => Some(auth::Permission::TaskWrite),
                "TaskDelete" => Some(auth::Permission::TaskDelete),
                "WorkerRead" => Some(auth::Permission::WorkerRead),
                "WorkerWrite" => Some(auth::Permission::WorkerWrite),
                "SystemRead" => Some(auth::Permission::SystemRead),
                "SystemWrite" => Some(auth::Permission::SystemWrite),
                "Admin" => Some(auth::Permission::Admin),
                _ => None,
            })
            .collect();

        auth_api_keys.insert(
            hash.clone(),
            auth::ApiKeyInfo {
                name: key_config.name.clone(),
                permissions,
                is_active: key_config.is_active,
            },
        );
    }

    Arc::new(auth::AuthConfig {
        enabled: core_config.enabled,
        jwt_secret: core_config.jwt_secret.clone(),
        api_keys: auth_api_keys,
        jwt_expiration_hours: core_config.jwt_expiration_hours as i64,
    })
}

#[cfg(test)]
mod tests {
    use super::{create_simple_app, create_test_app};
    use async_trait::async_trait;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use scheduler_application::{task_services::TaskControlService, AuthenticationService};
    use scheduler_domain::{
        repositories::{CreateUserRequest, User, UserRepository, UserRole},
        *,
    };
    use scheduler_errors::SchedulerResult;
    use std::sync::Arc;
    use tower::ServiceExt;
    use uuid::Uuid;

    struct MockTaskRepository;
    struct MockTaskRunRepository;
    struct MockWorkerRepository;
    struct MockTaskController;
    struct MockUserRepository;

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, _request: &CreateUserRequest) -> SchedulerResult<User> {
            let user = User {
                id: Uuid::new_v4(),
                username: "testuser".to_string(),
                email: "test@example.com".to_string(),
                password_hash: "hashed_password".to_string(),
                role: UserRole::Operator,
                is_active: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            Ok(user)
        }

        async fn get_by_id(&self, _user_id: Uuid) -> SchedulerResult<Option<User>> {
            Ok(None)
        }

        async fn get_by_username(&self, username: &str) -> SchedulerResult<Option<User>> {
            if username == "testuser" {
                let user = User {
                    id: Uuid::new_v4(),
                    username: "testuser".to_string(),
                    email: "test@example.com".to_string(),
                    password_hash: bcrypt::hash("password123", bcrypt::DEFAULT_COST).unwrap(),
                    role: UserRole::Operator,
                    is_active: true,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };
                Ok(Some(user))
            } else {
                Ok(None)
            }
        }

        async fn get_by_email(&self, _email: &str) -> SchedulerResult<Option<User>> {
            Ok(None)
        }

        async fn authenticate_user(
            &self,
            username: &str,
            password: &str,
        ) -> SchedulerResult<Option<User>> {
            if let Some(user) = self.get_by_username(username).await? {
                if user.is_active
                    && bcrypt::verify(password, &user.password_hash).unwrap_or(false)
                {
                    Ok(Some(user))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }

        async fn update(&self, _user: &User) -> SchedulerResult<()> {
            Ok(())
        }

        async fn delete(&self, _user_id: Uuid) -> SchedulerResult<()> {
            Ok(())
        }

        async fn list_users(
            &self,
            _limit: Option<i64>,
            _offset: Option<i64>,
        ) -> SchedulerResult<Vec<User>> {
            Ok(vec![])
        }

        async fn update_password(
            &self,
            _user_id: Uuid,
            _password_hash: &str,
        ) -> SchedulerResult<()> {
            Ok(())
        }

        async fn update_role(&self, _user_id: Uuid, _role: UserRole) -> SchedulerResult<()> {
            Ok(())
        }

        async fn activate_user(&self, _user_id: Uuid) -> SchedulerResult<()> {
            Ok(())
        }

        async fn deactivate_user(&self, _user_id: Uuid) -> SchedulerResult<()> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn create(
            &self,
            _task: &scheduler_domain::entities::Task,
        ) -> scheduler_errors::SchedulerResult<scheduler_domain::entities::Task> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _id: i64,
        ) -> scheduler_errors::SchedulerResult<Option<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn get_by_name(
            &self,
            _name: &str,
        ) -> scheduler_errors::SchedulerResult<Option<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _task: &scheduler_domain::entities::Task,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn list(
            &self,
            _filter: &scheduler_domain::entities::TaskFilter,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            Ok(vec![])
        }

        async fn get_active_tasks(
            &self,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn get_schedulable_tasks(
            &self,
            _current_time: chrono::DateTime<chrono::Utc>,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn check_dependencies(
            &self,
            _task_id: i64,
        ) -> scheduler_errors::SchedulerResult<bool> {
            unimplemented!()
        }

        async fn get_dependencies(
            &self,
            _task_id: i64,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _task_ids: &[i64],
            _status: scheduler_domain::entities::TaskStatus,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl TaskRunRepository for MockTaskRunRepository {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        
        async fn create(
            &self,
            _task_run: &scheduler_domain::entities::TaskRun,
        ) -> scheduler_errors::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _id: i64,
        ) -> scheduler_errors::SchedulerResult<Option<scheduler_domain::entities::TaskRun>>
        {
            unimplemented!()
        }

        async fn update(
            &self,
            _task_run: &scheduler_domain::entities::TaskRun,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_by_task_id(
            &self,
            _task_id: i64,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_by_worker_id(
            &self,
            _worker_id: &str,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_by_status(
            &self,
            _status: scheduler_domain::entities::TaskRunStatus,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            Ok(vec![])
        }

        async fn get_pending_runs(
            &self,
            _limit: Option<i64>,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_running_runs(
            &self,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_timeout_runs(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _id: i64,
            _status: scheduler_domain::entities::TaskRunStatus,
            _worker_id: Option<&str>,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn update_result(
            &self,
            _id: i64,
            _result: Option<&str>,
            _error_message: Option<&str>,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_recent_runs(
            &self,
            _task_id: i64,
            _limit: i64,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_execution_stats(
            &self,
            _task_id: i64,
            _days: i32,
        ) -> scheduler_errors::SchedulerResult<TaskExecutionStats> {
            unimplemented!()
        }

        async fn cleanup_old_runs(&self, _days: i32) -> scheduler_errors::SchedulerResult<u64> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _run_ids: &[i64],
            _status: scheduler_domain::entities::TaskRunStatus,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl WorkerRepository for MockWorkerRepository {
        async fn register(
            &self,
            _worker: &scheduler_domain::entities::WorkerInfo,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn unregister(&self, _worker_id: &str) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _worker_id: &str,
        ) -> scheduler_errors::SchedulerResult<Option<scheduler_domain::entities::WorkerInfo>>
        {
            unimplemented!()
        }

        async fn update(
            &self,
            _worker: &scheduler_domain::entities::WorkerInfo,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn list(
            &self,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>>
        {
            Ok(vec![])
        }

        async fn get_alive_workers(
            &self,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>>
        {
            Ok(vec![])
        }

        async fn get_workers_by_task_type(
            &self,
            _task_type: &str,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>>
        {
            unimplemented!()
        }

        async fn update_heartbeat(
            &self,
            _worker_id: &str,
            _heartbeat_time: chrono::DateTime<chrono::Utc>,
            _current_task_count: i32,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _worker_id: &str,
            _status: scheduler_domain::entities::WorkerStatus,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_timeout_workers(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>>
        {
            unimplemented!()
        }

        async fn cleanup_offline_workers(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_errors::SchedulerResult<u64> {
            unimplemented!()
        }

        async fn get_worker_load_stats(
            &self,
        ) -> scheduler_errors::SchedulerResult<Vec<WorkerLoadStats>> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _worker_ids: &[String],
            _status: scheduler_domain::entities::WorkerStatus,
        ) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl TaskControlService for MockTaskController {
        async fn trigger_task(
            &self,
            _task_id: i64,
        ) -> scheduler_errors::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn pause_task(&self, _task_id: i64) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn resume_task(&self, _task_id: i64) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn restart_task_run(
            &self,
            _task_run_id: i64,
        ) -> scheduler_errors::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn abort_task_run(&self, _task_run_id: i64) -> scheduler_errors::SchedulerResult<()> {
            unimplemented!()
        }

        async fn cancel_all_task_runs(
            &self,
            _task_id: i64,
        ) -> scheduler_errors::SchedulerResult<usize> {
            unimplemented!()
        }

        async fn has_running_instances(
            &self,
            _task_id: i64,
        ) -> scheduler_errors::SchedulerResult<bool> {
            unimplemented!()
        }

        async fn get_recent_executions(
            &self,
            _task_id: i64,
            _limit: usize,
        ) -> scheduler_errors::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let task_repo = Arc::new(MockTaskRepository) as Arc<dyn TaskRepository>;
        let task_run_repo = Arc::new(MockTaskRunRepository) as Arc<dyn TaskRunRepository>;
        let worker_repo = Arc::new(MockWorkerRepository) as Arc<dyn WorkerRepository>;
        let task_controller = Arc::new(MockTaskController) as Arc<dyn TaskControlService>;
        let user_repo = Arc::new(MockUserRepository) as Arc<dyn UserRepository>;
        let authentication_service = Arc::new(AuthenticationService::new(user_repo));

        let app = create_simple_app(
            task_repo,
            task_run_repo,
            worker_repo,
            task_controller,
            authentication_service,
        );

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_routes_exist() {
        let task_repo = Arc::new(MockTaskRepository) as Arc<dyn TaskRepository>;
        let task_run_repo = Arc::new(MockTaskRunRepository) as Arc<dyn TaskRunRepository>;
        let worker_repo = Arc::new(MockWorkerRepository) as Arc<dyn WorkerRepository>;
        let task_controller = Arc::new(MockTaskController) as Arc<dyn TaskControlService>;
        let user_repo = Arc::new(MockUserRepository) as Arc<dyn UserRepository>;
        let authentication_service = Arc::new(AuthenticationService::new(user_repo));

        let app = create_test_app(
            task_repo,
            task_run_repo,
            worker_repo,
            task_controller,
            authentication_service,
        );
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/tasks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/workers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/system/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/system/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
