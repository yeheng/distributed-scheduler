use anyhow::Result;
use clap::{Arg, Command};
use scheduler::app::AppMode;
use scheduler::common::{start_application, StartupConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let matches = Command::new("scheduler-dispatcher")
        .version("1.0.0")
        .about("分布式任务调度系统 - Dispatcher服务")
        .long_about("启动任务调度器服务，负责任务的调度和分发")
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
        )
        .get_matches();

    // 获取命令行参数
    let config_path = matches
        .get_one::<String>("config")
        .unwrap()
        .to_string();
    let log_level = matches
        .get_one::<String>("log-level")
        .unwrap()
        .to_string();
    let log_format = matches
        .get_one::<String>("log-format")
        .unwrap()
        .to_string();

    // 创建启动配置
    let startup_config = StartupConfig {
        config_path,
        log_level,
        log_format,
        worker_id: None, // Dispatcher不需要worker_id
    };

    // 启动应用程序
    start_application(startup_config, AppMode::Dispatcher, "Dispatcher").await
}