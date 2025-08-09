use std::collections::HashMap;

use crate::config::models::{
    api_observability::AuthConfig, ApiConfig, DatabaseConfig, DispatcherConfig, MessageQueueConfig,
    ObservabilityConfig, WorkerConfig,
};

#[test]
fn test_database_validation() {
    let mut config = DatabaseConfig {
        url: "postgresql://localhost/test".to_string(),
        max_connections: 10,
        min_connections: 1,
        connection_timeout_seconds: 30,
        idle_timeout_seconds: 600,
    };

    assert!(config.validate().is_ok());
    config.url = "".to_string();
    assert!(config.validate().is_err());
    config.url = "mysql://localhost/test".to_string();
    assert!(config.validate().is_err());
    config.url = "postgresql://localhost/test".to_string();
    assert!(config.validate().is_ok());
    config.max_connections = 0;
    assert!(config.validate().is_err());

    config.max_connections = 10;
    config.min_connections = 15;
    assert!(config.validate().is_err());
}

#[test]
fn test_message_queue_validation() {
    let mut config = MessageQueueConfig::default();
    config.url = "".to_string();
    assert!(config.validate().is_err());
    config.url = "http://localhost".to_string();
    assert!(config.validate().is_err());
    config.url = "redis://localhost:6379".to_string();
    assert!(config.validate().is_ok());
    config.task_queue = "".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_dispatcher_validation() {
    let mut config = DispatcherConfig {
        enabled: true,
        schedule_interval_seconds: 10,
        max_concurrent_dispatches: 100,
        worker_timeout_seconds: 90,
        dispatch_strategy: "round_robin".to_string(),
    };

    assert!(config.validate().is_ok());
    config.schedule_interval_seconds = 0;
    assert!(config.validate().is_err());
    config.schedule_interval_seconds = 10;
    config.max_concurrent_dispatches = 0;
    assert!(config.validate().is_err());
    config.max_concurrent_dispatches = 100;
    config.dispatch_strategy = "invalid_strategy".to_string();
    assert!(config.validate().is_err());
    config.dispatch_strategy = "round_robin".to_string();
    assert!(config.validate().is_ok());
}

#[test]
fn test_worker_validation() {
    let mut config = WorkerConfig {
        enabled: true,
        worker_id: "test-worker".to_string(),
        hostname: "test-host".to_string(),
        ip_address: "192.168.1.100".to_string(),
        max_concurrent_tasks: 5,
        supported_task_types: vec!["shell".to_string(), "http".to_string()],
        heartbeat_interval_seconds: 30,
        task_poll_interval_seconds: 5,
    };

    assert!(config.validate().is_ok());
    config.worker_id = "".to_string();
    assert!(config.validate().is_err());
    config.worker_id = "test-worker".to_string();
    config.hostname = "".to_string();
    assert!(config.validate().is_err());
    config.hostname = "test-host".to_string();
    config.ip_address = "invalid-ip".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_api_validation() {
    let mut config = ApiConfig {
        enabled: true,
        bind_address: "127.0.0.1:8080".to_string(),
        cors_enabled: true,
        cors_origins: vec!["*".to_string()],
        request_timeout_seconds: 30,
        max_request_size_mb: 10,
        auth: AuthConfig {
            jwt_secret: "test-secret-that-is-at-least-32-characters-long-for-testing".to_string(),
            api_keys: HashMap::new(),
            jwt_expiration_hours: 24,
            enabled: true,
        },
    };

    assert!(config.validate().is_ok());
    config.bind_address = "".to_string();
    assert!(config.validate().is_err());
    config.bind_address = "invalid-address".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_observability_validation() {
    let mut config = ObservabilityConfig {
        tracing_enabled: true,
        metrics_enabled: true,
        metrics_endpoint: "/metrics".to_string(),
        log_level: "info".to_string(),
        jaeger_endpoint: None,
    };

    assert!(config.validate().is_ok());
    config.log_level = "invalid".to_string();
    assert!(config.validate().is_err());
    config.log_level = "info".to_string();
    config.metrics_endpoint = "".to_string();
    assert!(config.validate().is_err());
    config.metrics_endpoint = "invalid".to_string();
    assert!(config.validate().is_err());
}
