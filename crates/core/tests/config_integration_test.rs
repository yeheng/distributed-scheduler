use scheduler_core::config::{AppConfig, Environment};
use std::env;
use std::sync::Mutex;

// Mutex to ensure environment variable tests run serially
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_config_loader_basic() {
    let config = AppConfig::load(None).unwrap_or_else(|_| AppConfig::default());
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_loader_environment_detection() {
    let _guard = ENV_MUTEX.lock().unwrap();

    // 测试默认环境
    env::remove_var("APP_ENV");
    let env = Environment::current().unwrap();
    assert_eq!(env, Environment::Development);
    assert!(env.is_development());
    assert!(!env.is_production());

    // 测试生产环境
    env::set_var("APP_ENV", "production");
    let env = Environment::current().unwrap();
    assert_eq!(env, Environment::Production);
    assert!(!env.is_development());
    assert!(env.is_production());

    // 清理
    env::remove_var("APP_ENV");
}

#[test]
fn test_config_serialization() {
    let config = AppConfig::default();

    // 测试序列化为TOML
    let toml_str = config.to_toml().unwrap();
    assert!(!toml_str.is_empty());

    // 测试从TOML反序列化
    let parsed_config = AppConfig::from_toml(&toml_str).unwrap();

    // 验证关键字段
    assert_eq!(config.database.url, parsed_config.database.url);
    assert_eq!(config.dispatcher.enabled, parsed_config.dispatcher.enabled);
    assert_eq!(config.worker.worker_id, parsed_config.worker.worker_id);
}

#[test]
fn test_config_validation_comprehensive() {
    let mut config = AppConfig::default();

    // 测试有效配置
    assert!(config.validate().is_ok());

    // 测试各种无效配置
    config.database.url = "".to_string();
    assert!(config.validate().is_err());

    config = AppConfig::default();
    config.message_queue.task_queue = "".to_string();
    assert!(config.validate().is_err());

    config = AppConfig::default();
    config.dispatcher.dispatch_strategy = "invalid".to_string();
    assert!(config.validate().is_err());

    config = AppConfig::default();
    config.worker.max_concurrent_tasks = 0;
    assert!(config.validate().is_err());

    config = AppConfig::default();
    config.api.bind_address = "invalid".to_string();
    assert!(config.validate().is_err());

    config = AppConfig::default();
    config.observability.log_level = "invalid".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_current_env() {
    let _guard = ENV_MUTEX.lock().unwrap();

    // 测试默认环境
    env::remove_var("APP_ENV");
    let env = Environment::current().unwrap();
    assert_eq!(env, Environment::Development);

    // 测试设置环境
    env::set_var("APP_ENV", "production");
    let env = Environment::current().unwrap();
    assert_eq!(env, Environment::Production);

    // 清理
    env::remove_var("APP_ENV");
}

#[test]
fn test_is_development() {
    let _guard = ENV_MUTEX.lock().unwrap();

    env::set_var("APP_ENV", "development");
    let env = Environment::current().unwrap();
    assert!(env.is_development());
    assert!(!env.is_production());

    env::set_var("APP_ENV", "production");
    let env = Environment::current().unwrap();
    assert!(!env.is_development());
    assert!(env.is_production());

    // 清理
    env::remove_var("APP_ENV");
}

#[test]
fn test_database_url_override() {
    let _guard = ENV_MUTEX.lock().unwrap();

    let config = AppConfig::default();

    // 测试默认值
    env::remove_var("SCHEDULER_DATABASE_URL");
    assert_eq!(config.database.url, "postgresql://localhost/scheduler");

    // 测试环境变量覆盖
    let test_url = "postgresql://test:5432/test_db";
    env::set_var("SCHEDULER_DATABASE_URL", test_url);
    let config_with_env = AppConfig::load(None).unwrap_or_else(|_| AppConfig::default());
    assert_eq!(config_with_env.database.url, test_url);

    // 清理
    env::remove_var("SCHEDULER_DATABASE_URL");
}

#[test]
fn test_message_queue_url_override() {
    let _guard = ENV_MUTEX.lock().unwrap();

    let config = AppConfig::default();

    // 测试默认值
    env::remove_var("SCHEDULER_MESSAGE_QUEUE_URL");
    assert_eq!(config.message_queue.url, "amqp://localhost:5672");

    // 测试环境变量覆盖
    let test_url = "amqp://test:5672";
    env::set_var("SCHEDULER_MESSAGE_QUEUE_URL", test_url);
    let config_with_env = AppConfig::load(None).unwrap_or_else(|_| AppConfig::default());
    assert_eq!(config_with_env.message_queue.url, test_url);

    // 清理
    env::remove_var("SCHEDULER_MESSAGE_QUEUE_URL");
}
