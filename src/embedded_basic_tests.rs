use anyhow::Result;
use scheduler_config::AppConfig;
use tempfile::TempDir;

/// 基础测试：验证嵌入式配置的默认值
#[test]
fn test_embedded_configuration_defaults() -> Result<()> {
    // 测试嵌入式默认配置
    let config = AppConfig::embedded_default();

    // 验证数据库配置
    assert_eq!(config.database.url, "sqlite:scheduler.db");
    assert_eq!(config.database.max_connections, 5);
    assert_eq!(config.database.min_connections, 1);

    // 验证消息队列配置
    assert_eq!(config.message_queue.r#type, scheduler_config::MessageQueueType::InMemory);
    assert_eq!(config.message_queue.task_queue, "tasks");
    assert_eq!(config.message_queue.status_queue, "status_updates");

    // 验证API配置
    assert!(config.api.enabled);
    assert_eq!(config.api.bind_address, "127.0.0.1:8080");
    assert!(!config.api.auth.enabled); // 认证默认关闭
    assert!(!config.api.rate_limiting.enabled); // 限流默认关闭

    // 验证Worker配置
    assert!(config.worker.enabled);
    assert_eq!(config.worker.worker_id, "embedded-worker");
    assert_eq!(config.worker.max_concurrent_tasks, 10);

    // 验证调度器配置
    assert!(config.dispatcher.enabled);
    assert_eq!(config.dispatcher.max_concurrent_dispatches, 50);

    Ok(())
}

/// 测试环境变量配置覆盖
#[test]
fn test_embedded_configuration_with_env() -> Result<()> {
    // 设置环境变量
    std::env::set_var("SCHEDULER_API__BIND_ADDRESS", "127.0.0.1:9090");
    std::env::set_var("SCHEDULER_WORKER__MAX_CONCURRENT_TASKS", "20");
    std::env::set_var("SCHEDULER_DATABASE__MAX_CONNECTIONS", "8");

    // 使用环境变量配置
    let config = AppConfig::embedded_with_env()?;

    // 验证环境变量覆盖生效
    assert_eq!(config.api.bind_address, "127.0.0.1:9090");
    assert_eq!(config.worker.max_concurrent_tasks, 20);
    assert_eq!(config.database.max_connections, 8);

    // 清理环境变量
    std::env::remove_var("SCHEDULER_API__BIND_ADDRESS");
    std::env::remove_var("SCHEDULER_WORKER__MAX_CONCURRENT_TASKS");
    std::env::remove_var("SCHEDULER_DATABASE__MAX_CONNECTIONS");

    Ok(())
}

/// 测试数据库路径配置
#[test]
fn test_database_path_configuration() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_scheduler.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url.clone();

    // 验证配置正确
    assert_eq!(config.database.url, db_url);
    assert!(config.database.url.starts_with("sqlite:"));

    Ok(())
}

/// 测试消息队列类型配置
#[test]
fn test_message_queue_type_configuration() -> Result<()> {
    let config = AppConfig::embedded_default();

    // 验证使用内存消息队列
    assert_eq!(config.message_queue.r#type, scheduler_config::MessageQueueType::InMemory);

    // 验证队列名称配置
    assert_eq!(config.message_queue.task_queue, "tasks");
    assert_eq!(config.message_queue.status_queue, "status_updates");
    assert_eq!(config.message_queue.heartbeat_queue, "heartbeats");
    assert_eq!(config.message_queue.control_queue, "control");

    // 验证重试配置
    assert_eq!(config.message_queue.max_retries, 3);
    assert_eq!(config.message_queue.retry_delay_seconds, 5);

    Ok(())
}

/// 测试安全配置
#[test]
fn test_security_configuration() -> Result<()> {
    let config = AppConfig::embedded_default();

    // 验证认证默认关闭
    assert!(!config.api.auth.enabled);

    // 验证限流默认关闭
    assert!(!config.api.rate_limiting.enabled);

    // 验证CORS默认启用
    assert!(config.api.cors_enabled);
    assert_eq!(config.api.cors_origins, vec!["*"]);

    Ok(())
}

/// 测试资源配置优化
#[test]
fn test_resource_optimization_configuration() -> Result<()> {
    let config = AppConfig::embedded_default();

    // 验证数据库连接池配置适合嵌入式环境
    assert_eq!(config.database.max_connections, 5); // 较小的连接池
    assert_eq!(config.database.min_connections, 1);

    // 验证调度器并发配置
    assert_eq!(config.dispatcher.max_concurrent_dispatches, 50); // 适中的并发数

    // 验证Worker配置
    assert_eq!(config.worker.max_concurrent_tasks, 10); // 适中的任务并发数

    // 验证超时配置合理
    assert_eq!(config.database.connection_timeout_seconds, 30);
    assert_eq!(config.api.request_timeout_seconds, 30);

    Ok(())
}

/// 测试可观测性配置
#[test]
fn test_observability_configuration() -> Result<()> {
    let config = AppConfig::embedded_default();

    // 验证追踪和指标启用
    assert!(config.observability.tracing_enabled);
    assert!(config.observability.metrics_enabled);

    // 验证指标端点配置
    assert_eq!(config.observability.metrics_endpoint, "/metrics");

    // 验证日志级别
    assert_eq!(config.observability.log_level, "info");

    // 验证外部追踪默认关闭（嵌入式环境）
    assert!(config.observability.jaeger_endpoint.is_none());

    Ok(())
}

/// 测试任务类型支持
#[test]
fn test_supported_task_types() -> Result<()> {
    let config = AppConfig::embedded_default();

    // 验证支持的任务类型
    let supported_types = &config.worker.supported_task_types;
    assert!(supported_types.contains(&"shell".to_string()));
    assert!(supported_types.contains(&"http".to_string()));
    assert_eq!(supported_types.len(), 2);

    Ok(())
}

/// 测试网络配置
#[test]
fn test_network_configuration() -> Result<()> {
    let config = AppConfig::embedded_default();

    // 验证API绑定地址
    assert_eq!(config.api.bind_address, "127.0.0.1:8080");

    // 验证Worker网络配置
    assert_eq!(config.worker.hostname, "localhost");
    assert_eq!(config.worker.ip_address, "127.0.0.1");

    // 验证请求大小限制
    assert_eq!(config.api.max_request_size_mb, 10);

    Ok(())
}

/// 测试时间间隔配置
#[test]
fn test_timing_configuration() -> Result<()> {
    let config = AppConfig::embedded_default();

    // 验证调度间隔
    assert_eq!(config.dispatcher.schedule_interval_seconds, 10);

    // 验证心跳间隔
    assert_eq!(config.worker.heartbeat_interval_seconds, 30);

    // 验证任务轮询间隔
    assert_eq!(config.worker.task_poll_interval_seconds, 5);

    // 验证Worker超时
    assert_eq!(config.dispatcher.worker_timeout_seconds, 90);

    Ok(())
}

/// 测试配置序列化和反序列化
#[test]
fn test_configuration_serialization() -> Result<()> {
    let config = AppConfig::embedded_default();

    // 测试序列化
    let serialized = serde_json::to_string(&config)?;
    assert!(!serialized.is_empty());

    // 测试反序列化
    let deserialized: AppConfig = serde_json::from_str(&serialized)?;

    // 验证关键配置项
    assert_eq!(deserialized.database.url, config.database.url);
    assert_eq!(deserialized.api.bind_address, config.api.bind_address);
    assert_eq!(deserialized.worker.worker_id, config.worker.worker_id);

    Ok(())
}