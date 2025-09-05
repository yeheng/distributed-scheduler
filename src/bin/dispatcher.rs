use anyhow::Result;
use scheduler::app::AppMode;
use scheduler::common::run_cli;

#[tokio::main]
async fn main() -> Result<()> {
    run_cli(
        "scheduler-dispatcher",
        "分布式任务调度系统 - Dispatcher服务",
        Some("启动任务调度器服务，负责任务的调度和分发"),
        vec![], // Dispatcher没有特殊参数
        AppMode::Dispatcher,
        "Dispatcher",
    ).await
}
