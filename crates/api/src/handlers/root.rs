use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub status: String,
    pub api_version: String,
    pub documentation: DocumentationLinks,
    pub endpoints: ApiEndpoints,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationLinks {
    pub api_docs: String,
    pub health_check: String,
    pub metrics: String,
    pub system_stats: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiEndpoints {
    pub tasks: String,
    pub workers: String,
    pub system: String,
    pub auth: String,
}

/// 根路径处理器 - 返回系统状态和文档链接
pub async fn root_handler() -> Json<SystemInfo> {
    Json(SystemInfo {
        name: "嵌入式任务调度系统".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "轻量级、零配置的任务调度系统".to_string(),
        status: "running".to_string(),
        api_version: "v1".to_string(),
        documentation: DocumentationLinks {
            api_docs: "/api/docs".to_string(),
            health_check: "/health".to_string(),
            metrics: "/api/metrics".to_string(),
            system_stats: "/api/system/stats".to_string(),
        },
        endpoints: ApiEndpoints {
            tasks: "/api/tasks".to_string(),
            workers: "/api/workers".to_string(),
            system: "/api/system".to_string(),
            auth: "/api/auth".to_string(),
        },
        timestamp: chrono::Utc::now(),
    })
}

/// API文档处理器
pub async fn api_docs_handler() -> Json<Value> {
    Json(json!({
        "title": "嵌入式任务调度系统 API",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "RESTful API for managing tasks, workers, and system operations",
        "base_url": "/api",
        "endpoints": {
            "tasks": {
                "GET /api/tasks": "列出所有任务",
                "POST /api/tasks": "创建新任务",
                "GET /api/tasks/{id}": "获取任务详情",
                "PUT /api/tasks/{id}": "更新任务",
                "DELETE /api/tasks/{id}": "删除任务",
                "POST /api/tasks/{id}/trigger": "手动触发任务"
            },
            "workers": {
                "GET /api/workers": "列出所有Worker",
                "GET /api/workers/{id}": "获取Worker详情",
                "GET /api/workers/{id}/stats": "获取Worker统计信息"
            },
            "system": {
                "GET /api/system/stats": "获取系统统计信息",
                "GET /api/system/health": "获取系统健康状态",
                "GET /api/metrics": "获取系统指标"
            },
            "monitoring": {
                "GET /health": "健康检查",
                "GET /api/metrics": "Prometheus格式指标"
            }
        },
        "authentication": {
            "note": "嵌入式版本默认关闭认证",
            "endpoints": {
                "POST /api/auth/login": "用户登录（如果启用认证）",
                "POST /api/auth/logout": "用户登出",
                "GET /api/auth/validate": "验证令牌"
            }
        },
        "examples": {
            "create_task": {
                "method": "POST",
                "url": "/api/tasks",
                "body": {
                    "name": "example-task",
                    "task_type": "shell",
                    "schedule": "0 */5 * * * *",
                    "parameters": {
                        "command": "echo 'Hello World'"
                    },
                    "timeout_seconds": 300,
                    "max_retries": 3
                }
            },
            "trigger_task": {
                "method": "POST",
                "url": "/api/tasks/1/trigger",
                "description": "立即执行指定任务"
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_root_handler() {
        let response = root_handler().await;
        let system_info = response.0;
        
        assert_eq!(system_info.name, "嵌入式任务调度系统");
        assert_eq!(system_info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(system_info.status, "running");
        assert_eq!(system_info.api_version, "v1");
        assert_eq!(system_info.documentation.health_check, "/health");
        assert_eq!(system_info.endpoints.tasks, "/api/tasks");
    }

    #[tokio::test]
    async fn test_api_docs_handler() {
        let response = api_docs_handler().await;
        let docs = response.0;
        
        assert!(docs.get("title").is_some());
        assert!(docs.get("version").is_some());
        assert!(docs.get("endpoints").is_some());
        assert!(docs.get("examples").is_some());
    }

    #[test]
    fn test_system_info_serialization() {
        let system_info = SystemInfo {
            name: "Test System".to_string(),
            version: "1.0.0".to_string(),
            description: "Test Description".to_string(),
            status: "running".to_string(),
            api_version: "v1".to_string(),
            documentation: DocumentationLinks {
                api_docs: "/api/docs".to_string(),
                health_check: "/health".to_string(),
                metrics: "/api/metrics".to_string(),
                system_stats: "/api/system/stats".to_string(),
            },
            endpoints: ApiEndpoints {
                tasks: "/api/tasks".to_string(),
                workers: "/api/workers".to_string(),
                system: "/api/system".to_string(),
                auth: "/api/auth".to_string(),
            },
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&system_info).unwrap();
        assert!(json.contains("Test System"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("running"));
    }
}