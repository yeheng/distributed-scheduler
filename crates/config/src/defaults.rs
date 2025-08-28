use figment::value::Value;
use serde_json::json;

/// 系统默认配置
///
/// 这个文件包含所有配置项的默认值，用于在没有配置文件或环境变量时提供回退值
pub fn default_config() -> Value {
    let json_value = json!({
        // 数据库配置
        "database": {
            "url": "postgresql://localhost/scheduler",
            "max_connections": 10,
            "min_connections": 1,
            "connection_timeout_seconds": 30,
            "idle_timeout_seconds": 600
        },

        // 消息队列配置
        "message_queue": {
            "type": "RedisStream",
            "url": "redis://localhost:6379",
            "redis": {
                "host": "127.0.0.1",
                "port": 6379,
                "database": 0,
                "password": null,
                "connection_timeout_seconds": 30,
                "max_retry_attempts": 3,
                "retry_delay_seconds": 5
            },
            "task_queue": "tasks",
            "status_queue": "status_updates",
            "heartbeat_queue": "heartbeats",
            "control_queue": "control",
            "max_retries": 3,
            "retry_delay_seconds": 5,
            "connection_timeout_seconds": 30
        },

        // 调度器配置
        "dispatcher": {
            "enabled": true,
            "schedule_interval_seconds": 10,
            "max_concurrent_dispatches": 100,
            "worker_timeout_seconds": 90,
            "dispatch_strategy": "round_robin"
        },

        // Worker配置
        "worker": {
            "enabled": false,
            "worker_id": "worker-001",
            "hostname": "localhost",
            "ip_address": "127.0.0.1",
            "max_concurrent_tasks": 5,
            "heartbeat_interval_seconds": 30,
            "task_poll_interval_seconds": 5,
            "supported_task_types": ["shell", "http"]
        },

        // API服务配置
        "api": {
            "enabled": true,
            "bind_address": "0.0.0.0:8080",
            "cors_enabled": true,
            "cors_origins": ["*"],
            "request_timeout_seconds": 30,
            "max_request_size_mb": 10,
            "auth": {
                "enabled": false,
                "jwt_secret": "your-secret-key-change-this-in-production",
                "jwt_expiration_hours": 24,
                "api_keys": {
                    "default": {
                        "name": "Default API Key",
                        "permissions": ["TaskRead", "TaskWrite"],
                        "is_active": true
                    }
                }
            },
            "rate_limiting": {
                "enabled": false,
                "max_requests_per_minute": 60,
                "refill_rate_per_second": 1.0,
                "burst_size": 10,
                "endpoint_limits": {
                    "/api/tasks": {
                        "max_requests_per_minute": 100,
                        "burst_size": 10
                    }
                }
            }
        },

        // 可观测性配置
        "observability": {
            "tracing_enabled": true,
            "metrics_enabled": true,
            "metrics_endpoint": "/metrics",
            "log_level": "info",
            "jaeger_endpoint": null
        },

        // 执行器配置
        "executor": {
            "enabled": true,
            "executors": {
                "shell": {
                    "executor_type": "shell",
                    "config": null,
                    "supported_task_types": ["shell"],
                    "priority": 100,
                    "max_concurrent_tasks": 10,
                    "timeout_seconds": 300,
                    "retry_config": {
                        "max_retries": 3,
                        "base_delay_ms": 1000,
                        "max_delay_ms": 30000,
                        "backoff_multiplier": 2.0,
                        "jitter_enabled": true
                    }
                },
                "http": {
                    "executor_type": "http",
                    "config": null,
                    "supported_task_types": ["http"],
                    "priority": 90,
                    "max_concurrent_tasks": 20,
                    "timeout_seconds": 60,
                    "retry_config": {
                        "max_retries": 3,
                        "base_delay_ms": 1000,
                        "max_delay_ms": 30000,
                        "backoff_multiplier": 2.0,
                        "jitter_enabled": true
                    }
                }
            },
            "default_executor": "shell",
            "executor_factory": {
                "auto_discovery": false,
                "discovery_path": "./executors",
                "dynamic_loading": false,
                "plugin_directories": ["./plugins"],
                "validation_enabled": true
            }
        },

        // 弹性配置
        "resilience": {
            "database_circuit_breaker": {
                "failure_threshold": 5,
                "recovery_timeout": 60,
                "success_threshold": 3,
                "call_timeout": 30,
                "backoff_multiplier": 2.0,
                "max_recovery_timeout": 300
            },
            "message_queue_circuit_breaker": {
                "failure_threshold": 3,
                "recovery_timeout": 30,
                "success_threshold": 2,
                "call_timeout": 10,
                "backoff_multiplier": 1.5,
                "max_recovery_timeout": 180
            },
            "external_service_circuit_breaker": {
                "failure_threshold": 3,
                "recovery_timeout": 45,
                "success_threshold": 3,
                "call_timeout": 15,
                "backoff_multiplier": 2.0,
                "max_recovery_timeout": 240
            }
        }
    });

    // 转换为figment Value
    figment::value::Value::serialize(&json_value).unwrap()
}

/// 环境特定的配置覆盖
pub fn environment_overrides(env: &str) -> Option<Value> {
    let json_override = match env {
        "development" => Some(json!({
            "database": {
                "url": "postgresql://localhost/scheduler_dev"
            },
            "observability": {
                "log_level": "debug"
            },
            "api": {
                "cors_origins": ["http://localhost:3000", "http://localhost:8000"]
            }
        })),

        "production" => Some(json!({
            "worker": {
                "max_concurrent_tasks": 10
            },
            "dispatcher": {
                "max_concurrent_dispatches": 200
            },
            "observability": {
                "log_level": "warn"
            },
            "api": {
                "auth": {
                    "enabled": true
                },
                "rate_limiting": {
                    "enabled": true,
                    "requests_per_minute": 100
                }
            }
        })),

        "test" => Some(json!({
            "database": {
                "url": "postgresql://localhost/scheduler_test"
            },
            "message_queue": {
                "url": "redis://localhost:6379/1"
            },
            "observability": {
                "log_level": "debug",
                "tracing_enabled": false
            }
        })),

        _ => None,
    };

    json_override.map(|json_val| figment::value::Value::serialize(&json_val).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_structure() {
        let config = default_config();
        assert!(matches!(config, Value::Dict(..)));

        // 验证主要配置段存在
        if let Value::Dict(_, dict) = &config {
            assert!(dict.contains_key("database"));
            assert!(dict.contains_key("message_queue"));
            assert!(dict.contains_key("dispatcher"));
            assert!(dict.contains_key("worker"));
            assert!(dict.contains_key("api"));
            assert!(dict.contains_key("observability"));
            assert!(dict.contains_key("executor"));
            assert!(dict.contains_key("resilience"));
        }
    }

    #[test]
    fn test_environment_overrides() {
        let dev_overrides = environment_overrides("development");
        assert!(dev_overrides.is_some());

        let prod_overrides = environment_overrides("production");
        assert!(prod_overrides.is_some());

        let test_overrides = environment_overrides("test");
        assert!(test_overrides.is_some());

        let unknown_overrides = environment_overrides("unknown");
        assert!(unknown_overrides.is_none());
    }

    #[test]
    fn test_development_overrides_structure() {
        let overrides = environment_overrides("development").unwrap();

        // 验证开发环境的特定覆盖
        if let Value::Dict(_, dict) = &overrides {
            assert!(dict.contains_key("database"));
            assert!(dict.contains_key("observability"));
            assert!(dict.contains_key("api"));
        }
    }
}
