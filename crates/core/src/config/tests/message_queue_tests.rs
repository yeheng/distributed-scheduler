use crate::config::models::{AppConfig, MessageQueueConfig, RedisConfig};

#[test]
fn test_rabbitmq_config() {
    let toml_content = r#"
[database]
url = "postgresql://localhost/scheduler"
max_connections = 10
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[message_queue]
type = "rabbitmq"
url = "amqp://localhost:5672"
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 5
connection_timeout_seconds = 30

[dispatcher]
enabled = true
schedule_interval_seconds = 10
max_concurrent_dispatches = 100
worker_timeout_seconds = 90
dispatch_strategy = "round_robin"

[worker]
enabled = false
worker_id = "worker-001"
hostname = "localhost"
ip_address = "127.0.0.1"
max_concurrent_tasks = 5
supported_task_types = ["shell", "http"]
heartbeat_interval_seconds = 30
task_poll_interval_seconds = 5

[api]
enabled = true
bind_address = "0.0.0.0:8080"
cors_enabled = true
cors_origins = ["*"]
request_timeout_seconds = 30
max_request_size_mb = 10

[api.auth]
enabled = false
jwt_secret = "test-jwt-secret-for-testing-only"
jwt_expiration_hours = 24
api_keys = {}

[observability]
tracing_enabled = true
metrics_enabled = true
metrics_endpoint = "/metrics"
log_level = "info"
"#;

    let config = AppConfig::from_toml(toml_content).unwrap();
    assert!(config.message_queue.is_rabbitmq());
    assert!(!config.message_queue.is_redis_stream());
    assert_eq!(config.message_queue.url, "amqp://localhost:5672");
    assert!(config.validate().is_ok());
}

#[test]
fn test_redis_stream_config_with_url() {
    let toml_content = r#"
[database]
url = "postgresql://localhost/scheduler"
max_connections = 10
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[message_queue]
type = "redis_stream"
url = "redis://localhost:6379/0"
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 5
connection_timeout_seconds = 30

[dispatcher]
enabled = true
schedule_interval_seconds = 10
max_concurrent_dispatches = 100
worker_timeout_seconds = 90
dispatch_strategy = "round_robin"

[worker]
enabled = false
worker_id = "worker-001"
hostname = "localhost"
ip_address = "127.0.0.1"
max_concurrent_tasks = 5
supported_task_types = ["shell", "http"]
heartbeat_interval_seconds = 30
task_poll_interval_seconds = 5

[api]
enabled = true
bind_address = "0.0.0.0:8080"
cors_enabled = true
cors_origins = ["*"]
request_timeout_seconds = 30
max_request_size_mb = 10

[api.auth]
enabled = false
jwt_secret = "test-jwt-secret-for-testing-only"
jwt_expiration_hours = 24
api_keys = {}

[observability]
tracing_enabled = true
metrics_enabled = true
metrics_endpoint = "/metrics"
log_level = "info"
"#;

    let config = AppConfig::from_toml(toml_content).unwrap();
    assert!(!config.message_queue.is_rabbitmq());
    assert!(config.message_queue.is_redis_stream());
    assert_eq!(config.message_queue.url, "redis://localhost:6379/0");
    assert!(config.validate().is_ok());
}

#[test]
fn test_redis_stream_config_with_redis_config() {
    let toml_content = r#"
[database]
url = "postgresql://localhost/scheduler"
max_connections = 10
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[message_queue]
type = "redis_stream"
url = ""
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 5
connection_timeout_seconds = 30

[message_queue.redis]
host = "127.0.0.1"
port = 6379
database = 0
password = "secret"
connection_timeout_seconds = 30
max_retry_attempts = 3
retry_delay_seconds = 1

[dispatcher]
enabled = true
schedule_interval_seconds = 10
max_concurrent_dispatches = 100
worker_timeout_seconds = 90
dispatch_strategy = "round_robin"

[worker]
enabled = false
worker_id = "worker-001"
hostname = "localhost"
ip_address = "127.0.0.1"
max_concurrent_tasks = 5
supported_task_types = ["shell", "http"]
heartbeat_interval_seconds = 30
task_poll_interval_seconds = 5

[api]
enabled = true
bind_address = "0.0.0.0:8080"
cors_enabled = true
cors_origins = ["*"]
request_timeout_seconds = 30
max_request_size_mb = 10

[api.auth]
enabled = false
jwt_secret = "test-jwt-secret-for-testing-only"
jwt_expiration_hours = 24
api_keys = {}

[observability]
tracing_enabled = true
metrics_enabled = true
metrics_endpoint = "/metrics"
log_level = "info"
"#;

    let config = AppConfig::from_toml(toml_content).unwrap();
    assert!(config.message_queue.is_redis_stream());
    assert_eq!(
        config.message_queue.get_redis_url().unwrap(),
        "redis://:secret@127.0.0.1:6379/0"
    );
    assert!(config.validate().is_ok());
}

#[test]
fn test_redis_config_functionality() {
    let redis_config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        database: 0,
        password: Some("secret".to_string()),
        connection_timeout_seconds: 30,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
    };

    assert!(redis_config.has_password());
    assert_eq!(redis_config.build_url(), "redis://:secret@localhost:6379/0");
    assert!(redis_config.validate().is_ok());
    let redis_config_no_password = RedisConfig {
        password: None,
        ..redis_config
    };

    assert!(!redis_config_no_password.has_password());
    assert_eq!(
        redis_config_no_password.build_url(),
        "redis://localhost:6379/0"
    );
}

#[test]
fn test_message_queue_type_detection() {
    let mut mq_config = MessageQueueConfig::default();
    mq_config.r#type = crate::config::models::MessageQueueType::Rabbitmq;
    mq_config.url = "redis://localhost:6379".to_string();
    assert!(mq_config.is_rabbitmq());
    assert!(!mq_config.is_redis_stream());
    mq_config.r#type = crate::config::models::MessageQueueType::RedisStream;
    mq_config.url = "redis://localhost:6379".to_string();
    assert!(!mq_config.is_rabbitmq());
    assert!(mq_config.is_redis_stream());
}
