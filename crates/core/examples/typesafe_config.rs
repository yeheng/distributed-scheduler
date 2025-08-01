//! 类型安全配置示例
//!
//! 此示例展示了如何使用类型安全的配置包装器来确保配置值的类型正确性。
//! 它演示了 TypedConfig、ConfigBuilder 和环境特定配置的使用方法。

use scheduler_core::config::typesafe::{ConfigBuilder, Environment, TypedConfig};
use serde::{Deserialize, Serialize};

/// 数据库配置结构
#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    pub url: String,
    pub pool_size: u32,
    pub timeout_seconds: u64,
    pub max_connections: u32,
    pub min_connections: u32,
}

/// 服务器配置结构
#[derive(Debug, Serialize, Deserialize)]
struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub debug: bool,
    pub workers: usize,
}

/// 应用配置结构
#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    pub app_name: String,
    pub version: String,
    pub environment: String,
    pub database: DatabaseConfig,
    pub server: ServerConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: "My Application".to_string(),
            version: "1.0.0".to_string(),
            environment: "development".to_string(),
            database: DatabaseConfig {
                url: "postgresql://localhost:5432/mydb".to_string(),
                pool_size: 10,
                timeout_seconds: 30,
                max_connections: 20,
                min_connections: 2,
            },
            server: ServerConfig {
                host: "localhost".to_string(),
                port: 8080,
                debug: true,
                workers: 4,
            },
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 类型安全配置示例 ===\n");

    // 1. 使用构建器创建配置管理器
    println!("1. 创建配置管理器:");
    let manager = ConfigBuilder::new()
        .add_source(scheduler_core::config::manager::ConfigSource::Memory {
            data: serde_json::json!({
                "app_name": "类型安全应用",
                "version": "2.0.0",
                "environment": "development",
                "database": {
                    "url": "postgresql://localhost:5432/typed_example",
                    "pool_size": 15,
                    "timeout_seconds": 45,
                    "max_connections": 30,
                    "min_connections": 5
                },
                "server": {
                    "host": "0.0.0.0",
                    "port": 9000,
                    "debug": true,
                    "workers": 8
                }
            }),
        })
        .build();

    // 加载配置
    manager.load().await?;
    println!("   ✓ 配置管理器创建并加载成功");

    // 2. 创建类型安全的配置包装器
    println!("\n2. 创建类型安全的配置包装器:");

    let app_config = TypedConfig::<AppConfig>::new(manager.clone(), "app_config".to_string());

    let db_config = TypedConfig::<DatabaseConfig>::new(manager.clone(), "database".to_string());

    let server_config = TypedConfig::<ServerConfig>::new(manager.clone(), "server".to_string());

    let app_name = TypedConfig::<String>::new(manager.clone(), "app_name".to_string());

    println!("   ✓ 应用配置包装器已创建");
    println!("   ✓ 数据库配置包装器已创建");
    println!("   ✓ 服务器配置包装器已创建");
    println!("   ✓ 应用名称包装器已创建");

    // 3. 获取配置值（类型安全）
    println!("\n3. 获取配置值（类型安全）:");

    // 尝试获取完整的应用配置（这个会失败，因为我们的数据结构不同）
    match app_config.get().await? {
        Some(config) => {
            println!("   应用名称: {}", config.app_name);
            println!("   版本: {}", config.version);
            println!("   环境: {}", config.environment);
        }
        None => {
            println!("   ℹ  应用配置不存在，这是因为数据结构与键名不匹配");
        }
    }

    // 获取数据库配置
    match db_config.get().await? {
        Some(db) => {
            println!("   数据库URL: {}", db.url);
            println!("   连接池大小: {}", db.pool_size);
            println!("   超时时间: {}秒", db.timeout_seconds);
            println!("   最大连接数: {}", db.max_connections);
            println!("   最小连接数: {}", db.min_connections);
        }
        None => {
            println!("   ✗ 数据库配置不存在");
        }
    }

    // 获取服务器配置
    match server_config.get().await? {
        Some(server) => {
            println!("   服务器地址: {}:{}", server.host, server.port);
            println!("   调试模式: {}", server.debug);
            println!("   工作线程数: {}", server.workers);
        }
        None => {
            println!("   ✗ 服务器配置不存在");
        }
    }

    // 获取应用名称
    match app_name.get().await? {
        Some(name) => {
            println!("   应用名称: {}", name);
        }
        None => {
            println!("   ✗ 应用名称不存在");
        }
    }

    // 4. 使用默认值
    println!("\n4. 使用默认值:");

    let default_port = server_config
        .get_or_default(ServerConfig {
            host: "default_host".to_string(),
            port: 8080,
            debug: false,
            workers: 2,
        })
        .await?;

    println!(
        "   默认服务器配置: {}:{}",
        default_port.host, default_port.port
    );

    // 5. 设置配置值
    println!("\n5. 设置配置值:");

    let new_db_config = DatabaseConfig {
        url: "postgresql://newhost:5432/newdb".to_string(),
        pool_size: 20,
        timeout_seconds: 60,
        max_connections: 50,
        min_connections: 10,
    };

    db_config.set(&new_db_config).await?;
    println!("   ✓ 数据库配置已更新");

    // 验证更新后的配置
    match db_config.get().await? {
        Some(db) => {
            println!("   更新后的数据库URL: {}", db.url);
            println!("   更新后的连接池大小: {}", db.pool_size);
        }
        None => {
            println!("   ✗ 配置更新后获取失败");
        }
    }

    // 6. 检查配置是否存在
    println!("\n6. 检查配置存在性:");

    println!("   数据库配置存在: {}", db_config.exists().await?);
    println!("   服务器配置存在: {}", server_config.exists().await?);
    println!("   应用名称存在: {}", app_name.exists().await?);

    // 7. 删除配置
    println!("\n7. 删除配置:");

    let deleted = app_name.delete().await?;
    println!("   应用名称删除成功: {}", deleted);
    println!("   删除后应用名称存在: {}", app_name.exists().await?);

    // 8. 环境特定配置
    println!("\n8. 环境特定配置:");

    let current_env = Environment::current()?;
    println!("   当前环境: {:?}", current_env);
    println!("   是否为开发环境: {}", current_env.is_development());
    println!("   是否为生产环境: {}", current_env.is_production());

    // 9. 构建器模式的类型安全配置
    println!("\n9. 构建器模式的类型安全配置:");

    let builder_manager = ConfigBuilder::new()
        .add_source(scheduler_core::config::manager::ConfigSource::Memory {
            data: serde_json::json!({
                "direct_string": "Hello from builder!",
                "direct_number": 42,
                "direct_bool": true
            }),
        })
        .build();

    builder_manager.load().await?;

    // 直接使用构建器获取类型安全的值
    let string_value: String = builder_manager.get("direct_string").await?;
    let number_value: i32 = builder_manager.get("direct_number").await?;
    let bool_value: bool = builder_manager.get("direct_bool").await?;

    println!("   直接字符串: {}", string_value);
    println!("   直接数字: {}", number_value);
    println!("   直接布尔值: {}", bool_value);

    // 10. 配置验证和错误处理
    println!("\n10. 配置验证和错误处理:");

    // 尝试获取不存在的配置
    let missing_config = TypedConfig::<String>::new(manager.clone(), "nonexistent_key".to_string());

    match missing_config.get().await? {
        Some(value) => println!("   意外找到值: {}", value),
        None => println!("   ✓ 正确处理了不存在的配置键"),
    }

    // 尝试类型错误的配置
    let type_mismatch = TypedConfig::<u32>::new(
        manager.clone(),
        "app_name".to_string(), // 这是字符串，不是u32
    );

    match type_mismatch.get().await {
        Ok(_) => println!("   意外成功"),
        Err(e) => println!("   ✓ 正确捕获了类型错误: {}", e),
    }

    println!("\n=== 类型安全配置示例完成 ===");
    Ok(())
}
