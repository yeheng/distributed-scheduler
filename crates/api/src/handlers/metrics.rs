use axum::extract::State;
use serde::{Deserialize, Serialize};
use scheduler_domain::entities::{TaskFilter, TaskRunStatus, TaskStatus, WorkerStatus};

use crate::{
    error::ApiResult,
    response::success,
    routes::AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub system: SystemInfo,
    pub tasks: TaskMetrics,
    pub workers: WorkerMetrics,
    pub performance: PerformanceMetrics,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub uptime_seconds: u64,
    pub version: String,
    pub status: String,
    pub memory_usage_mb: Option<f64>,
    pub cpu_usage_percent: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub total_tasks: i64,
    pub active_tasks: i64,
    pub inactive_tasks: i64,
    pub task_runs_today: TaskRunMetrics,
    pub task_runs_total: TaskRunTotalMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRunMetrics {
    pub completed: i64,
    pub failed: i64,
    pub running: i64,
    pub pending: i64,
    pub success_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRunTotalMetrics {
    pub completed: i64,
    pub failed: i64,
    pub running: i64,
    pub pending: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerMetrics {
    pub total_workers: i64,
    pub active_workers: i64,
    pub inactive_workers: i64,
    pub total_capacity: i64,
    pub used_capacity: i64,
    pub capacity_utilization: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub avg_task_duration_seconds: Option<f64>,
    pub tasks_per_minute: f64,
    pub queue_depth: i64,
    pub error_rate: f64,
}

/// 获取系统指标
pub async fn get_metrics(
    State(state): State<AppState>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 获取任务统计
    let all_tasks = state.task_repo.list(&TaskFilter::default()).await?;
    let total_tasks = all_tasks.len() as i64;
    let active_tasks = all_tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Active))
        .count() as i64;
    let inactive_tasks = total_tasks - active_tasks;

    // 获取Worker统计
    let all_workers = state.worker_repo.list().await?;
    let total_workers = all_workers.len() as i64;
    let active_workers = all_workers
        .iter()
        .filter(|w| matches!(w.status, WorkerStatus::Alive))
        .count() as i64;
    let inactive_workers = total_workers - active_workers;

    // 计算总容量和已用容量
    let total_capacity: i64 = all_workers.iter().map(|w| w.max_concurrent_tasks as i64).sum();
    let used_capacity: i64 = all_workers.iter().map(|w| w.current_task_count as i64).sum();
    let capacity_utilization = if total_capacity > 0 {
        (used_capacity as f64 / total_capacity as f64) * 100.0
    } else {
        0.0
    };

    // 获取任务执行统计
    let running_runs = state.task_run_repo.get_by_status(TaskRunStatus::Running).await?;
    let pending_runs = state.task_run_repo.get_by_status(TaskRunStatus::Pending).await?;
    let completed_runs = state.task_run_repo.get_by_status(TaskRunStatus::Completed).await?;
    let failed_runs = state.task_run_repo.get_by_status(TaskRunStatus::Failed).await?;

    let total_runs = running_runs.len() + pending_runs.len() + completed_runs.len() + failed_runs.len();

    // 计算今日统计
    let today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();

    let completed_today = completed_runs
        .iter()
        .filter(|run| {
            run.completed_at
                .is_some_and(|completed| completed >= today_start)
        })
        .count() as i64;

    let failed_today = failed_runs
        .iter()
        .filter(|run| {
            run.completed_at
                .is_some_and(|completed| completed >= today_start)
        })
        .count() as i64;

    let total_today = completed_today + failed_today;
    let success_rate_today = if total_today > 0 {
        (completed_today as f64 / total_today as f64) * 100.0
    } else {
        0.0
    };

    // 计算性能指标
    let avg_duration = calculate_average_task_duration(&completed_runs);
    let tasks_per_minute = calculate_tasks_per_minute(&completed_runs);
    let queue_depth = pending_runs.len() as i64;
    let error_rate = if total_runs > 0 {
        (failed_runs.len() as f64 / total_runs as f64) * 100.0
    } else {
        0.0
    };

    // 获取系统信息
    let uptime_seconds = get_system_uptime();
    let (memory_usage, cpu_usage) = get_system_resources();

    let metrics = SystemMetrics {
        system: SystemInfo {
            uptime_seconds,
            version: env!("CARGO_PKG_VERSION").to_string(),
            status: determine_system_status(active_workers, total_workers, error_rate),
            memory_usage_mb: memory_usage,
            cpu_usage_percent: cpu_usage,
        },
        tasks: TaskMetrics {
            total_tasks,
            active_tasks,
            inactive_tasks,
            task_runs_today: TaskRunMetrics {
                completed: completed_today,
                failed: failed_today,
                running: running_runs.len() as i64,
                pending: pending_runs.len() as i64,
                success_rate: success_rate_today,
            },
            task_runs_total: TaskRunTotalMetrics {
                completed: completed_runs.len() as i64,
                failed: failed_runs.len() as i64,
                running: running_runs.len() as i64,
                pending: pending_runs.len() as i64,
                total: total_runs as i64,
            },
        },
        workers: WorkerMetrics {
            total_workers,
            active_workers,
            inactive_workers,
            total_capacity,
            used_capacity,
            capacity_utilization,
        },
        performance: PerformanceMetrics {
            avg_task_duration_seconds: avg_duration,
            tasks_per_minute,
            queue_depth,
            error_rate,
        },
        timestamp: chrono::Utc::now(),
    };

    Ok(success(metrics))
}

/// 计算平均任务执行时间
fn calculate_average_task_duration(completed_runs: &[scheduler_domain::entities::TaskRun]) -> Option<f64> {
    let durations: Vec<f64> = completed_runs
        .iter()
        .filter_map(|run| {
            if let (Some(started), Some(completed)) = (run.started_at, run.completed_at) {
                Some((completed - started).num_seconds() as f64)
            } else {
                None
            }
        })
        .collect();

    if durations.is_empty() {
        None
    } else {
        Some(durations.iter().sum::<f64>() / durations.len() as f64)
    }
}

/// 计算每分钟任务完成数
fn calculate_tasks_per_minute(completed_runs: &[scheduler_domain::entities::TaskRun]) -> f64 {
    let now = chrono::Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);

    let recent_completions = completed_runs
        .iter()
        .filter(|run| {
            run.completed_at
                .is_some_and(|completed| completed >= one_hour_ago)
        })
        .count();

    recent_completions as f64 / 60.0 // 转换为每分钟
}

/// 获取系统运行时间（秒）
fn get_system_uptime() -> u64 {
    // 简化实现，实际应该从系统启动时间计算
    // 这里返回一个占位值，实际实现需要跟踪应用启动时间
    0
}

/// 获取系统资源使用情况
fn get_system_resources() -> (Option<f64>, Option<f64>) {
    // 简化实现，实际应该使用系统API获取真实的内存和CPU使用率
    // 这里返回占位值，实际实现可以使用 sysinfo crate
    (None, None)
}

/// 确定系统状态
fn determine_system_status(active_workers: i64, total_workers: i64, error_rate: f64) -> String {
    if active_workers == 0 {
        "critical".to_string()
    } else if error_rate > 50.0 {
        "degraded".to_string()
    } else if active_workers < total_workers / 2 {
        "warning".to_string()
    } else {
        "healthy".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_domain::entities::TaskRun;

    #[test]
    fn test_calculate_average_task_duration() {
        let now = chrono::Utc::now();
        let runs = vec![
            TaskRun {
                id: 1,
                task_id: 1,
                status: TaskRunStatus::Completed,
                worker_id: Some("worker1".to_string()),
                retry_count: 0,
                shard_index: None,
                shard_total: None,
                scheduled_at: now - chrono::Duration::minutes(10),
                started_at: Some(now - chrono::Duration::minutes(5)),
                completed_at: Some(now),
                result: None,
                error_message: None,
                created_at: now - chrono::Duration::minutes(10),
            },
        ];

        let avg = calculate_average_task_duration(&runs);
        assert!(avg.is_some());
        assert_eq!(avg.unwrap(), 300.0); // 5 minutes = 300 seconds
    }

    #[test]
    fn test_calculate_average_task_duration_empty() {
        let runs = vec![];
        let avg = calculate_average_task_duration(&runs);
        assert!(avg.is_none());
    }

    #[test]
    fn test_calculate_tasks_per_minute() {
        let now = chrono::Utc::now();
        let runs = vec![
            TaskRun {
                id: 1,
                task_id: 1,
                status: TaskRunStatus::Completed,
                worker_id: Some("worker1".to_string()),
                retry_count: 0,
                shard_index: None,
                shard_total: None,
                scheduled_at: now - chrono::Duration::minutes(30),
                started_at: Some(now - chrono::Duration::minutes(30)),
                completed_at: Some(now - chrono::Duration::minutes(29)),
                result: None,
                error_message: None,
                created_at: now - chrono::Duration::minutes(30),
            },
        ];

        let rate = calculate_tasks_per_minute(&runs);
        assert_eq!(rate, 1.0 / 60.0); // 1 task in the last hour = 1/60 per minute
    }

    #[test]
    fn test_determine_system_status() {
        assert_eq!(determine_system_status(0, 5, 10.0), "critical");
        assert_eq!(determine_system_status(3, 5, 60.0), "degraded");
        assert_eq!(determine_system_status(1, 5, 10.0), "warning"); // 1 < 5/2 (2), so warning
        assert_eq!(determine_system_status(2, 5, 10.0), "healthy"); // 2 >= 5/2 (2), so healthy
        assert_eq!(determine_system_status(3, 5, 10.0), "healthy"); // 3 >= 5/2 (2), so healthy
        assert_eq!(determine_system_status(5, 5, 10.0), "healthy");
    }

    #[test]
    fn test_get_system_uptime() {
        let uptime = get_system_uptime();
        assert_eq!(uptime, 0); // Placeholder implementation
    }

    #[test]
    fn test_get_system_resources() {
        let (memory, cpu) = get_system_resources();
        assert!(memory.is_none()); // Placeholder implementation
        assert!(cpu.is_none()); // Placeholder implementation
    }
}