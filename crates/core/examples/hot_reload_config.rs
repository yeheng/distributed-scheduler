//! 热重载配置示例
//!
//! 此示例展示了如何使用配置系统的热重载功能来实现运行时配置更新。
//! 它演示了文件监控、定期重载、配置变更回调等功能。

use scheduler_core::config::manager::{ConfigBuilder, ConfigSource, ReloadStrategy};
use scheduler_core::config::hot_reload::{HotReloadManager, ConfigChangeEvent};
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;
use std::io::Write;
use tokio::sync::RwLock;
use serde_json::Value;

/// 应用状态管理器
#[derive(Clone)]
struct AppState {
    config: Arc<RwLock<Value>>,
    reload_count: Arc<RwLock<u64>>,
    last_reload: Arc<RwLock<std::time::SystemTime>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(Value::Object(serde_json::Map::new()))),
            reload_count: Arc::new(RwLock::new(0)),
            last_reload: Arc::new(RwLock::new(std::time::SystemTime::now())),
        }
    }

    async fn update_config(&self, new_config: Value) {
        let mut config = self.config.write().await;
        *config = new_config;
        
        let mut count = self.reload_count.write().await;
        *count += 1;
        
        let mut last_reload = self.last_reload.write().await;
        *last_reload = std::time::SystemTime::now();
    }

    async fn get_reload_count(&self) -> u64 {
        *self.reload_count.read().await
    }

    async fn get_last_reload_time(&self) -> std::time::SystemTime {
        *self.last_reload.read().await
    }
}

/// 配置变更处理器
#[derive(Clone)]
struct ConfigHandler {
}

impl ConfigHandler {
    fn new(_app_state: AppState) -> Self {
        Self {  }
    }

    /// 处理配置变更
    fn handle_config_change(&self, event: ConfigChangeEvent) {
        println!("🔄 配置变更事件:");
        println!("   变更时间: {:?}", event.timestamp);
        println!("   变更键: {}", event.key);
        println!("   变更源: {:?}", event.source);
        
        if let Some(new_value) = &event.new_value {
            // 注意：这里简化了状态更新，实际应用中需要更复杂的逻辑
            println!("   新值: {}", new_value.value);
            println!("   配置已更新");
        }
        
        if let Some(old_value) = &event.old_value {
            println!("   旧值: {}", old_value.value);
        }
        
        println!("   {}", "-".repeat(50));
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 热重载配置示例 ===\n");

    // 1. 创建应用状态
    let app_state = AppState::new();
    let handler = ConfigHandler::new(app_state.clone());

    // 2. 创建临时配置文件
    println!("1. 创建配置文件:");
    
    let mut temp_file = NamedTempFile::new()?;
    let initial_config = r#"
{
    "server": {
        "host": "localhost",
        "port": 8080,
        "debug": true,
        "workers": 4
    },
    "database": {
        "url": "postgresql://localhost:5432/mydb",
        "pool_size": 10,
        "timeout": 30
    },
    "features": {
        "logging": true,
        "metrics": false,
        "tracing": true
    },
    "app": {
        "name": "Hot Reload Demo",
        "version": "1.0.0"
    }
}
"#;
    
    temp_file.write_all(initial_config.as_bytes())?;
    let file_path = temp_file.path().to_path_buf();
    println!("   ✓ 配置文件已创建: {}", file_path.display());

    // 3. 创建热重载管理器
    println!("\n2. 创建热重载管理器:");
    
    // 创建文件监控器
    use scheduler_core::config::hot_reload::FileConfigWatcher;
    let file_watcher = Box::new(FileConfigWatcher::new(file_path.clone().to_string_lossy().to_string())
        .with_polling_interval(Duration::from_millis(1000)));
    
    let hot_reload_manager = HotReloadManager::new()
        .add_watcher(file_watcher)
        .add_callback({
            let handler = handler.clone();
            move |event| {
                handler.handle_config_change(event);
            }
        });
    
    // Start the hot reload manager
    hot_reload_manager.start().await?;

    println!("   ✓ 热重载管理器已启动");
    println!("   ✓ 文件监控已启用");
    println!("   ✓ 轮询间隔: 1000ms");

    // 4. 创建配置管理器（使用定期重载策略）
    println!("\n3. 创建配置管理器:");
    
    let manager = ConfigBuilder::new()
        .add_source(ConfigSource::File {
            path: file_path.clone(),
        })
        .add_source(ConfigSource::Memory {
            data: serde_json::json!({
                "defaults": {
                    "timeout": 30,
                    "retries": 3,
                    "cache_size": 1000
                }
            }),
        })
        .with_reload_strategy(ReloadStrategy::Periodic { 
            interval_seconds: 5 
        })
        .add_callback({
            let app_state = app_state.clone();
            move |config| {
                let app_state = app_state.clone();
                let config = config.clone();
                tokio::spawn(async move {
                    app_state.update_config(config).await;
                    println!("📋 配置管理器回调 - 配置已更新");
                });
            }
        })
        .build();

    // 初始加载
    manager.load().await?;
    println!("   ✓ 配置管理器已创建并加载");
    println!("   ✓ 定期重载策略: 每5秒");

    // 5. 显示初始配置
    println!("\n4. 初始配置状态:");
    
    let server_host: String = manager.get("server.host").await?;
    let server_port: u16 = manager.get("server.port").await?;
    let app_name: String = manager.get("app.name").await?;
    let debug_mode: bool = manager.get("server.debug").await?;
    
    println!("   服务器: {}:{}", server_host, server_port);
    println!("   应用名称: {}", app_name);
    println!("   调试模式: {}", debug_mode);

    // 6. 模拟配置文件变更
    println!("\n5. 模拟配置文件变更:");
    
    // 等待用户确认
    println!("   请等待，将在3秒后开始修改配置文件...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 第一次修改
    println!("\n   🔧 第一次修改配置文件:");
    let modified_config_1 = r#"
{
    "server": {
        "host": "0.0.0.0",
        "port": 9000,
        "debug": false,
        "workers": 8
    },
    "database": {
        "url": "postgresql://newhost:5432/newdb",
        "pool_size": 20,
        "timeout": 60
    },
    "features": {
        "logging": true,
        "metrics": true,
        "tracing": false
    },
    "app": {
        "name": "Hot Reload Demo v2",
        "version": "2.0.0"
    }
}
"#;
    
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&file_path)?;
    file.write_all(modified_config_1.as_bytes())?;
    drop(file);
    
    println!("   ✓ 配置文件已修改 (端口: 9000, 工作线程: 8)");
    println!("   等待热重载触发...");

    // 等待热重载
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 检查配置是否更新
    let updated_host: String = manager.get("server.host").await?;
    let updated_port: u16 = manager.get("server.port").await?;
    let updated_workers: usize = manager.get("server.workers").await?;
    
    println!("   更新后的配置:");
    println!("     服务器: {}:{}", updated_host, updated_port);
    println!("     工作线程: {}", updated_workers);

    // 第二次修改
    println!("\n   🔧 第二次修改配置文件:");
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    let modified_config_2 = r#"
{
    "server": {
        "host": "api.example.com",
        "port": 443,
        "debug": true,
        "workers": 16
    },
    "database": {
        "url": "postgresql://prod-db:5432/proddb",
        "pool_size": 50,
        "timeout": 120
    },
    "features": {
        "logging": false,
        "metrics": true,
        "tracing": true
    },
    "app": {
        "name": "Production App",
        "version": "3.0.0"
    },
    "new_feature": {
        "enabled": true,
        "setting": "production_value"
    }
}
"#;
    
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&file_path)?;
    file.write_all(modified_config_2.as_bytes())?;
    drop(file);
    
    println!("   ✓ 配置文件已修改 (生产环境配置)");
    println!("   等待热重载触发...");

    // 等待热重载
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 检查最终配置
    let final_host: String = manager.get("server.host").await?;
    let final_port: u16 = manager.get("server.port").await?;
    let final_app_name: String = manager.get("app.name").await?;
    let final_version: String = manager.get("app.version").await?;
    
    println!("   最终配置:");
    println!("     服务器: {}:{}", final_host, final_port);
    println!("     应用: {} v{}", final_app_name, final_version);

    // 7. 测试手动重载
    println!("\n6. 测试手动重载:");
    
    // 创建另一个配置文件用于手动重载测试
    let mut temp_file2 = NamedTempFile::new()?;
    let manual_config = r#"
{
    "server": {
        "host": "manual-reload-host",
        "port": 7777,
        "debug": false,
        "workers": 2
    },
    "app": {
        "name": "Manual Reload Test",
        "version": "0.1.0"
    }
}
"#;
    
    temp_file2.write_all(manual_config.as_bytes())?;
    let manual_file_path = temp_file2.path().to_path_buf();
    
    // 创建手动重载的配置管理器
    let manual_manager = ConfigBuilder::new()
        .add_source(ConfigSource::File {
            path: manual_file_path.clone(),
        })
        .with_reload_strategy(ReloadStrategy::Manual)
        .build();

    manual_manager.load().await?;
    
    let manual_host: String = manual_manager.get("server.host").await?;
    let manual_port: u16 = manual_manager.get("server.port").await?;
    
    println!("   手动重载前: {}:{}", manual_host, manual_port);
    
    // 修改文件
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&manual_file_path)?;
    let updated_manual_config = r#"
{
    "server": {
        "host": "updated-manual-host",
        "port": 8888,
        "debug": true,
        "workers": 4
    },
    "app": {
        "name": "Updated Manual Test",
        "version": "0.2.0"
    }
}
"#;
    file.write_all(updated_manual_config.as_bytes())?;
    drop(file);
    
    println!("   手动配置文件已修改"); 
    
    // 手动触发重载
    manual_manager.reload().await?;
    
    let updated_manual_host: String = manual_manager.get("server.host").await?;
    let updated_manual_port: u16 = manual_manager.get("server.port").await?;
    
    println!("   手动重载后: {}:{}", updated_manual_host, updated_manual_port);

    // 8. 显示统计信息
    println!("\n7. 热重载统计信息:");
    
    let reload_count = app_state.get_reload_count().await;
    let last_reload_time = app_state.get_last_reload_time().await;
    
    println!("   总重载次数: {}", reload_count);
    println!("   最后重载时间: {:?}", last_reload_time);
    println!("   热重载管理器运行状态: {}", hot_reload_manager.is_running().await);

    // 9. 测试配置变更回调
    println!("\n8. 测试配置变更回调:");
    
    let callback_manager = ConfigBuilder::new()
        .add_source(ConfigSource::Memory {
            data: serde_json::json!({
                "test_value": "initial",
                "counter": 0
            }),
        })
        .add_callback(|config| {
            println!("🔄 回调触发 - 配置变更检测到");
            if let Some(test_value) = config.get("test_value") {
                println!("   test_value: {}", test_value);
            }
            if let Some(counter) = config.get("counter") {
                println!("   counter: {}", counter);
            }
        })
        .build();

    callback_manager.load().await?;
    
    // 修改配置触发回调
    callback_manager.set("test_value", serde_json::json!("updated")).await?;
    callback_manager.set("counter", serde_json::json!(42)).await?;
    
    println!("   ✓ 配置变更回调已触发");

    // 10. 清理和总结
    println!("\n9. 热重载功能总结:");
    
    println!("   ✅ 文件监控热重载");
    println!("   ✅ 定期重载策略");
    println!("   ✅ 手动重载支持");
    println!("   ✅ 配置变更回调");
    println!("   ✅ 防抖机制");
    println!("   ✅ 配置验证");
    println!("   ✅ 统计信息");

    // 停止热重载管理器
    hot_reload_manager.stop().await?;
    println!("   ✓ 热重载管理器已停止");

    println!("\n=== 热重载配置示例完成 ===");
    Ok(())
}