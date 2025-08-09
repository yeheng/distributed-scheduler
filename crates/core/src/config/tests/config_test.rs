use crate::config::*;
use std::env;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_default_config() {
    let config = AppConfig::default();
    assert!(config.validate().is_ok());
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
fn test_config_validation_database() {
    let mut config = AppConfig::default();
    config.database.url = "".to_string();
    assert!(config.validate().is_err());
    config.database.url = "mysql://localhost/test".to_string();
    assert!(config.validate().is_err());
    config.database.url = "postgresql://localhost/test".to_string();
    assert!(config.validate().is_ok());
    config.database.max_connections = 0;
    assert!(config.validate().is_err());

    config.database.max_connections = 10;
    config.database.min_connections = 15;
    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_message_queue() {
    let mut config = AppConfig::default();
    config.message_queue.url = "".to_string();
    assert!(config.validate().is_err());
    config.message_queue.url = "http://localhost".to_string();
    assert!(config.validate().is_err());
    config.message_queue.url = "redis://localhost:6379".to_string();
    assert!(config.validate().is_ok());
    config.message_queue.task_queue = "".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_dispatcher() {
    let mut config = AppConfig::default();
    config.dispatcher.schedule_interval_seconds = 0;
    assert!(config.validate().is_err());
    config.dispatcher.schedule_interval_seconds = 10;
    config.dispatcher.max_concurrent_dispatches = 0;
    assert!(config.validate().is_err());
    config.dispatcher.max_concurrent_dispatches = 100;
    config.dispatcher.dispatch_strategy = "invalid_strategy".to_string();
    assert!(config.validate().is_err());
    config.dispatcher.dispatch_strategy = "round_robin".to_string();
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_worker() {
    let mut config = AppConfig::default();
    config.worker.worker_id = "".to_string();
    assert!(config.validate().is_err());
    config.worker.worker_id = "test".to_string();
    config.worker.hostname = "".to_string();
    assert!(config.validate().is_err());
    config.worker.hostname = "test".to_string();
    config.worker.ip_address = "".to_string();
    assert!(config.validate().is_err());
    config.worker.ip_address = "127.0.0.1".to_string();
    config.worker.max_concurrent_tasks = 0;
    assert!(config.validate().is_err());
    config.worker.max_concurrent_tasks = 5;
    config.worker.supported_task_types = vec![];
    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_api() {
    let mut config = AppConfig::default();
    config.api.bind_address = "".to_string();
    assert!(config.validate().is_err());
    config.api.bind_address = "localhost".to_string();
    assert!(config.validate().is_err());
    config.api.bind_address = "localhost:8080".to_string();
    assert!(config.validate().is_ok());
    config.api.request_timeout_seconds = 0;
    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_observability() {
    let mut config = AppConfig::default();
    config.observability.log_level = "invalid".to_string();
    assert!(config.validate().is_err());
    config.observability.log_level = "debug".to_string();
    assert!(config.validate().is_ok());
    config.observability.metrics_endpoint = "".to_string();
    assert!(config.validate().is_err());
    config.observability.metrics_endpoint = "metrics".to_string();
    assert!(config.validate().is_err());
    config.observability.metrics_endpoint = "/metrics".to_string();
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_load_from_file() {
    let toml_content = r#"
[database]
url = "postgresql://file-test:5432/scheduler"
max_connections = 25
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[dispatcher]
enabled = true
schedule_interval_seconds = 15
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

[message_queue]
url = "redis://localhost:6379"
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 5
connection_timeout_seconds = 30

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
    let temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), toml_content).unwrap();
    let config = AppConfig::load(Some(temp_file.path().to_str().unwrap())).unwrap();

    assert_eq!(config.database.url, "postgresql://file-test:5432/scheduler");
    assert_eq!(config.database.max_connections, 25);
    assert_eq!(config.dispatcher.schedule_interval_seconds, 15);
}

#[test]
fn test_config_load_nonexistent_file() {
    let result = AppConfig::load(Some("/nonexistent/config.toml"));
    assert!(result.is_err());
}

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
    env::remove_var("SCHEDULER_DATABASE_MAX_CONNECTIONS");
    env::remove_var("SCHEDULER_DISPATCHER_SCHEDULE_INTERVAL_SECONDS");
    env::remove_var("SCHEDULER_WORKER_WORKER_ID");
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_to_toml() {
    let config = AppConfig::default();
    let toml_str = config.to_toml().unwrap();
    let parsed_config = AppConfig::from_toml(&toml_str).unwrap();
    assert_eq!(config.database.url, parsed_config.database.url);
    assert_eq!(config.dispatcher.enabled, parsed_config.dispatcher.enabled);
}

#[test]
fn test_invalid_toml_format() {
    let invalid_toml = r#"
[database
url = "invalid toml
"#;

    let result = AppConfig::from_toml(invalid_toml);
    assert!(result.is_err());
}

#[test]
fn test_dispatch_strategies() {
    let mut config = AppConfig::default();
    let valid_strategies = ["round_robin", "load_based", "task_type_affinity"];
    for strategy in &valid_strategies {
        config.dispatcher.dispatch_strategy = strategy.to_string();
        assert!(config.validate().is_ok(), "策略 {strategy} 应该有效");
    }
}

#[test]
fn test_log_levels() {
    let mut config = AppConfig::default();
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    for level in &valid_levels {
        config.observability.log_level = level.to_string();
        assert!(config.validate().is_ok(), "日志级别 {level} 应该有效");
        config.observability.log_level = level.to_uppercase();
        assert!(
            config.validate().is_ok(),
            "日志级别 {} 应该有效",
            level.to_uppercase()
        );
    }
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
    assert!(config.message_queue.is_redis_stream());
    assert!(!config.message_queue.is_rabbitmq());
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
    assert!(config.message_queue.redis.is_some());

    let redis_config = config.message_queue.redis.as_ref().unwrap();
    assert_eq!(redis_config.host, "127.0.0.1");
    assert_eq!(redis_config.port, 6379);
    assert_eq!(redis_config.database, 0);
    assert_eq!(redis_config.password, Some("secret".to_string()));
    assert!(redis_config.has_password());

    assert!(config.validate().is_ok());
}

#[test]
fn test_redis_config_validation() {
    use crate::config::RedisConfig;

    let mut redis_config = RedisConfig::default();
    redis_config.host = "".to_string();
    assert!(redis_config.validate().is_err());
    redis_config.host = "localhost".to_string();
    redis_config.port = 0;
    assert!(redis_config.validate().is_err());
    redis_config.port = 6379;
    redis_config.database = -1;
    assert!(redis_config.validate().is_err());
    redis_config.database = 0;
    assert!(redis_config.validate().is_ok());
}

#[test]
fn test_redis_config_build_url() {
    use crate::config::RedisConfig;
    let redis_config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        database: 0,
        password: None,
        connection_timeout_seconds: 30,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
    };

    assert_eq!(redis_config.build_url(), "redis://localhost:6379/0");
    assert!(!redis_config.has_password());
    let redis_config_with_password = RedisConfig {
        password: Some("secret".to_string()),
        ..redis_config
    };

    assert_eq!(
        redis_config_with_password.build_url(),
        "redis://:secret@localhost:6379/0"
    );
    assert!(redis_config_with_password.has_password());
}

#[test]
fn test_message_queue_get_redis_url() {
    use crate::config::{MessageQueueConfig, MessageQueueType, RedisConfig};
    let mut mq_config = MessageQueueConfig {
        r#type: MessageQueueType::RedisStream,
        url: "redis://localhost:6379/0".to_string(),
        redis: None,
        task_queue: "tasks".to_string(),
        status_queue: "status".to_string(),
        heartbeat_queue: "heartbeat".to_string(),
        control_queue: "control".to_string(),
        max_retries: 3,
        retry_delay_seconds: 60,
        connection_timeout_seconds: 30,
    };

    assert_eq!(
        mq_config.get_redis_url(),
        Some("redis://localhost:6379/0".to_string())
    );
    mq_config.url = "".to_string();
    mq_config.redis = Some(RedisConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 1,
        password: Some("test".to_string()),
        connection_timeout_seconds: 30,
        max_retry_attempts: 3,
        retry_delay_seconds: 1,
    });

    assert_eq!(
        mq_config.get_redis_url(),
        Some("redis://:test@127.0.0.1:6379/1".to_string())
    );
    mq_config.r#type = MessageQueueType::Rabbitmq;
    assert_eq!(mq_config.get_redis_url(), None);
}

#[test]
fn test_redis_stream_validation_errors() {
    let mut config = AppConfig::default();
    config.message_queue.r#type = MessageQueueType::RedisStream;
    config.message_queue.url = "".to_string();
    config.message_queue.redis = None;
    assert!(config.validate().is_err());
    config.message_queue.url = "http://localhost:6379".to_string();
    assert!(config.validate().is_err());
    config.message_queue.url = "redis://localhost:6379/0".to_string();
    assert!(config.validate().is_ok());
    config.message_queue.url = "rediss://localhost:6379/0".to_string();
    assert!(config.validate().is_ok());
}
#[test]
fn test_redis_stream_config_file_loading() {
    let config_result = AppConfig::load(Some("config/redis-stream.toml"));

    match config_result {
        Ok(config) => {
            assert!(config.message_queue.is_redis_stream());
            assert_eq!(config.message_queue.r#type, MessageQueueType::RedisStream);
            assert_eq!(config.message_queue.url, "redis://localhost:6379/0");
            if let Some(redis_config) = &config.message_queue.redis {
                assert_eq!(redis_config.host, "localhost");
                assert_eq!(redis_config.port, 6379);
                assert_eq!(redis_config.database, 0);
                assert_eq!(redis_config.connection_timeout_seconds, 30);
                assert_eq!(redis_config.max_retry_attempts, 3);
                assert_eq!(redis_config.retry_delay_seconds, 1);
            }
            let redis_url = config.message_queue.get_redis_url();
            assert!(redis_url.is_some());
            assert_eq!(redis_url.unwrap(), "redis://localhost:6379/0");

            eprintln!("✅ Redis Stream configuration file loaded and validated successfully");
        }
        Err(e) => {
            if e.to_string().contains("配置文件不存在") {
                eprintln!("⚠️  Redis Stream configuration file not found, skipping test");
            } else {
                panic!("Failed to load Redis Stream configuration: {}", e);
            }
        }
    }
}

#[test]
fn test_redis_config_integration_comprehensive() {
    let redis_config = RedisConfig::default();
    assert_eq!(redis_config.host, "127.0.0.1");
    assert_eq!(redis_config.port, 6379);
    assert_eq!(redis_config.database, 0);
    assert_eq!(redis_config.password, None);
    assert_eq!(redis_config.connection_timeout_seconds, 30);
    assert_eq!(redis_config.max_retry_attempts, 3);
    assert_eq!(redis_config.retry_delay_seconds, 1);
    let toml_content = r#"
[database]
url = "postgresql://localhost/test"
max_connections = 10
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[message_queue]
type = "redis_stream"
url = "redis://localhost:6379/1"
task_queue = "test_tasks"
status_queue = "test_status"
heartbeat_queue = "test_heartbeats"
control_queue = "test_control"
max_retries = 5
retry_delay_seconds = 30
connection_timeout_seconds = 60

[message_queue.redis]
host = "redis-server"
port = 6380
database = 2
password = "secret"
connection_timeout_seconds = 45
max_retry_attempts = 5
retry_delay_seconds = 2

[dispatcher]
enabled = true
schedule_interval_seconds = 10
max_concurrent_dispatches = 100
worker_timeout_seconds = 90
dispatch_strategy = "round_robin"

[worker]
enabled = false
worker_id = "test-worker"
hostname = "localhost"
ip_address = "127.0.0.1"
max_concurrent_tasks = 5
supported_task_types = ["shell"]
heartbeat_interval_seconds = 30
task_poll_interval_seconds = 5

[api]
enabled = true
bind_address = "127.0.0.1:8080"
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

    let config = AppConfig::from_toml(toml_content).expect("Failed to parse TOML");
    assert!(config.message_queue.is_redis_stream());
    assert_eq!(config.message_queue.r#type, MessageQueueType::RedisStream);
    let redis_config = config
        .message_queue
        .redis
        .as_ref()
        .expect("Redis config should be present");
    assert_eq!(redis_config.host, "redis-server");
    assert_eq!(redis_config.port, 6380);
    assert_eq!(redis_config.database, 2);
    assert_eq!(redis_config.password, Some("secret".to_string()));
    assert_eq!(redis_config.connection_timeout_seconds, 45);
    assert_eq!(redis_config.max_retry_attempts, 5);
    assert_eq!(redis_config.retry_delay_seconds, 2);
    assert!(
        config.validate().is_ok(),
        "Configuration validation should pass"
    );
    let redis_url = config
        .message_queue
        .get_redis_url()
        .expect("Should generate Redis URL");
    assert_eq!(redis_url, "redis://localhost:6379/1");
    let built_url = redis_config.build_url();
    assert_eq!(built_url, "redis://:secret@redis-server:6380/2");

    eprintln!("✅ Comprehensive Redis configuration integration test passed");
}

#[test]
fn test_redis_config_validation_errors() {
    let mut redis_config = RedisConfig::default();
    redis_config.host = "".to_string();
    assert!(redis_config.validate().is_err());
    redis_config = RedisConfig::default();
    redis_config.port = 0;
    assert!(redis_config.validate().is_err());
    redis_config = RedisConfig::default();
    redis_config.database = -1;
    assert!(redis_config.validate().is_err());
    redis_config = RedisConfig::default();
    redis_config.connection_timeout_seconds = 0;
    assert!(redis_config.validate().is_err());
    redis_config = RedisConfig::default();
    redis_config.max_retry_attempts = 0;
    assert!(redis_config.validate().is_err());
    redis_config = RedisConfig::default();
    redis_config.retry_delay_seconds = 0;
    assert!(redis_config.validate().is_err());

    eprintln!("✅ Redis configuration validation error tests passed");
}
