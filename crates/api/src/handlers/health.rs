use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::routes::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub service: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub components: ComponentHealth,
    pub system: SystemHealth,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub database: ComponentStatus,
    pub message_queue: ComponentStatus,
    pub api: ComponentStatus,
    pub scheduler: ComponentStatus,
    pub worker: ComponentStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub status: String,
    pub message: String,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemHealth {
    pub memory_usage_mb: Option<f64>,
    pub cpu_usage_percent: Option<f64>,
    pub disk_usage_percent: Option<f64>,
    pub active_connections: i32,
}

/// 简单的健康检查端点（向后兼容）
pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "embedded-task-scheduler",
        "version": env!("CARGO_PKG_VERSION"),
        "mode": "embedded"
    }))
}

/// 详细的健康检查端点
pub async fn detailed_health_check(
    State(state): State<AppState>,
) -> Json<HealthStatus> {
    let now = chrono::Utc::now();
    
    // 检查数据库健康状态
    let database_status = check_database_health(&state).await;
    
    // 检查消息队列健康状态
    let message_queue_status = check_message_queue_health().await;
    
    // 检查其他组件
    let api_status = ComponentStatus {
        status: "healthy".to_string(),
        message: "API服务正常运行".to_string(),
        last_check: now,
    };
    
    let scheduler_status = ComponentStatus {
        status: "healthy".to_string(),
        message: "调度器服务正常".to_string(),
        last_check: now,
    };
    
    let worker_status = check_worker_health(&state).await;
    
    // 获取系统资源信息
    let system_health = get_system_health_info();
    
    // 确定整体状态
    let overall_status = determine_overall_status(&[
        &database_status,
        &message_queue_status,
        &api_status,
        &scheduler_status,
        &worker_status,
    ]);
    
    Json(HealthStatus {
        status: overall_status,
        timestamp: now,
        service: "embedded-task-scheduler".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: get_uptime_seconds(),
        components: ComponentHealth {
            database: database_status,
            message_queue: message_queue_status,
            api: api_status,
            scheduler: scheduler_status,
            worker: worker_status,
        },
        system: system_health,
    })
}

/// 检查数据库健康状态
async fn check_database_health(state: &AppState) -> ComponentStatus {
    let now = chrono::Utc::now();
    
    match state.task_repo.list(&scheduler_domain::entities::TaskFilter::default()).await {
        Ok(_) => ComponentStatus {
            status: "healthy".to_string(),
            message: "数据库连接正常".to_string(),
            last_check: now,
        },
        Err(e) => ComponentStatus {
            status: "unhealthy".to_string(),
            message: format!("数据库连接失败: {}", e),
            last_check: now,
        },
    }
}

/// 检查消息队列健康状态
async fn check_message_queue_health() -> ComponentStatus {
    let now = chrono::Utc::now();
    
    // 对于内存队列，我们假设它总是健康的
    // 实际实现中可以检查队列的连接状态
    ComponentStatus {
        status: "healthy".to_string(),
        message: "内存消息队列正常运行".to_string(),
        last_check: now,
    }
}

/// 检查Worker健康状态
async fn check_worker_health(state: &AppState) -> ComponentStatus {
    let now = chrono::Utc::now();
    
    match state.worker_repo.get_alive_workers().await {
        Ok(workers) => {
            if workers.is_empty() {
                ComponentStatus {
                    status: "warning".to_string(),
                    message: "没有活跃的Worker".to_string(),
                    last_check: now,
                }
            } else {
                ComponentStatus {
                    status: "healthy".to_string(),
                    message: format!("有 {} 个活跃Worker", workers.len()),
                    last_check: now,
                }
            }
        }
        Err(e) => ComponentStatus {
            status: "unhealthy".to_string(),
            message: format!("无法获取Worker状态: {}", e),
            last_check: now,
        },
    }
}

/// 获取系统健康信息
fn get_system_health_info() -> SystemHealth {
    // 简化实现，实际应该使用系统API获取真实数据
    SystemHealth {
        memory_usage_mb: None,
        cpu_usage_percent: None,
        disk_usage_percent: None,
        active_connections: 0,
    }
}

/// 确定整体健康状态
fn determine_overall_status(components: &[&ComponentStatus]) -> String {
    let unhealthy_count = components.iter().filter(|c| c.status == "unhealthy").count();
    let warning_count = components.iter().filter(|c| c.status == "warning").count();
    
    if unhealthy_count > 0 {
        "unhealthy".to_string()
    } else if warning_count > 0 {
        "warning".to_string()
    } else {
        "healthy".to_string()
    }
}

/// 获取系统运行时间
fn get_uptime_seconds() -> u64 {
    // 简化实现，实际应该跟踪应用启动时间
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        let health = response.0;
        
        assert_eq!(health["status"], "ok");
        assert_eq!(health["service"], "embedded-task-scheduler");
        assert_eq!(health["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(health["mode"], "embedded");
    }

    #[test]
    fn test_determine_overall_status() {
        let healthy = ComponentStatus {
            status: "healthy".to_string(),
            message: "OK".to_string(),
            last_check: chrono::Utc::now(),
        };
        
        let warning = ComponentStatus {
            status: "warning".to_string(),
            message: "Warning".to_string(),
            last_check: chrono::Utc::now(),
        };
        
        let unhealthy = ComponentStatus {
            status: "unhealthy".to_string(),
            message: "Error".to_string(),
            last_check: chrono::Utc::now(),
        };
        
        // All healthy
        assert_eq!(determine_overall_status(&[&healthy, &healthy]), "healthy");
        
        // Some warnings
        assert_eq!(determine_overall_status(&[&healthy, &warning]), "warning");
        
        // Some unhealthy
        assert_eq!(determine_overall_status(&[&healthy, &unhealthy]), "unhealthy");
        
        // Mixed
        assert_eq!(determine_overall_status(&[&healthy, &warning, &unhealthy]), "unhealthy");
    }

    #[test]
    fn test_get_uptime_seconds() {
        let uptime = get_uptime_seconds();
        assert_eq!(uptime, 0); // Placeholder implementation
    }

    #[test]
    fn test_get_system_health_info() {
        let health = get_system_health_info();
        assert!(health.memory_usage_mb.is_none());
        assert!(health.cpu_usage_percent.is_none());
        assert!(health.disk_usage_percent.is_none());
        assert_eq!(health.active_connections, 0);
    }
}
