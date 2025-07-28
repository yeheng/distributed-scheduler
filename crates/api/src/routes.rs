use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::handlers::{
    health::health_check,
    system::{get_system_health, get_system_stats},
    tasks::{
        create_task, delete_task, get_task, get_task_run, get_task_runs, list_tasks, trigger_task,
        update_task,
    },
    workers::{get_worker, get_worker_stats, list_workers},
};

/// API应用状态
#[derive(Clone)]
pub struct AppState {
    pub task_repo: Arc<dyn scheduler_core::traits::repository::TaskRepository>,
    pub task_run_repo: Arc<dyn scheduler_core::traits::repository::TaskRunRepository>,
    pub worker_repo: Arc<dyn scheduler_core::traits::repository::WorkerRepository>,
    pub task_controller: Arc<dyn scheduler_core::traits::scheduler::TaskControlService>,
}

/// 创建API路由
pub fn create_routes(state: AppState) -> Router {
    Router::new()
        // 健康检查
        .route("/health", get(health_check))
        // 任务管理API
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/tasks/{id}", get(get_task))
        .route("/api/tasks/{id}/update", post(update_task))
        .route("/api/tasks/{id}/delete", post(delete_task))
        .route("/api/tasks/{id}/trigger", post(trigger_task))
        .route("/api/tasks/{id}/runs", get(get_task_runs))
        .route("/api/task-runs/{id}", get(get_task_run))
        // Worker管理API
        .route("/api/workers", get(list_workers))
        .route("/api/workers/{id}", get(get_worker))
        .route("/api/workers/{id}/stats", get(get_worker_stats))
        // 系统监控API
        .route("/api/system/stats", get(get_system_stats))
        .route("/api/system/health", get(get_system_health))
        .with_state(state)
}
