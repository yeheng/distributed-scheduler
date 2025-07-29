use axum::extract::State;
use scheduler_core::models::{TaskFilter, TaskRunStatus, TaskStatus, WorkerStatus};
use serde::Serialize;

use crate::{error::ApiResult, response::success, routes::AppState};

/// 系统统计信息
#[derive(Debug, Serialize)]
pub struct SystemStats {
    pub total_tasks: i64,
    pub active_tasks: i64,
    pub total_workers: i64,
    pub active_workers: i64,
    pub running_task_runs: i64,
    pub completed_task_runs_today: i64,
    pub failed_task_runs_today: i64,
}

/// 系统健康状态
#[derive(Debug, Serialize)]
pub struct SystemHealth {
    pub status: String,
    pub database_status: String,
    pub message_queue_status: String,
    pub uptime_seconds: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 获取系统统计信息
pub async fn get_system_stats(
    State(state): State<AppState>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 获取任务统计
    let all_tasks = state.task_repo.list(&TaskFilter::default()).await?;
    let total_tasks = all_tasks.len() as i64;
    let active_tasks = all_tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Active))
        .count() as i64;

    // 获取Worker统计
    let all_workers = state.worker_repo.list().await?;
    let total_workers = all_workers.len() as i64;
    let active_workers = all_workers
        .iter()
        .filter(|w| matches!(w.status, WorkerStatus::Alive))
        .count() as i64;

    // 获取任务执行统计
    let running_task_runs = state
        .task_run_repo
        .get_by_status(TaskRunStatus::Running)
        .await?
        .len() as i64;

    // 获取今天的完成和失败任务数
    let today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    let completed_runs_today = state
        .task_run_repo
        .get_by_status(TaskRunStatus::Completed)
        .await?
        .into_iter()
        .filter(|run| {
            run.completed_at
                .is_some_and(|completed| completed >= today_start)
        })
        .count() as i64;

    let failed_runs_today = state
        .task_run_repo
        .get_by_status(TaskRunStatus::Failed)
        .await?
        .into_iter()
        .filter(|run| {
            run.completed_at
                .is_some_and(|completed| completed >= today_start)
        })
        .count() as i64;

    let stats = SystemStats {
        total_tasks,
        active_tasks,
        total_workers,
        active_workers,
        running_task_runs,
        completed_task_runs_today: completed_runs_today,
        failed_task_runs_today: failed_runs_today,
    };

    Ok(success(stats))
}

/// 获取系统健康状态
pub async fn get_system_health(
    State(state): State<AppState>,
) -> ApiResult<impl axum::response::IntoResponse> {
    let database_status;
    let message_queue_status = "healthy".to_string(); // 暂时设为健康，实际应该检查消息队列连接
    let mut overall_status = "healthy".to_string();

    // 检查数据库连接
    match state.task_repo.list(&TaskFilter::default()).await {
        Ok(_) => {
            database_status = "healthy".to_string();
        }
        Err(_) => {
            database_status = "unhealthy".to_string();
            overall_status = "unhealthy".to_string();
        }
    }

    // 检查是否有活跃的Worker
    match state.worker_repo.get_alive_workers().await {
        Ok(workers) => {
            if workers.is_empty() {
                overall_status = "degraded".to_string();
            }
        }
        Err(_) => {
            overall_status = "unhealthy".to_string();
        }
    }

    // 计算系统运行时间（这里简化处理，实际应该从系统启动时间计算）
    let uptime_seconds = 0u64; // 暂时设为0，实际应该从系统启动时间计算

    let health = SystemHealth {
        status: overall_status,
        database_status,
        message_queue_status,
        uptime_seconds,
        timestamp: chrono::Utc::now(),
    };

    Ok(success(health))
}
