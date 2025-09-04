use anyhow::Result;
use scheduler_config::{AppConfig, MessageQueueType};
use scheduler_core::task_types::SHELL;
use std::env;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, timeout};
use tracing_test::traced_test;

use crate::embedded::{EmbeddedApplication, EmbeddedApplicationHandle};

/// 测试完全零配置启动
#[tokio::test]
#[traced_test]
async fn test_complete_zero_configuration_startup() -> Result<()> {
    // 创建临时目录模拟用户工作目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("zero_config.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 使用完全默认的嵌入式配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url; // 只修改数据库路径避免冲突
    config.api.bind_address = "127.0.0.1:0".to_string(); // 使用随机端口

    // 验证默认配置符合零配置要求
    verify_zero_config_defaults(&config);

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = timeout(Duration::from_secs(10), app.start()).await??;

    // 验证应用成功启动
    assert!(!app_handle.api_address().is_empty());

    // 验证数据库自动创建
    assert!(db_path.exists());

    // 验证基本功能可用
    verify_basic_functionality(&app_handle).await?;

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 验证零配置默认值
fn verify_zero_config_defaults(config: &AppConfig) {
    // 验证消息队列配置
    assert_eq!(config.message_queue.r#type, MessageQueueType::InMemory);
    assert_eq!(config.message_queue.task_queue, "tasks");
    assert_eq!(config.message_queue.status_queue, "status_updates");
    assert_eq!(config.message_queue.heartbeat_queue, "heartbeats");
    assert_eq!(config.message_queue.control_queue, "control");
    assert_eq!(config.message_queue.max_retries, 3);
    assert_eq!(config.message_queue.retry_delay_seconds, 5);

    // 验证API配置
    assert!(config.api.enabled);
    assert!(config.api.cors_enabled);
    assert_eq!(config.api.cors_origins, vec!["*"]);
    assert!(!config.api.auth.enabled); // 认证默认关闭
    assert!(!config.api.rate_limiting.enabled); // 限流默认关闭
    assert_eq!(config.api.request_timeout_seconds, 30);
    assert_eq!(config.api.max_request_size_mb, 10);

    // 验证调度器配置
    assert!(config.dispatcher.enabled);
    assert_eq!(config.dispatcher.schedule_interval_seconds, 10);
    assert_eq!(config.dispatcher.max_concurrent_dispatches, 50);
    assert_eq!(config.dispatcher.worker_timeout_seconds, 90);
    assert_eq!(config.dispatcher.dispatch_strategy, "round_robin");

    // 验证Worker配置
    assert!(config.worker.enabled);
    assert_eq!(config.worker.worker_id, "embedded-worker");
    assert_eq!(config.worker.hostname, "localhost");
    assert_eq!(config.worker.ip_address, "127.0.0.1");
    assert_eq!(config.worker.max_concurrent_tasks, 10);
    assert_eq!(config.worker.heartbeat_interval_seconds, 30);
    assert_eq!(config.worker.task_poll_interval_seconds, 5);
    assert!(config.worker.supported_task_types.contains(&"shell".to_string()));
    assert!(config.worker.supported_task_types.contains(&"http".to_string()));

    // 验证数据库配置
    assert_eq!(config.database.max_connections, 5);
    assert_eq!(config.database.min_connections, 1);
    assert_eq!(config.database.connection_timeout_seconds, 30);
    assert_eq!(config.database.idle_timeout_seconds, 600);

    // 验证可观测性配置
    assert!(config.observability.tracing_enabled);
    assert!(config.observability.metrics_enabled);
    assert_eq!(config.observability.metrics_endpoint, "/metrics");
    assert_eq!(config.observability.log_level, "info");
    assert!(config.observability.jaeger_endpoint.is_none());
}

/// 验证基本功能可用性
async fn verify_basic_functionality(app_handle: &EmbeddedApplicationHandle) -> Result<()> {
    let service_locator = app_handle.service_locator();

    // 验证数据库仓库可用
    let task_repo = service_locator.task_repository().await?;
    let _task_run_repo = service_locator.task_run_repository().await?;
    let _worker_repo = service_locator.worker_repository().await?;

    // 验证消息队列可用
    let _message_queue = service_locator.message_queue().await?;

    // 创建测试任务验证数据库功能
    let task = scheduler_domain::entities::Task {
        id: 0,
        name: "zero_config_test_task".to_string(),
        task_type: SHELL.to_string(),
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'zero config test'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: scheduler_domain::entities::TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_task = task_repo.create(&task).await?;
    assert!(created_task.id > 0);

    // 验证任务查询功能
    let found_task = task_repo.get_by_id(created_task.id).await?;
    assert!(found_task.is_some());

    Ok(())
}

/// 测试环境变量配置覆盖
#[tokio::test]
#[traced_test]
async fn test_environment_variable_configuration_override() -> Result<()> {
    // 设置环境变量
    env::set_var("SCHEDULER_API_BIND_ADDRESS", "127.0.0.1:19080");
    env::set_var("SCHEDULER_WORKER_MAX_CONCURRENT_TASKS", "20");
    env::set_var("SCHEDULER_DATABASE_MAX_CONNECTIONS", "8");
    env::set_var("SCHEDULER_LOG_LEVEL", "debug");

    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("env_override_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 使用环境变量配置
    let mut config = AppConfig::embedded_with_env()?;
    config.database.url = db_url; // 只修改数据库路径

    // 验证环境变量覆盖生效
    assert_eq!(config.api.bind_address, "127.0.0.1:19080");
    assert_eq!(config.worker.max_concurrent_tasks, 20);
    assert_eq!(config.database.max_connections, 8);
    assert_eq!(config.observability.log_level, "debug");

    // 启动应用验证配置有效
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 验证API地址使用了环境变量设置
    assert_eq!(app_handle.api_address(), "127.0.0.1:19080");

    // 关闭应用
    app_handle.shutdown().await?;

    // 清理环境变量
    env::remove_var("SCHEDULER_API_BIND_ADDRESS");
    env::remove_var("SCHEDULER_WORKER_MAX_CONCURRENT_TASKS");
    env::remove_var("SCHEDULER_DATABASE_MAX_CONNECTIONS");
    env::remove_var("SCHEDULER_LOG_LEVEL");

    Ok(())
}

/// 测试默认数据库路径创建
#[tokio::test]
#[traced_test]
async fn test_default_database_path_creation() -> Result<()> {
    // 创建临时目录作为工作目录
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    // 设置工作目录
    let original_dir = env::current_dir()?;
    env::set_current_dir(work_dir)?;

    // 使用默认配置（数据库路径为 "sqlite:scheduler.db"）
    let config = AppConfig::embedded_default();
    
    // 验证默认数据库路径
    assert_eq!(config.database.url, "sqlite:scheduler.db");

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 验证数据库文件在当前工作目录创建
    let db_file = work_dir.join("scheduler.db");
    assert!(db_file.exists());

    // 关闭应用
    app_handle.shutdown().await?;

    // 恢复原始工作目录
    env::set_current_dir(original_dir)?;

    Ok(())
}

/// 测试快速启动时间
#[tokio::test]
#[traced_test]
async fn test_fast_startup_time() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("fast_startup_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 测量启动时间
    let start_time = std::time::Instant::now();

    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    let startup_duration = start_time.elapsed();

    // 验证启动时间合理（应该在5秒内完成）
    assert!(startup_duration < Duration::from_secs(5), 
           "Startup took too long: {:?}", startup_duration);

    // 验证应用正常工作
    assert!(!app_handle.api_address().is_empty());

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试内存使用优化
#[tokio::test]
#[traced_test]
async fn test_memory_usage_optimization() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("memory_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置，使用较小的连接池
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.database.max_connections = 2; // 减少连接数
    config.database.min_connections = 1;
    config.worker.max_concurrent_tasks = 5; // 减少并发任务数
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 等待应用稳定
    sleep(Duration::from_millis(500)).await;

    // 验证应用正常工作
    let service_locator = app_handle.service_locator();
    let task_repo = service_locator.task_repository().await?;

    // 创建少量任务测试功能
    for i in 0..3 {
        let task = scheduler_domain::entities::Task {
            id: 0,
            name: format!("memory_test_task_{}", i),
            task_type: SHELL.to_string(),
            schedule: "0 0 * * *".to_string(),
            parameters: serde_json::json!({"command": format!("echo 'memory test {}'", i)}),
            timeout_seconds: 300,
            max_retries: 3,
            status: scheduler_domain::entities::TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let created_task = task_repo.create(&task).await?;
        assert!(created_task.id > 0);
    }

    // 验证任务创建成功
    let all_tasks = task_repo.find_all().await?;
    let memory_test_tasks: Vec<_> = all_tasks
        .into_iter()
        .filter(|t| t.name.starts_with("memory_test_task_"))
        .collect();

    assert_eq!(memory_test_tasks.len(), 3);

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试配置验证和错误处理
#[tokio::test]
#[traced_test]
async fn test_configuration_validation_and_error_handling() -> Result<()> {
    // 测试无效的数据库URL
    let mut invalid_config = AppConfig::embedded_default();
    invalid_config.database.url = "invalid://database/url".to_string();
    invalid_config.api.bind_address = "127.0.0.1:0".to_string();

    let app_result = EmbeddedApplication::new(invalid_config).await;
    // 注意：应用创建可能成功，但启动时会失败
    if let Ok(app) = app_result {
        let start_result = app.start().await;
        assert!(start_result.is_err()); // 启动应该失败
    }

    // 测试无效的端口配置
    let mut invalid_port_config = AppConfig::embedded_default();
    invalid_port_config.api.bind_address = "127.0.0.1:99999".to_string(); // 无效端口
    
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("invalid_port_test.db");
    invalid_port_config.database.url = format!("sqlite:{}", db_path.display());

    let app = EmbeddedApplication::new(invalid_port_config).await?;
    let start_result = app.start().await;
    assert!(start_result.is_err()); // 启动应该失败

    Ok(())
}

/// 测试多实例启动冲突处理
#[tokio::test]
#[traced_test]
async fn test_multiple_instance_conflict_handling() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("multi_instance_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建第一个实例
    let mut config1 = AppConfig::embedded_default();
    config1.database.url = db_url.clone();
    config1.api.bind_address = "127.0.0.1:19081".to_string();

    let app1 = EmbeddedApplication::new(config1).await?;
    let app_handle1 = app1.start().await?;

    // 等待第一个实例完全启动
    sleep(Duration::from_millis(200)).await;

    // 尝试创建第二个实例使用相同端口（应该失败）
    let mut config2 = AppConfig::embedded_default();
    config2.database.url = db_url;
    config2.api.bind_address = "127.0.0.1:19081".to_string(); // 相同端口

    let app2 = EmbeddedApplication::new(config2).await?;
    let start_result2 = app2.start().await;
    assert!(start_result2.is_err()); // 第二个实例启动应该失败

    // 关闭第一个实例
    app_handle1.shutdown().await?;

    Ok(())
}

/// 测试资源清理和重启
#[tokio::test]
#[traced_test]
async fn test_resource_cleanup_and_restart() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("cleanup_restart_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 第一次启动
    {
        let mut config = AppConfig::embedded_default();
        config.database.url = db_url.clone();
        config.api.bind_address = "127.0.0.1:0".to_string();

        let app = EmbeddedApplication::new(config).await?;
        let app_handle = app.start().await?;

        // 创建一些数据
        let service_locator = app_handle.service_locator();
        let task_repo = service_locator.task_repository().await?;

        let task = scheduler_domain::entities::Task {
            id: 0,
            name: "cleanup_test_task".to_string(),
            task_type: SHELL.to_string(),
            schedule: "0 0 * * *".to_string(),
            parameters: serde_json::json!({"command": "echo 'cleanup test'"}),
            timeout_seconds: 300,
            max_retries: 3,
            status: scheduler_domain::entities::TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let created_task = task_repo.create(&task).await?;
        assert!(created_task.id > 0);

        // 正常关闭
        app_handle.shutdown().await?;
    }

    // 验证数据库文件仍然存在
    assert!(db_path.exists());

    // 第二次启动（重启）
    {
        let mut config = AppConfig::embedded_default();
        config.database.url = db_url;
        config.api.bind_address = "127.0.0.1:0".to_string();

        let app = EmbeddedApplication::new(config).await?;
        let app_handle = app.start().await?;

        // 验证数据已恢复
        let service_locator = app_handle.service_locator();
        let task_repo = service_locator.task_repository().await?;

        let found_task = task_repo.get_by_name("cleanup_test_task").await?;
        assert!(found_task.is_some());
        assert_eq!(found_task.unwrap().name, "cleanup_test_task");

        // 关闭应用
        app_handle.shutdown().await?;
    }

    Ok(())
}