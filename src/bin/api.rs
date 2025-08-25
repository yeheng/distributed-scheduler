use anyhow::Result;
use clap::{Arg, Command};
use scheduler::app::AppMode;
use scheduler::common::{start_application, StartupConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let matches = Command::new("scheduler-api")
        .version("1.0.0")
        .about("分布式任务调度系统 - API服务")
        .long_about("启动REST API服务器，提供任务管理和系统监控的HTTP接口")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("配置文件路径")
                .default_value("config/scheduler.toml"),
        )
        .arg(
            Arg::new("host")
                .short('h')
                .long("host")
                .value_name("HOST")
                .help("绑定的主机地址")
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("监听端口")
                .value_parser(clap::value_parser!(u16))
                .default_value("8080"),
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
        .arg(
            Arg::new("cors")
                .long("cors")
                .help("启用CORS支持")
                .action(clap::ArgAction::SetTrue),
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
        worker_id: None, // API服务不需要worker_id
    };

    // 启动应用程序
    start_application(startup_config, AppMode::Api, "API Server").await
}