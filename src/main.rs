use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use scheduler_config::AppConfig;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod app;
mod shutdown;

use app::{AppMode, Application};
use shutdown::ShutdownManager;

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let matches = Command::new("scheduler")
        .version("1.0.0")
        .about("分布式任务调度系统")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("配置文件路径")
                .default_value("config/scheduler.toml"),
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .value_name("MODE")
                .help("运行模式")
                .value_parser(["dispatcher", "worker", "api", "all"])
                .default_value("all"),
        )
        .arg(
            Arg::new("worker-id")
                .long("worker-id")
                .value_name("ID")
                .help("Worker ID (仅在worker模式下使用)")
                .required_if_eq("mode", "worker"),
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .value_name("LEVEL")
                .help("日志级别")
                .value_parser(["trace", "debug", "info", "warn", "error"])
                .default_value("info"),
        )
        .arg(
            Arg::new("log-format")
                .long("log-format")
                .value_name("FORMAT")
                .help("日志格式")
                .value_parser(["json", "pretty"])
                .default_value("pretty"),
        )
        .get_matches();

    // 获取命令行参数
    let config_path = matches.get_one::<String>("config").unwrap();
    let mode_str = matches.get_one::<String>("mode").unwrap();
    let worker_id = matches.get_one::<String>("worker-id");
    let log_level = matches.get_one::<String>("log-level").unwrap();
    let log_format = matches.get_one::<String>("log-format").unwrap();

    // 初始化日志系统
    init_logging(log_level, log_format)?;

    info!("启动分布式任务调度系统");
    info!("配置文件: {config_path}");
    info!("运行模式: {mode_str}");
    if let Some(id) = worker_id {
        info!("Worker ID: {}", id);
    }

    // 加载配置
    let mut config = AppConfig::load(Some(config_path))
        .with_context(|| format!("加载配置文件失败: {config_path}"))?;

    // 如果指定了worker-id，覆盖配置中的worker_id
    if let Some(id) = worker_id {
        config.worker.worker_id = id.clone();
    }

    // 解析运行模式
    let app_mode = parse_app_mode(mode_str, &config)?;

    // 创建应用实例
    let app = Application::new(config, app_mode).await?;

    // 创建优雅关闭管理器
    let shutdown_manager = ShutdownManager::new();

    // 启动应用
    let app_handle = {
        let app = Arc::new(app);
        let shutdown_rx = shutdown_manager.subscribe().await;
        let app_clone = Arc::clone(&app);

        tokio::spawn(async move {
            if let Err(e) = app_clone.run(shutdown_rx).await {
                error!("应用运行失败: {e}");
            }
        })
    };

    // 等待关闭信号
    wait_for_shutdown_signal().await;

    info!("收到关闭信号，开始优雅关闭...");

    // 触发关闭
    shutdown_manager.shutdown().await;

    // 等待应用关闭，设置超时
    match tokio::time::timeout(Duration::from_secs(30), app_handle).await {
        Ok(result) => {
            if let Err(e) = result {
                error!("应用关闭时发生错误: {e}");
            } else {
                info!("应用已优雅关闭");
            }
        }
        Err(_) => {
            warn!("应用关闭超时，强制退出");
        }
    }

    info!("分布式任务调度系统已退出");
    Ok(())
}

/// 初始化日志系统
fn init_logging(log_level: &str, log_format: &str) -> Result<()> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let registry = tracing_subscriber::registry().with(env_filter);

    match log_format {
        "json" => {
            registry
                .with(tracing_subscriber::fmt::layer().json())
                .try_init()
                .context("初始化JSON日志格式失败")?;
        }
        "pretty" => {
            registry
                .with(tracing_subscriber::fmt::layer().pretty())
                .try_init()
                .context("初始化Pretty日志格式失败")?;
        }
        _ => {
            return Err(anyhow::anyhow!("不支持的日志格式: {log_format}"));
        }
    }

    Ok(())
}

/// 解析应用运行模式
fn parse_app_mode(mode_str: &str, config: &AppConfig) -> Result<AppMode> {
    match mode_str {
        "dispatcher" => {
            if !config.dispatcher.enabled {
                return Err(anyhow::anyhow!("Dispatcher模式被禁用，请检查配置"));
            }
            Ok(AppMode::Dispatcher)
        }
        "worker" => {
            if !config.worker.enabled {
                return Err(anyhow::anyhow!("Worker模式被禁用，请检查配置"));
            }
            Ok(AppMode::Worker)
        }
        "api" => {
            if !config.api.enabled {
                return Err(anyhow::anyhow!("API模式被禁用，请检查配置"));
            }
            Ok(AppMode::Api)
        }
        "all" => Ok(AppMode::All),
        _ => Err(anyhow::anyhow!("不支持的运行模式: {mode_str}")),
    }
}

/// 等待关闭信号
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("安装Ctrl+C信号处理器失败");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("安装SIGTERM信号处理器失败")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("收到Ctrl+C信号");
        },
        _ = terminate => {
            info!("收到SIGTERM信号");
        },
    }
}
