use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};
use scheduler_config::AppConfig;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::app::{AppMode, Application};
use crate::shutdown::ShutdownManager;

/// CLI构建器，用于创建统一的命令行接口
pub struct CliBuilder {
    command: Command,
}

impl CliBuilder {
    /// 创建新的CLI构建器
    pub fn new(name: &'static str, about: &'static str) -> Self {
        let command = Command::new(name)
            .version(env!("CARGO_PKG_VERSION"))
            .about(about);
        
        Self { command }
    }

    /// 添加通用参数（config, log-level, log-format）
    pub fn with_common_args(mut self) -> Self {
        self.command = self.command
            .arg(
                Arg::new("config")
                    .short('c')
                    .long("config")
                    .value_name("FILE")
                    .help("配置文件路径")
                    .default_value("config/scheduler.toml"),
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
            );
        self
    }

    /// 添加长描述
    pub fn with_long_about(mut self, long_about: &'static str) -> Self {
        self.command = self.command.long_about(long_about);
        self
    }

    /// 添加自定义参数
    pub fn with_arg(mut self, arg: Arg) -> Self {
        self.command = self.command.arg(arg);
        self
    }

    /// 添加多个自定义参数
    pub fn with_args(mut self, args: Vec<Arg>) -> Self {
        for arg in args {
            self.command = self.command.arg(arg);
        }
        self
    }

    /// 构建并解析命令行参数
    pub fn build_and_parse(self) -> ArgMatches {
        self.command.get_matches()
    }
}

/// 从命令行参数创建StartupConfig
pub fn create_startup_config_from_matches(matches: &ArgMatches) -> StartupConfig {
    let config_path = matches.get_one::<String>("config").unwrap().to_string();
    let log_level = matches.get_one::<String>("log-level").unwrap().to_string();
    let log_format = matches.get_one::<String>("log-format").unwrap().to_string();
    let worker_id = matches.get_one::<String>("worker-id").map(|s| s.to_string());

    StartupConfig {
        config_path,
        log_level,
        log_format,
        worker_id,
    }
}

/// 统一的CLI运行函数
pub async fn run_cli(
    app_name: &'static str,
    app_description: &'static str,
    long_description: Option<&'static str>,
    custom_args: Vec<Arg>,
    app_mode: AppMode,
    service_name: &str,
) -> Result<()> {
    // 创建CLI构建器
    let mut cli_builder = CliBuilder::new(app_name, app_description)
        .with_common_args();

    // 添加长描述
    if let Some(long_desc) = long_description {
        cli_builder = cli_builder.with_long_about(long_desc);
    }

    // 添加自定义参数
    cli_builder = cli_builder.with_args(custom_args);

    // 解析命令行参数
    let matches = cli_builder.build_and_parse();

    // 创建启动配置
    let startup_config = create_startup_config_from_matches(&matches);

    // 启动应用程序
    start_application(startup_config, app_mode, service_name).await
}

/// 通用的应用启动配置
#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub config_path: String,
    pub log_level: String,
    pub log_format: String,
    pub worker_id: Option<String>,
}

/// 初始化日志系统
pub fn init_logging(log_level: &str, log_format: &str) -> Result<()> {
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

/// 加载应用配置
pub fn load_config(startup_config: &StartupConfig) -> Result<AppConfig> {
    // 验证配置文件路径
    if !std::path::Path::new(&startup_config.config_path).exists() {
        return Err(anyhow::anyhow!(
            "配置文件不存在: {}",
            startup_config.config_path
        ));
    }

    // 加载配置
    let mut config = AppConfig::load(Some(&startup_config.config_path))
        .with_context(|| format!("加载配置文件失败: {}", startup_config.config_path))?;

    // 如果指定了worker-id，覆盖配置中的worker_id
    if let Some(ref worker_id) = startup_config.worker_id {
        config.worker.worker_id = worker_id.clone();
    }

    Ok(config)
}

/// 启动应用程序的通用函数
pub async fn start_application(
    startup_config: StartupConfig,
    app_mode: AppMode,
    service_name: &str,
) -> Result<()> {
    // 初始化日志系统
    init_logging(&startup_config.log_level, &startup_config.log_format)?;

    info!("启动 {} 服务", service_name);
    info!("配置文件: {}", startup_config.config_path);
    info!("运行模式: {:?}", app_mode);
    if let Some(ref worker_id) = startup_config.worker_id {
        info!("Worker ID: {}", worker_id);
    }

    // 加载配置
    let config = load_config(&startup_config)?;

    // 验证模式是否被启用
    validate_mode_enabled(&app_mode, &config)?;

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
                info!("{} 服务已优雅关闭", service_name);
            }
        }
        Err(_) => {
            warn!("{} 服务关闭超时，强制退出", service_name);
        }
    }

    info!("{} 服务已退出", service_name);
    Ok(())
}

/// 验证指定的模式是否在配置中被启用
fn validate_mode_enabled(app_mode: &AppMode, config: &AppConfig) -> Result<()> {
    match app_mode {
        AppMode::Dispatcher => {
            if !config.dispatcher.enabled {
                return Err(anyhow::anyhow!("Dispatcher模式被禁用，请检查配置"));
            }
        }
        AppMode::Worker => {
            if !config.worker.enabled {
                return Err(anyhow::anyhow!("Worker模式被禁用，请检查配置"));
            }
        }
        AppMode::Api => {
            if !config.api.enabled {
                return Err(anyhow::anyhow!("API模式被禁用，请检查配置"));
            }
        }
        AppMode::All => {
            // All模式不需要特殊验证
        }
    }
    Ok(())
}

/// 等待关闭信号
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.unwrap_or_else(|e| {
            error!("安装Ctrl+C信号处理器失败: {}", e);
            std::process::exit(1);
        })
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => signal.recv().await,
            Err(e) => {
                error!("安装SIGTERM信号处理器失败: {}", e);
                std::process::exit(1);
            }
        }
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

/// 解析应用运行模式
pub fn parse_app_mode(mode_str: &str, config: &AppConfig) -> Result<AppMode> {
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
