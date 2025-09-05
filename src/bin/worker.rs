use anyhow::Result;
use clap::Arg;
use scheduler::app::AppMode;
use scheduler::common::run_cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Worker服务特有参数
    let custom_args = vec![
        Arg::new("worker-id")
            .short('w')
            .long("worker-id")
            .value_name("ID")
            .help("Worker节点唯一标识符")
            .required(true),
        Arg::new("max-tasks")
            .short('m')
            .long("max-tasks")
            .value_name("COUNT")
            .help("最大并发任务数量")
            .value_parser(clap::value_parser!(u32)),
    ];

    run_cli(
        "scheduler-worker",
        "分布式任务调度系统 - Worker服务",
        Some("启动Worker节点服务，负责执行从调度器分发的任务"),
        custom_args,
        AppMode::Worker,
        "Worker",
    ).await
}
