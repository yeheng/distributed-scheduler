use anyhow::Result;
use clap::{Arg, Command};
use scheduler::app::AppMode;
use scheduler::common::{start_application, StartupConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let matches = Command::new("scheduler-worker")
        .version("1.0.0")
        .about("分布式任务调度系统 - Worker服务")
        .long_about("启动Worker节点服务，负责执行从调度器分发的任务")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("配置文件路径")
                .default_value("config/scheduler.toml"),
        )
        .arg(
            Arg::new("worker-id")
                .short('w')
                .long("worker-id")
                .value_name("ID")
                .help("Worker节点唯一标识符")
                .required(true),
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
            Arg::new("max-tasks")
                .short('m')
                .long("max-tasks")
                .value_name("COUNT")
                .help("最大并发任务数量")
                .value_parser(clap::value_parser!(u32)),
        )
        .get_matches();

    // 获取命令行参数
    let config_path = matches.get_one::<String>("config").unwrap().to_string();
    let worker_id = matches.get_one::<String>("worker-id").unwrap().to_string();
    let log_level = matches.get_one::<String>("log-level").unwrap().to_string();
    let log_format = matches.get_one::<String>("log-format").unwrap().to_string();

    // 创建启动配置
    let startup_config = StartupConfig {
        config_path,
        log_level,
        log_format,
        worker_id: Some(worker_id),
    };

    // 启动应用程序
    start_application(startup_config, AppMode::Worker, "Worker").await
}
