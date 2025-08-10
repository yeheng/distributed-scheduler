use axum::extract::{Path, Query, State};
use scheduler_domain::entities::{WorkerInfo, WorkerStatus};
use scheduler_domain::repositories::WorkerLoadStats;
use serde::{Deserialize, Serialize};

use crate::{
    error::ApiResult,
    response::{success, PaginatedResponse},
    routes::AppState,
};

#[derive(Debug, Deserialize)]
pub struct WorkerQueryParams {
    pub status: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct WorkerDetailResponse {
    pub id: String,
    pub hostname: String,
    pub ip_address: String,
    pub supported_task_types: Vec<String>,
    pub max_concurrent_tasks: i32,
    pub current_task_count: i32,
    pub status: WorkerStatus,
    pub load_percentage: f64,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
    pub registered_at: chrono::DateTime<chrono::Utc>,
}

impl From<WorkerInfo> for WorkerDetailResponse {
    fn from(worker: WorkerInfo) -> Self {
        let load_percentage = worker.load_percentage();
        Self {
            id: worker.id,
            hostname: worker.hostname,
            ip_address: worker.ip_address,
            supported_task_types: worker.supported_task_types,
            max_concurrent_tasks: worker.max_concurrent_tasks,
            current_task_count: worker.current_task_count,
            status: worker.status,
            load_percentage,
            last_heartbeat: worker.last_heartbeat,
            registered_at: worker.registered_at,
        }
    }
}

pub async fn list_workers(
    State(state): State<AppState>,
    Query(params): Query<WorkerQueryParams>,
) -> ApiResult<impl axum::response::IntoResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let mut workers = if let Some(status) = params.status {
        match status.to_uppercase().as_str() {
            "ALIVE" => state.worker_repo.get_alive_workers().await?,
            "DOWN" => {
                let all_workers = state.worker_repo.list().await?;
                all_workers
                    .into_iter()
                    .filter(|w| matches!(w.status, WorkerStatus::Down))
                    .collect()
            }
            _ => state.worker_repo.list().await?,
        }
    } else {
        state.worker_repo.list().await?
    };

    let total = workers.len() as i64;
    let start = ((page - 1) * page_size) as usize;
    let _end = (start + page_size as usize).min(workers.len());

    if start < workers.len() {
        workers = workers
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();
    } else {
        workers = Vec::new();
    }

    let worker_responses: Vec<WorkerDetailResponse> = workers
        .into_iter()
        .map(WorkerDetailResponse::from)
        .collect();

    let paginated_response = PaginatedResponse::new(worker_responses, total, page, page_size);
    Ok(success(paginated_response))
}

pub async fn get_worker(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<impl axum::response::IntoResponse> {
    match state.worker_repo.get_by_id(&id).await? {
        Some(worker) => {
            let response = WorkerDetailResponse::from(worker);
            Ok(success(response))
        }
        None => Err(crate::error::ApiError::NotFound),
    }
}

pub async fn get_worker_stats(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<impl axum::response::IntoResponse> {
    let worker = match state.worker_repo.get_by_id(&id).await? {
        Some(worker) => worker,
        None => return Err(crate::error::ApiError::NotFound),
    };
    let load_stats = state.worker_repo.get_worker_load_stats().await?;
    let worker_load_stat = load_stats.into_iter().find(|stat| stat.worker_id == id);

    match worker_load_stat {
        Some(stat) => Ok(success(stat)),
        None => {
            let load_percentage = worker.load_percentage();
            let basic_stat = WorkerLoadStats {
                worker_id: worker.id,
                current_task_count: worker.current_task_count,
                max_concurrent_tasks: worker.max_concurrent_tasks,
                load_percentage,
                total_completed_tasks: 0,
                total_failed_tasks: 0,
                average_task_duration_ms: None,
                last_heartbeat: worker.last_heartbeat,
            };
            Ok(success(basic_stat))
        }
    }
}
