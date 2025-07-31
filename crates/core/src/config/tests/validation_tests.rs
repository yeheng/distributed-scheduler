use crate::config::models::{
    ApiConfig, DatabaseConfig, DispatcherConfig, MessageQueueConfig,
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

    // Test empty URL
    config.url = "".to_string();
    assert!(config.validate().is_err());

    // Test invalid URL format
    config.url = "mysql://localhost/test".to_string();
    assert!(config.validate().is_err());

    // Test valid URL
    config.url = "postgresql://localhost/test".to_string();
    assert!(config.validate().is_ok());

    // Test connection count config
    config.max_connections = 0;
    assert!(config.validate().is_err());

    config.max_connections = 10;
    config.min_connections = 15;
    assert!(config.validate().is_err());
}

#[test]
fn test_message_queue_validation() {
    let mut config = MessageQueueConfig::default();

    // Test empty URL
    config.url = "".to_string();
    assert!(config.validate().is_err());

    // Test invalid URL format
    config.url = "http://localhost".to_string();
    assert!(config.validate().is_err());

    // Test valid URL
    config.url = "amqp://localhost:5672".to_string();
    assert!(config.validate().is_ok());

    // Test empty queue name
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

    // Test schedule interval
    config.schedule_interval_seconds = 0;
    assert!(config.validate().is_err());

    // Test concurrent count
    config.schedule_interval_seconds = 10;
    config.max_concurrent_dispatches = 0;
    assert!(config.validate().is_err());

    // Test invalid strategy
    config.max_concurrent_dispatches = 100;
    config.dispatch_strategy = "invalid_strategy".to_string();
    assert!(config.validate().is_err());

    // Test valid strategy
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

    // Test empty worker ID
    config.worker_id = "".to_string();
    assert!(config.validate().is_err());

    // Test empty hostname
    config.worker_id = "test-worker".to_string();
    config.hostname = "".to_string();
    assert!(config.validate().is_err());

    // Test invalid IP address
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
    };

    assert!(config.validate().is_ok());

    // Test empty bind address
    config.bind_address = "".to_string();
    assert!(config.validate().is_err());

    // Test invalid address format
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

    // Test invalid log level
    config.log_level = "invalid".to_string();
    assert!(config.validate().is_err());

    // Test empty metrics endpoint
    config.log_level = "info".to_string();
    config.metrics_endpoint = "".to_string();
    assert!(config.validate().is_err());

    // Test invalid metrics endpoint
    config.metrics_endpoint = "invalid".to_string();
    assert!(config.validate().is_err());
}
