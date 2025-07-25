use scheduler_core::config::{AppConfig, ConfigLoader};
use std::env;

fn main() -> anyhow::Result<()> {
    println!("=== 分布式任务调度系统配置管理示例 ===\n");

    // 1. 加载默认配置
    println!("1. 加载默认配置:");
    let default_config = AppConfig::default();
    println!("   数据库URL: {}", default_config.database.url);
    println!("   最大连接数: {}", default_config.database.max_connections);
    println!("   Dispatcher启用: {}", default_config.dispatcher.enabled);
    println!("   Worker启用: {}\n", default_config.worker.enabled);

    // 2. 从TOML字符串加载配置
    println!("2. 从TOML字符串加载配置:");
    let toml_config = r#"
[database]
url = "postgresql://example:5432/scheduler_example"
max_connections = 25
min_connections = 3
connection_timeout_seconds = 45
idle_timeout_seconds = 900

[dispatcher]
enabled = true
schedule_interval_seconds = 15
max_concurrent_dispatches = 200
worker_timeout_seconds = 120
dispatch_strategy = "load_based"

[worker]
enabled = true
worker_id = "example-worker-001"
hostname = "example-host"
ip_address = "192.168.1.100"
max_concurrent_tasks = 10
supported_task_types = ["shell", "http", "sql"]
heartbeat_interval_seconds = 45
task_poll_interval_seconds = 8

[api]
enabled = true
bind_address = "0.0.0.0:9090"
cors_enabled = false
cors_origins = []
request_timeout_seconds = 60
max_request_size_mb = 20

[message_queue]
url = "amqp://example:5672"
task_queue = "example_tasks"
status_queue = "example_status"
heartbeat_queue = "example_heartbeats"
control_queue = "example_control"
max_retries = 5
retry_delay_seconds = 120
connection_timeout_seconds = 45

[observability]
tracing_enabled = true
metrics_enabled = true
metrics_endpoint = "/custom-metrics"
log_level = "debug"
jaeger_endpoint = "http://jaeger:14268/api/traces"
"#;

    let config_from_toml = AppConfig::from_toml(toml_config)?;
    println!("   数据库URL: {}", config_from_toml.database.url);
    println!(
        "   最大连接数: {}",
        config_from_toml.database.max_connections
    );
    println!(
        "   调度策略: {}",
        config_from_toml.dispatcher.dispatch_strategy
    );
    println!("   Worker ID: {}", config_from_toml.worker.worker_id);
    println!("   API绑定地址: {}", config_from_toml.api.bind_address);
    println!(
        "   日志级别: {}\n",
        config_from_toml.observability.log_level
    );

    // 3. 演示环境变量覆盖
    println!("3. 演示环境变量覆盖:");
    env::set_var("SCHEDULER_DATABASE_MAX_CONNECTIONS", "100");
    env::set_var("SCHEDULER_WORKER_WORKER_ID", "env-override-worker");
    env::set_var("SCHEDULER_OBSERVABILITY_LOG_LEVEL", "warn");

    println!("   设置环境变量:");
    println!("   SCHEDULER_DATABASE_MAX_CONNECTIONS=100");
    println!("   SCHEDULER_WORKER_WORKER_ID=env-override-worker");
    println!("   SCHEDULER_OBSERVABILITY_LOG_LEVEL=warn");

    // 使用ConfigLoader加载配置（会应用环境变量覆盖）
    let config_with_env = ConfigLoader::load().unwrap_or_else(|_| AppConfig::default());
    println!("   加载后的配置:");
    println!(
        "   数据库最大连接数: {}",
        config_with_env.database.max_connections
    );
    println!("   Worker ID: {}", config_with_env.worker.worker_id);
    println!("   日志级别: {}\n", config_with_env.observability.log_level);

    // 4. 配置验证
    println!("4. 配置验证:");
    match config_with_env.validate() {
        Ok(_) => println!("   ✓ 配置验证通过"),
        Err(e) => println!("   ✗ 配置验证失败: {e}"),
    }

    // 5. 配置序列化
    println!("\n5. 配置序列化为TOML:");
    let serialized = config_with_env.to_toml()?;
    println!("   配置已序列化为TOML格式 ({} 字符)", serialized.len());

    // 6. 环境检测
    println!("\n6. 环境检测:");
    println!("   当前环境: {}", ConfigLoader::current_env());
    println!("   是否为开发环境: {}", ConfigLoader::is_development());
    println!("   是否为生产环境: {}", ConfigLoader::is_production());

    // 7. 数据库和消息队列URL获取（支持常见环境变量覆盖）
    println!("\n7. 连接字符串获取:");
    println!(
        "   数据库URL: {}",
        ConfigLoader::get_database_url(&config_with_env)
    );
    println!(
        "   消息队列URL: {}",
        ConfigLoader::get_message_queue_url(&config_with_env)
    );

    // 清理环境变量
    env::remove_var("SCHEDULER_DATABASE_MAX_CONNECTIONS");
    env::remove_var("SCHEDULER_WORKER_WORKER_ID");
    env::remove_var("SCHEDULER_OBSERVABILITY_LOG_LEVEL");

    println!("\n=== 配置管理示例完成 ===");
    Ok(())
}
