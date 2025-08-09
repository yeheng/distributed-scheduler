
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod response;
pub mod routes;
pub mod auth;

use axum::Router;
use std::sync::Arc;
use tower::ServiceBuilder;

use middleware::{cors_layer, request_logging, trace_layer};
use routes::{create_routes, AppState};
use scheduler_core::config::models::api_observability::AuthConfig as CoreAuthConfig;
use scheduler_core::traits::repository::*;
use scheduler_core::traits::scheduler::*;
use scheduler_core::config::models::api_observability::ApiConfig;

pub fn create_app(
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    worker_repo: Arc<dyn WorkerRepository>,
    task_controller: Arc<dyn TaskControlService>,
    api_config: ApiConfig,
) -> Router {
    let auth_config = convert_auth_config(&api_config.auth);

    let state = AppState {
        task_repo,
        task_run_repo,
        worker_repo,
        task_controller,
        auth_config,
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
) -> Router {
    let default_api_config = ApiConfig {
        enabled: true,
        bind_address: "0.0.0.0:8080".to_string(),
        cors_enabled: true,
        cors_origins: vec!["*".to_string()],
        request_timeout_seconds: 30,
        max_request_size_mb: 10,
        auth: CoreAuthConfig::default(),
    };

    create_app(task_repo, task_run_repo, worker_repo, task_controller, default_api_config)
}
fn convert_auth_config(core_config: &CoreAuthConfig) -> Arc<auth::AuthConfig> {
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
        jwt_expiration_hours: core_config.jwt_expiration_hours,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    struct MockTaskRepository;
    struct MockTaskRunRepository;
    struct MockWorkerRepository;
    struct MockTaskController;

    #[async_trait::async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn create(
            &self,
            _task: &scheduler_domain::entities::Task,
        ) -> scheduler_core::SchedulerResult<scheduler_domain::entities::Task> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _id: i64,
        ) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn get_by_name(
            &self,
            _name: &str,
        ) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _task: &scheduler_domain::entities::Task,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn list(
            &self,
            _filter: &scheduler_domain::entities::TaskFilter,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            Ok(vec![])
        }

        async fn get_active_tasks(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn get_schedulable_tasks(
            &self,
            _current_time: chrono::DateTime<chrono::Utc>,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn check_dependencies(&self, _task_id: i64) -> scheduler_core::SchedulerResult<bool> {
            unimplemented!()
        }

        async fn get_dependencies(
            &self,
            _task_id: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _task_ids: &[i64],
            _status: scheduler_domain::entities::TaskStatus,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl TaskRunRepository for MockTaskRunRepository {
        async fn create(
            &self,
            _task_run: &scheduler_domain::entities::TaskRun,
        ) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _id: i64,
        ) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _task_run: &scheduler_domain::entities::TaskRun,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_by_task_id(
            &self,
            _task_id: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_by_worker_id(
            &self,
            _worker_id: &str,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_by_status(
            &self,
            _status: scheduler_domain::entities::TaskRunStatus,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            Ok(vec![])
        }

        async fn get_pending_runs(
            &self,
            _limit: Option<i64>,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_running_runs(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_timeout_runs(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _id: i64,
            _status: scheduler_domain::entities::TaskRunStatus,
            _worker_id: Option<&str>,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn update_result(
            &self,
            _id: i64,
            _result: Option<&str>,
            _error_message: Option<&str>,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_recent_runs(
            &self,
            _task_id: i64,
            _limit: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_execution_stats(
            &self,
            _task_id: i64,
            _days: i32,
        ) -> scheduler_core::SchedulerResult<TaskExecutionStats>
        {
            unimplemented!()
        }

        async fn cleanup_old_runs(&self, _days: i32) -> scheduler_core::SchedulerResult<u64> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _run_ids: &[i64],
            _status: scheduler_domain::entities::TaskRunStatus,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl WorkerRepository for MockWorkerRepository {
        async fn register(
            &self,
            _worker: &scheduler_domain::entities::WorkerInfo,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn unregister(&self, _worker_id: &str) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _worker_id: &str,
        ) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::WorkerInfo>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _worker: &scheduler_domain::entities::WorkerInfo,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn list(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
            Ok(vec![])
        }

        async fn get_alive_workers(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
            Ok(vec![])
        }

        async fn get_workers_by_task_type(
            &self,
            _task_type: &str,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
            unimplemented!()
        }

        async fn update_heartbeat(
            &self,
            _worker_id: &str,
            _heartbeat_time: chrono::DateTime<chrono::Utc>,
            _current_task_count: i32,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _worker_id: &str,
            _status: scheduler_domain::entities::WorkerStatus,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_timeout_workers(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
            unimplemented!()
        }

        async fn cleanup_offline_workers(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::SchedulerResult<u64> {
            unimplemented!()
        }

        async fn get_worker_load_stats(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<WorkerLoadStats>>
        {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _worker_ids: &[String],
            _status: scheduler_domain::entities::WorkerStatus,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl TaskControlService for MockTaskController {
        async fn trigger_task(
            &self,
            _task_id: i64,
        ) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn pause_task(&self, _task_id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn resume_task(&self, _task_id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn restart_task_run(
            &self,
            _task_run_id: i64,
        ) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn abort_task_run(&self, _task_run_id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let task_repo = Arc::new(MockTaskRepository)
            as Arc<dyn TaskRepository>;
        let task_run_repo = Arc::new(MockTaskRunRepository)
            as Arc<dyn TaskRunRepository>;
        let worker_repo = Arc::new(MockWorkerRepository)
            as Arc<dyn WorkerRepository>;
        let task_controller = Arc::new(MockTaskController)
            as Arc<dyn TaskControlService>;

        let app = create_simple_app(task_repo, task_run_repo, worker_repo, task_controller);

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
        let task_repo = Arc::new(MockTaskRepository)
            as Arc<dyn TaskRepository>;
        let task_run_repo = Arc::new(MockTaskRunRepository)
            as Arc<dyn TaskRunRepository>;
        let worker_repo = Arc::new(MockWorkerRepository)
            as Arc<dyn WorkerRepository>;
        let task_controller = Arc::new(MockTaskController)
            as Arc<dyn TaskControlService>;

        let app = create_simple_app(task_repo, task_run_repo, worker_repo, task_controller);
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
