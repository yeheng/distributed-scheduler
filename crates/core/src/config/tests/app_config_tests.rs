use crate::config::models::AppConfig;

#[test]
fn test_default_config() {
    let config = AppConfig::default();
    assert!(config.validate().is_ok());

    // 验证默认值
    assert_eq!(config.database.max_connections, 10);
    assert!(config.dispatcher.enabled);
    assert!(!config.worker.enabled);
    assert!(config.api.enabled);
}

#[test]
fn test_config_from_toml() {
    let toml_content = r#"
[database]
url = "postgresql://test:5432/scheduler_test"
max_connections = 15
min_connections = 2
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[dispatcher]
enabled = true
schedule_interval_seconds = 20
max_concurrent_dispatches = 100
worker_timeout_seconds = 90
dispatch_strategy = "round_robin"

[worker]
enabled = true
worker_id = "test-worker"
hostname = "test-host"
ip_address = "192.168.1.100"
max_concurrent_tasks = 8
supported_task_types = ["shell", "http"]
heartbeat_interval_seconds = 30
task_poll_interval_seconds = 5

[api]
enabled = true
bind_address = "127.0.0.1:9090"
cors_enabled = true
cors_origins = ["*"]
request_timeout_seconds = 30
max_request_size_mb = 10

[message_queue]
url = "redis://test:6379"
task_queue = "test_tasks"
status_queue = "test_status"
heartbeat_queue = "test_heartbeats"
control_queue = "test_control"
max_retries = 3
retry_delay_seconds = 60
connection_timeout_seconds = 30

[observability]
tracing_enabled = true
metrics_enabled = true
metrics_endpoint = "/metrics"
log_level = "debug"
"#;

    let config = AppConfig::from_toml(toml_content).unwrap();

    assert_eq!(config.database.url, "postgresql://test:5432/scheduler_test");
    assert_eq!(config.database.max_connections, 15);
    assert_eq!(config.dispatcher.schedule_interval_seconds, 20);
    assert_eq!(config.worker.worker_id, "test-worker");
    assert_eq!(config.api.bind_address, "127.0.0.1:9090");
    assert_eq!(config.observability.log_level, "debug");
}

#[test]
fn test_config_to_toml() {
    let config = AppConfig::default();
    let toml_str = config.to_toml().unwrap();
    
    // Should be valid TOML that can be parsed back
    let parsed_config = AppConfig::from_toml(&toml_str).unwrap();
    assert_eq!(config.database.url, parsed_config.database.url);
    assert_eq!(config.dispatcher.enabled, parsed_config.dispatcher.enabled);
}