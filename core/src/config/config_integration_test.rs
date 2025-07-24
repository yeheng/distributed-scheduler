use crate::config::AppConfig;
use crate::config::ConfigLoader;
use std::env;

#[test]
fn test_config_loader_basic() {
    let config = ConfigLoader::load().unwrap_or_else(|_| AppConfig::default());
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_loader_environment_detection() {
    // 测试默认环境
    env::remove_var("SCHEDULER_ENV");
    assert_eq!(ConfigLoader::current_env(), "development");
    assert!(ConfigLoader::is_development());
    assert!(!ConfigLoader::is_production());

    // 测试生产环境
    env::set_var("SCHEDULER_ENV", "production");
    assert_eq!(ConfigLoader::current_env(), "production");
    assert!(!ConfigLoader::is_development());
    assert!(ConfigLoader::is_production());

    // 清理
    env::remove_var("SCHEDULER_ENV");
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
    // 测试默认环境
    env::remove_var("SCHEDULER_ENV");
    assert_eq!(ConfigLoader::current_env(), "development");

    // 测试设置环境
    env::set_var("SCHEDULER_ENV", "production");
    assert_eq!(ConfigLoader::current_env(), "production");

    // 清理
    env::remove_var("SCHEDULER_ENV");
}

#[test]
fn test_is_development() {
    env::set_var("SCHEDULER_ENV", "development");
    assert!(ConfigLoader::is_development());
    assert!(!ConfigLoader::is_production());

    env::set_var("SCHEDULER_ENV", "production");
    assert!(!ConfigLoader::is_development());
    assert!(ConfigLoader::is_production());

    // 清理
    env::remove_var("SCHEDULER_ENV");
}

#[test]
fn test_database_url_override() {
    let config = AppConfig::default();

    // 测试默认值
    env::remove_var("DATABASE_URL");
    assert_eq!(ConfigLoader::get_database_url(&config), config.database.url);

    // 测试环境变量覆盖
    let test_url = "postgresql://test:5432/test_db";
    env::set_var("DATABASE_URL", test_url);
    assert_eq!(ConfigLoader::get_database_url(&config), test_url);

    // 清理
    env::remove_var("DATABASE_URL");
}

#[test]
fn test_message_queue_url_override() {
    let config = AppConfig::default();

    // 测试默认值
    env::remove_var("RABBITMQ_URL");
    env::remove_var("AMQP_URL");
    assert_eq!(
        ConfigLoader::get_message_queue_url(&config),
        config.message_queue.url
    );

    // 测试RABBITMQ_URL覆盖
    let test_url = "amqp://test:5672";
    env::set_var("RABBITMQ_URL", test_url);
    assert_eq!(ConfigLoader::get_message_queue_url(&config), test_url);

    // 测试AMQP_URL覆盖（优先级较低）
    env::remove_var("RABBITMQ_URL");
    env::set_var("AMQP_URL", test_url);
    assert_eq!(ConfigLoader::get_message_queue_url(&config), test_url);

    // 清理
    env::remove_var("RABBITMQ_URL");
    env::remove_var("AMQP_URL");
}
