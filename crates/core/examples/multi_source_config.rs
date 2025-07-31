//! 多源配置合并示例
//!
//! 此示例展示了如何从多个配置源加载配置并按照优先级合并它们。
//! 它演示了文件、环境变量、内存配置等多种配置源的使用。

use scheduler_core::config::manager::{ConfigBuilder, ConfigSource, ReloadStrategy};
use scheduler_core::typesafe::ConfigurationService;
use std::env;
use tempfile::NamedTempFile;
use std::io::Write;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 多源配置合并示例 ===\n");

    // 1. 创建临时配置文件
    println!("1. 创建配置文件:");
    
    let mut temp_file = NamedTempFile::new()?;
    let file_content = r#"
{
    "server": {
        "host": "file-host",
        "port": 3000,
        "debug": false,
        "workers": 4
    },
    "database": {
        "url": "postgresql://file-host:5432/filedb",
        "pool_size": 10,
        "timeout": 30
    },
    "features": {
        "logging": true,
        "metrics": false,
        "tracing": true
    },
    "app": {
        "name": "File Config App",
        "version": "1.0.0"
    }
}
"#;
    
    temp_file.write_all(file_content.as_bytes())?;
    let file_path = temp_file.path().to_path_buf();
    println!("   ✓ 配置文件已创建: {}", file_path.display());

    // 2. 设置环境变量
    println!("\n2. 设置环境变量:");
    env::set_var("APP_SERVER_HOST", "env-host");
    env::set_var("APP_SERVER_PORT", "5000");
    env::set_var("APP_DATABASE_POOL_SIZE", "20");
    env::set_var("APP_FEATURES_METRICS", "true");
    env::set_var("APP_CUSTOM_SETTING", "from_environment");
    println!("   ✓ 环境变量已设置 (前缀: APP_)");

    // 3. 创建内存配置（默认值）
    println!("\n3. 创建内存配置（默认值）:");
    let memory_config = serde_json::json!({
        "server": {
            "host": "default-host",
            "port": 8080,
            "debug": true,
            "workers": 2,
            "timeout": 60
        },
        "database": {
            "url": "postgresql://default-host:5432/defaultdb",
            "pool_size": 5,
            "timeout": 15,
            "max_connections": 10
        },
        "features": {
            "logging": false,
            "metrics": false,
            "tracing": false,
            "caching": true
        },
        "app": {
            "name": "Default App",
            "version": "0.1.0",
            "author": "Default Author"
        },
        "cache": {
            "enabled": true,
            "ttl": 300,
            "max_size": 1000
        }
    });
    println!("   ✓ 内存配置已创建");

    // 4. 创建命令行参数配置
    println!("\n4. 创建命令行参数配置:");
    let cli_args = vec![
        "--server.host".to_string(),
        "cli-host".to_string(),
        "--server.debug".to_string(),
        "--features.tracing".to_string(),
        "--cache.enabled".to_string(),
        "--custom.cli_value".to_string(),
        "from_cli".to_string(),
    ];
    println!("   ✓ 命令行参数已配置");

    // 5. 构建多源配置管理器
    println!("\n5. 构建多源配置管理器:");
    
    let manager = ConfigBuilder::new()
        // 1. 内存配置（最低优先级）
        .add_source(ConfigSource::Memory {
            data: memory_config,
        })
        // 2. 文件配置（中等优先级）
        .add_source(ConfigSource::File {
            path: file_path.clone(),
        })
        // 3. 环境变量配置（高优先级）
        .add_source(ConfigSource::Environment {
            prefix: "APP_".to_string(),
        })
        // 4. 命令行参数（最高优先级）
        .add_source(ConfigSource::CommandLine {
            args: cli_args,
        })
        // 设置重载策略
        .with_reload_strategy(ReloadStrategy::Manual)
        .build();

    println!("   ✓ 配置管理器已创建");
    println!("   ✓ 优先级顺序：内存 < 文件 < 环境变量 < 命令行");

    // 6. 加载配置
    println!("\n6. 加载并合并配置:");
    manager.load().await?;
    println!("   ✓ 配置加载完成");

    // 7. 验证配置合并结果
    println!("\n7. 验证配置合并结果:");
    
    // 服务器配置 - 应该来自命令行（最高优先级）
    let server_host: String = manager.get("server.host").await?;
    let server_port: u16 = manager.get("server.port").await?;
    let server_debug: bool = manager.get("server.debug").await?;
    let server_workers: usize = manager.get("server.workers").await?;
    
    println!("   服务器配置:");
    println!("     主机 (CLI): {}", server_host);
    println!("     端口 (ENV): {}", server_port);
    println!("     调试模式 (CLI): {}", server_debug);
    println!("     工作线程 (FILE): {}", server_workers);

    // 数据库配置 - 混合来源
    let db_url: String = manager.get("database.url").await?;
    let db_pool_size: u32 = manager.get("database.pool_size").await?;
    let db_timeout: u64 = manager.get("database.timeout").await?;
    let db_max_connections: u32 = manager.get("database.max_connections").await?;
    
    println!("   数据库配置:");
    println!("     URL (FILE): {}", db_url);
    println!("     连接池大小 (ENV): {}", db_pool_size);
    println!("     超时时间 (FILE): {}", db_timeout);
    println!("     最大连接数 (MEMORY): {}", db_max_connections);

    // 功能配置 - 混合来源
    let features_logging: bool = manager.get("features.logging").await?;
    let features_metrics: bool = manager.get("features.metrics").await?;
    let features_tracing: bool = manager.get("features.tracing").await?;
    let features_caching: bool = manager.get("features.caching").await?;
    
    println!("   功能配置:");
    println!("     日志 (FILE): {}", features_logging);
    println!("     指标 (ENV): {}", features_metrics);
    println!("     追踪 (CLI): {}", features_tracing);
    println!("     缓存 (MEMORY): {}", features_caching);

    // 应用配置 - 混合来源
    let app_name: String = manager.get("app.name").await?;
    let app_version: String = manager.get("app.version").await?;
    let app_author: String = manager.get("app.author").await?;
    
    println!("   应用配置:");
    println!("     名称 (FILE): {}", app_name);
    println!("     版本 (FILE): {}", app_version);
    println!("     作者 (MEMORY): {}", app_author);

    // 缓存配置 - 来自内存
    let cache_enabled: bool = manager.get("cache.enabled").await?;
    let cache_ttl: u64 = manager.get("cache.ttl").await?;
    let cache_max_size: usize = manager.get("cache.max_size").await?;
    
    println!("   缓存配置:");
    println!("     启用 (CLI): {}", cache_enabled);
    println!("     TTL (MEMORY): {}", cache_ttl);
    println!("     最大大小 (MEMORY): {}", cache_max_size);

    // 自定义配置
    let custom_setting: String = manager.get("custom_setting").await?;
    let cli_value: String = manager.get("custom.cli_value").await?;
    
    println!("   自定义配置:");
    println!("     环境变量设置: {}", custom_setting);
    println!("     命令行值: {}", cli_value);

    // 8. 演示优先级覆盖
    println!("\n8. 配置优先级演示:");
    
    println!("   优先级覆盖示例:");
    println!("   - server.host: 'default-host' (MEMORY) -> 'file-host' (FILE) -> 'env-host' (ENV) -> 'cli-host' (CLI)");
    println!("   - 最终值: '{}'", server_host);
    println!("   ");
    println!("   - database.pool_size: '5' (MEMORY) -> '10' (FILE) -> '20' (ENV)");
    println!("   - 最终值: {}", db_pool_size);
    println!("   ");
    println!("   - features.metrics: 'false' (MEMORY) -> 'false' (FILE) -> 'true' (ENV)");
    println!("   - 最终值: {}", features_metrics);

    // 9. 获取配置子集
    println!("\n9. 获取配置子集:");
    
    let server_subset = manager.get_subset("server").await?;
    let features_subset = manager.get_subset("features").await?;
    
    println!("   服务器配置子集: {}", serde_json::to_string_pretty(&server_subset)?);
    println!("   功能配置子集: {}", serde_json::to_string_pretty(&features_subset)?);

    // 10. 列出所有配置键
    println!("\n10. 所有配置键:");
    let all_keys = manager.list_config_keys().await?;
    for key in &all_keys {
        println!("   - {}", key);
    }

    // 11. 动态更新配置
    println!("\n11. 动态更新配置:");
    
    // 更新配置值
    manager.set("dynamic.setting", serde_json::json!("dynamic_value")).await?;
    manager.set("server.host", serde_json::json!("updated-host")).await?;
    
    let dynamic_value: String = manager.get("dynamic.setting").await?;
    let updated_host: String = manager.get("server.host").await?;
    
    println!("   动态设置值: {}", dynamic_value);
    println!("   更新后的主机: {}", updated_host);

    // 12. 手动重载配置
    println!("\n12. 手动重载配置:");
    
    // 修改文件内容
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&file_path)?;
    
    let updated_content = r#"
{
    "server": {
        "host": "updated-file-host",
        "port": 8888,
        "debug": true,
        "workers": 16
    },
    "database": {
        "url": "postgresql://updated-file-host:5432/updateddb",
        "pool_size": 50,
        "timeout": 120
    }
}
"#;
    
    file.write_all(updated_content.as_bytes())?;
    drop(file);
    
    println!("   ✓ 配置文件已修改");
    
    // 手动重载
    manager.reload().await?;
    println!("   ✓ 配置已重载");
    
    // 验证重载结果
    let reloaded_host: String = manager.get("server.host").await?;
    let reloaded_port: u16 = manager.get("server.port").await?;
    let reloaded_workers: usize = manager.get("server.workers").await?;
    
    println!("   重载后的配置:");
    println!("     主机: {} (应该还是 'cli-host'，因为命令行优先级更高)", reloaded_host);
    println!("     端口: {} (应该还是 '5000'，因为环境变量优先级更高)", reloaded_port);
    println!("     工作线程: {} (应该更新为 '16'，因为文件中没有冲突)", reloaded_workers);

    // 13. 导出配置
    println!("\n13. 导出配置:");
    
    let config_json = manager.to_json().await?;
    println!("   完整配置JSON (前200字符):");
    println!("   {}", &config_json[..config_json.len().min(200)]);

    // 清理
    env::remove_var("APP_SERVER_HOST");
    env::remove_var("APP_SERVER_PORT");
    env::remove_var("APP_DATABASE_POOL_SIZE");
    env::remove_var("APP_FEATURES_METRICS");
    env::remove_var("APP_CUSTOM_SETTING");

    println!("\n=== 多源配置合并示例完成 ===");
    Ok(())
}