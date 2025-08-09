use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::{
    auth::{auth_middleware, optional_auth_middleware, AuthConfig},
    handlers::{
        auth::{create_api_key, login, logout, refresh_token, validate_token},
        health::health_check,
        system::{get_system_health, get_system_stats},
        tasks::{
            create_task, delete_task, get_task, get_task_execution_stats, get_task_run, get_task_runs,
            list_tasks, trigger_task, update_task,
        },
        workers::{get_worker, get_worker_stats, list_workers},
    },
};

#[derive(Clone)]
pub struct AppState {
    pub task_repo: Arc<dyn scheduler_core::traits::repository::TaskRepository>,
    pub task_run_repo: Arc<dyn scheduler_core::traits::repository::TaskRunRepository>,
    pub worker_repo: Arc<dyn scheduler_core::traits::repository::WorkerRepository>,
    pub task_controller: Arc<dyn scheduler_core::traits::scheduler::TaskControlService>,
    pub auth_config: Arc<AuthConfig>,
}

pub fn create_routes(state: AppState) -> Router {
    let router = Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/login", post(login))
        .route("/api/auth/validate", get(validate_token))
        .route("/api/auth/refresh", post(refresh_token))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/api-keys", post(create_api_key))
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/tasks/{id}",
            get(get_task).put(update_task).delete(delete_task),
        )
        .route("/api/tasks/{id}/trigger", post(trigger_task))
        .route("/api/tasks/{id}/runs", get(get_task_runs))
        .route("/api/task-runs/{id}", get(get_task_run))
        .route("/api/workers", get(list_workers))
        .route("/api/workers/{id}", get(get_worker))
        .route("/api/workers/{id}/stats", get(get_worker_stats))
        .route("/api/system/stats", get(get_system_stats))
        .route("/api/system/health", get(get_system_health))
        .route("/api/tasks/{id}/stats", get(get_task_execution_stats))
        .with_state(state.clone());

    if state.auth_config.enabled {
        router.layer(middleware::from_fn_with_state(state, auth_middleware))
    } else {
        router.layer(middleware::from_fn_with_state(state, optional_auth_middleware))
    }
}
