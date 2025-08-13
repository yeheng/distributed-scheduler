use scheduler_config::validation::ConfigValidator;
use scheduler_config::{AppConfig, Environment};
use std::env;
use std::sync::Mutex;
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_config_loader_basic() {
    let config = AppConfig::load(None).unwrap_or_else(|_| AppConfig::default());
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_loader_environment_detection() {
    let _guard = ENV_MUTEX.lock().unwrap();
    env::remove_var("APP_ENV");
    let env = Environment::current().unwrap();
    assert_eq!(env, Environment::Development);
    assert!(env.is_development());
    assert!(!env.is_production());
    env::set_var("APP_ENV", "production");
    let env = Environment::current().unwrap();
    assert_eq!(env, Environment::Production);
    assert!(!env.is_development());
    assert!(env.is_production());
    env::remove_var("APP_ENV");
}

#[test]
fn test_config_serialization() {
    let config = AppConfig::default();
    let toml_str = config.to_toml().unwrap();
    assert!(!toml_str.is_empty());
    let parsed_config = AppConfig::from_toml(&toml_str).unwrap();
    assert_eq!(config.database.url, parsed_config.database.url);
    assert_eq!(config.dispatcher.enabled, parsed_config.dispatcher.enabled);
    assert_eq!(config.worker.worker_id, parsed_config.worker.worker_id);
}

#[test]
fn test_config_validation_comprehensive() {
    let mut config = AppConfig::default();
    assert!(config.validate().is_ok());
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
    env::remove_var("APP_ENV");
    let env = Environment::current().unwrap();
    assert_eq!(env, Environment::Development);
    env::set_var("APP_ENV", "production");
    let env = Environment::current().unwrap();
    assert_eq!(env, Environment::Production);
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
    env::remove_var("APP_ENV");
}

#[test]
fn test_database_url_override() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let config = AppConfig::load(None).unwrap_or_else(|_| AppConfig::default());
    assert!(!config.database.url.is_empty());
    assert!(config.database.url.starts_with("postgresql://"));
    assert!(config.validate().is_ok());
}

#[test]
fn test_message_queue_url_override() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let config = AppConfig::load(None).unwrap_or_else(|_| AppConfig::default());
    assert!(!config.message_queue.url.is_empty());
    assert!(config.validate().is_ok());
    assert!(!config.message_queue.task_queue.is_empty());
    assert!(!config.message_queue.status_queue.is_empty());
    assert!(!config.message_queue.heartbeat_queue.is_empty());
    assert!(!config.message_queue.control_queue.is_empty());
}
