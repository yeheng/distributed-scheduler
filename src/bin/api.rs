use anyhow::Result;
use clap::Arg;
use scheduler::app::AppMode;
use scheduler::common::run_cli;

#[tokio::main]
async fn main() -> Result<()> {
    // API服务特有参数
    let custom_args = vec![
        Arg::new("host")
            .long("host")
            .value_name("HOST")
            .help("绑定的主机地址")
            .default_value("127.0.0.1"),
        Arg::new("port")
            .short('p')
            .long("port")
            .value_name("PORT")
            .help("监听端口")
            .value_parser(clap::value_parser!(u16))
            .default_value("8080"),
        Arg::new("cors")
            .long("cors")
            .help("启用CORS支持")
            .action(clap::ArgAction::SetTrue),
    ];

    run_cli(
        "scheduler-api",
        "分布式任务调度系统 - API服务",
        Some("启动REST API服务器，提供任务管理和系统监控的HTTP接口"),
        custom_args,
        AppMode::Api,
        "API Server",
    ).await
}
