use axum::extract::State;
use serde::{Deserialize, Serialize};
use scheduler_observability::{HealthStatus, LogRotationStats, MonitoringStats};

use crate::{
    error::ApiResult,
    response::success,
    routes::AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitoringResponse {
    pub uptime_seconds: u64,
    pub health_status: String,
    pub performance_summary: String,
    pub log_rotation: Option<LogRotationInfo>,
    pub is_monitoring_active: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogRotationInfo {
    pub current_file_size_bytes: u64,
    pub rotated_files_count: u32,
    pub total_log_size_bytes: u64,
    pub max_file_size_bytes: u64,
    pub max_files: u32,
    pub size_utilization_percent: f64,
    pub files_utilization_percent: f64,
}

/// 获取监控统计信息
pub async fn get_monitoring_stats(
    State(_state): State<AppState>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 注意：这个实现是占位符，因为我们需要从嵌入式应用句柄获取监控统计
    // 在实际集成中，需要通过某种方式访问 EmbeddedApplicationHandle
    
    let monitoring_response = MonitoringResponse {
        uptime_seconds: 0,
        health_status: "unknown".to_string(),
        performance_summary: "Monitoring data not available in this context".to_string(),
        log_rotation: None,
        is_monitoring_active: false,
        timestamp: chrono::Utc::now(),
    };

    Ok(success(monitoring_response))
}

/// 获取详细的性能指标
pub async fn get_performance_metrics(
    State(state): State<AppState>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 获取基本的系统指标
    let all_tasks = state.task_repo.list(&scheduler_domain::entities::TaskFilter::default()).await?;
    let all_workers = state.worker_repo.list().await?;
    
    let active_tasks = all_tasks
        .iter()
        .filter(|t| matches!(t.status, scheduler_domain::entities::TaskStatus::Active))
        .count();
    
    let active_workers = all_workers
        .iter()
        .filter(|w| matches!(w.status, scheduler_domain::entities::WorkerStatus::Alive))
        .count();

    let performance_metrics = PerformanceMetricsResponse {
        active_tasks: active_tasks as u64,
        total_tasks: all_tasks.len() as u64,
        active_workers: active_workers as u64,
        total_workers: all_workers.len() as u64,
        system_load: SystemLoadInfo {
            cpu_usage_percent: None,
            memory_usage_mb: None,
            disk_usage_percent: None,
            network_io_mb_per_sec: None,
        },
        database_info: DatabaseInfo {
            connection_pool_size: 5, // 从配置获取
            active_connections: 1,   // 简化实现
            query_count_per_minute: 0.0,
            slow_query_count: 0,
        },
        queue_info: QueueInfo {
            memory_queue_size: 0,
            pending_tasks: 0,
            processing_rate_per_minute: 0.0,
        },
        timestamp: chrono::Utc::now(),
    };

    Ok(success(performance_metrics))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetricsResponse {
    pub active_tasks: u64,
    pub total_tasks: u64,
    pub active_workers: u64,
    pub total_workers: u64,
    pub system_load: SystemLoadInfo,
    pub database_info: DatabaseInfo,
    pub queue_info: QueueInfo,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemLoadInfo {
    pub cpu_usage_percent: Option<f64>,
    pub memory_usage_mb: Option<f64>,
    pub disk_usage_percent: Option<f64>,
    pub network_io_mb_per_sec: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub connection_pool_size: u32,
    pub active_connections: u32,
    pub query_count_per_minute: f64,
    pub slow_query_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueInfo {
    pub memory_queue_size: u64,
    pub pending_tasks: u64,
    pub processing_rate_per_minute: f64,
}

/// 将 HealthStatus 转换为字符串
fn health_status_to_string(status: HealthStatus) -> String {
    match status {
        HealthStatus::Healthy => "healthy".to_string(),
        HealthStatus::Warning => "warning".to_string(),
        HealthStatus::Critical => "critical".to_string(),
        HealthStatus::Unknown => "unknown".to_string(),
    }
}

impl From<LogRotationStats> for LogRotationInfo {
    fn from(stats: LogRotationStats) -> Self {
        let size_utilization = if stats.max_file_size > 0 {
            (stats.current_file_size as f64 / stats.max_file_size as f64) * 100.0
        } else {
            0.0
        };

        let files_utilization = if stats.max_files > 0 {
            (stats.rotated_files_count as f64 / stats.max_files as f64) * 100.0
        } else {
            0.0
        };

        Self {
            current_file_size_bytes: stats.current_file_size,
            rotated_files_count: stats.rotated_files_count,
            total_log_size_bytes: stats.total_log_size,
            max_file_size_bytes: stats.max_file_size,
            max_files: stats.max_files,
            size_utilization_percent: size_utilization,
            files_utilization_percent: files_utilization,
        }
    }
}

impl From<MonitoringStats> for MonitoringResponse {
    fn from(stats: MonitoringStats) -> Self {
        Self {
            uptime_seconds: stats.uptime_seconds,
            health_status: health_status_to_string(stats.health_status),
            performance_summary: stats.performance_summary,
            log_rotation: stats.log_rotation_stats.map(|s| s.into()),
            is_monitoring_active: stats.is_running,
            timestamp: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_conversion() {
        assert_eq!(health_status_to_string(HealthStatus::Healthy), "healthy");
        assert_eq!(health_status_to_string(HealthStatus::Warning), "warning");
        assert_eq!(health_status_to_string(HealthStatus::Critical), "critical");
        assert_eq!(health_status_to_string(HealthStatus::Unknown), "unknown");
    }

    #[test]
    fn test_log_rotation_info_conversion() {
        let stats = LogRotationStats {
            current_file_size: 1024,
            rotated_files_count: 3,
            total_log_size: 5120,
            max_file_size: 2048,
            max_files: 10,
        };

        let info: LogRotationInfo = stats.into();
        assert_eq!(info.current_file_size_bytes, 1024);
        assert_eq!(info.rotated_files_count, 3);
        assert_eq!(info.total_log_size_bytes, 5120);
        assert_eq!(info.max_file_size_bytes, 2048);
        assert_eq!(info.max_files, 10);
        assert_eq!(info.size_utilization_percent, 50.0); // 1024/2048 * 100
        assert_eq!(info.files_utilization_percent, 30.0); // 3/10 * 100
    }

    #[test]
    fn test_monitoring_response_creation() {
        let response = MonitoringResponse {
            uptime_seconds: 3600,
            health_status: "healthy".to_string(),
            performance_summary: "System running normally".to_string(),
            log_rotation: None,
            is_monitoring_active: true,
            timestamp: chrono::Utc::now(),
        };

        assert_eq!(response.uptime_seconds, 3600);
        assert_eq!(response.health_status, "healthy");
        assert!(response.is_monitoring_active);
        assert!(response.log_rotation.is_none());
    }

    #[test]
    fn test_performance_metrics_response_creation() {
        let response = PerformanceMetricsResponse {
            active_tasks: 5,
            total_tasks: 10,
            active_workers: 2,
            total_workers: 3,
            system_load: SystemLoadInfo {
                cpu_usage_percent: Some(45.5),
                memory_usage_mb: Some(2048.0),
                disk_usage_percent: Some(75.0),
                network_io_mb_per_sec: Some(10.5),
            },
            database_info: DatabaseInfo {
                connection_pool_size: 10,
                active_connections: 3,
                query_count_per_minute: 120.0,
                slow_query_count: 2,
            },
            queue_info: QueueInfo {
                memory_queue_size: 50,
                pending_tasks: 8,
                processing_rate_per_minute: 30.0,
            },
            timestamp: chrono::Utc::now(),
        };

        assert_eq!(response.active_tasks, 5);
        assert_eq!(response.total_tasks, 10);
        assert_eq!(response.active_workers, 2);
        assert_eq!(response.total_workers, 3);
        assert_eq!(response.system_load.cpu_usage_percent, Some(45.5));
        assert_eq!(response.database_info.connection_pool_size, 10);
        assert_eq!(response.queue_info.memory_queue_size, 50);
    }
}
