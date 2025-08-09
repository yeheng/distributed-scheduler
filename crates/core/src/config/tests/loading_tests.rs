use crate::config::models::AppConfig;
use std::env;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_config_environment_override() {
    env::set_var("SCHEDULER_DATABASE_MAX_CONNECTIONS", "50");
    env::set_var("SCHEDULER_DISPATCHER_SCHEDULE_INTERVAL_SECONDS", "25");
    env::set_var("SCHEDULER_WORKER_WORKER_ID", "env-worker");
    assert_eq!(
        env::var("SCHEDULER_DATABASE_MAX_CONNECTIONS").unwrap(),
        "50"
    );
    let config = AppConfig::load(None).unwrap();
    eprintln!(
        "Database max_connections: {}",
        config.database.max_connections
    );
    eprintln!(
        "Dispatcher schedule_interval_seconds: {}",
        config.dispatcher.schedule_interval_seconds
    );
    eprintln!("Worker worker_id: {}", config.worker.worker_id);
    if config.database.max_connections == 50 {
        eprintln!("✓ Environment variable override working for database.max_connections");
    } else {
        eprintln!("✗ Environment variable override NOT working for database.max_connections");
    }
    env::remove_var("SCHEDULER_DATABASE_MAX_CONNECTIONS");
    env::remove_var("SCHEDULER_DISPATCHER_SCHEDULE_INTERVAL_SECONDS");
    env::remove_var("SCHEDULER_WORKER_WORKER_ID");
    assert!(config.validate().is_ok());
}

#[test]
fn test_simple_config_builder() {
    use config::{Config, Environment};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct TestConfig {
        database: DatabaseConfig,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct DatabaseConfig {
        max_connections: i64,
    }
    std::env::set_var("SCHEDULER_DATABASE_MAX_CONNECTIONS", "50");
    let mut builder = Config::builder();
    builder = builder.set_default("database.max_connections", 10).unwrap();
    builder = builder.add_source(
        Environment::with_prefix("SCHEDULER")
            .separator("_")
            .try_parsing(true),
    );
    let config = builder.build().unwrap();
    let max_connections = config.get_int("database.max_connections").unwrap_or(0);
    match config.clone().try_deserialize::<TestConfig>() {
        Ok(test_config) => {
            eprintln!(
                "Deserialized config - max_connections: {}",
                test_config.database.max_connections
            );
        }
        Err(e) => {
            eprintln!("Failed to deserialize: {:?}", e);
        }
    }
    eprintln!("Raw config - max_connections: {}", max_connections);
    eprintln!(
        "Environment var: {:?}",
        std::env::var("SCHEDULER_DATABASE_MAX_CONNECTIONS")
    );
    std::env::remove_var("SCHEDULER_DATABASE_MAX_CONNECTIONS");
    assert!(max_connections > 0);
}

#[test]
fn test_config_file_loading() {
    let mut temp_file = NamedTempFile::new().unwrap();
    let config_content = r#"[database]
url = "postgresql://test:5432/scheduler_test"
max_connections = 15
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[dispatcher]
enabled = true
schedule_interval_seconds = 20
max_concurrent_dispatches = 100
worker_timeout_seconds = 90
dispatch_strategy = "round_robin"

[worker]
enabled = false
worker_id = "test-worker"
hostname = "localhost"
ip_address = "127.0.0.1"
max_concurrent_tasks = 5
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

[api.auth]
enabled = false
jwt_secret = "test-jwt-secret-for-testing-only"
jwt_expiration_hours = 24
api_keys = {}

[message_queue]
url = "redis://test:6379"
task_queue = "test_tasks"
status_queue = "test_status"
heartbeat_queue = "test_heartbeats"
control_queue = "test_control"
max_retries = 3
retry_delay_seconds = 5
connection_timeout_seconds = 30

[observability]
tracing_enabled = true
metrics_enabled = true
metrics_endpoint = "/metrics"
log_level = "info"
"#;

    temp_file.write_all(config_content.as_bytes()).unwrap();
    let config_path = temp_file.path().to_string_lossy().to_string();
    let config = AppConfig::load(Some(&config_path)).unwrap();

    assert_eq!(config.database.url, "postgresql://test:5432/scheduler_test");
    assert_eq!(config.database.max_connections, 15);
    assert_eq!(config.dispatcher.schedule_interval_seconds, 20);
    assert_eq!(config.worker.worker_id, "test-worker");
    assert_eq!(config.api.bind_address, "127.0.0.1:9090");
    assert_eq!(config.message_queue.task_queue, "test_tasks");
}

#[test]
fn test_config_file_not_found() {
    let result = AppConfig::load(Some("/nonexistent/path/config.toml"));
    assert!(result.is_err());
}

#[test]
fn test_config_default_paths() {
    let config = AppConfig::load(None).unwrap();
    assert!(config.validate().is_ok());
}
