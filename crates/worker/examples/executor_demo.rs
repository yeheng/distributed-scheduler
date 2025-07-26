use chrono::Utc;
use scheduler_core::{TaskExecutor, TaskRun, TaskRunStatus};
use scheduler_worker::{HttpExecutor, HttpTaskParams, ShellExecutor, ShellTaskParams};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== 任务执行器演示 ===\n");

    // 演示Shell执行器
    demo_shell_executor().await?;

    println!("\n{}\n", "=".repeat(50));

    // 演示HTTP执行器
    demo_http_executor().await?;

    Ok(())
}

async fn demo_shell_executor() -> Result<(), Box<dyn std::error::Error>> {
    println!("🐚 Shell执行器演示");

    let executor = Arc::new(ShellExecutor::new());

    // 创建Shell任务参数
    let shell_params = ShellTaskParams {
        command: "echo".to_string(),
        args: Some(vec!["Hello from Shell Executor!".to_string()]),
        working_dir: None,
        env_vars: None,
    };

    // 创建任务运行实例
    let task_run = TaskRun {
        id: 1,
        task_id: 1,
        status: TaskRunStatus::Running,
        worker_id: Some("demo-worker".to_string()),
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        result: Some(serde_json::to_string(&shell_params)?),
        error_message: None,
        created_at: Utc::now(),
    };

    println!("执行命令: {} {:?}", shell_params.command, shell_params.args);

    // 执行任务
    match executor.execute(&task_run).await {
        Ok(result) => {
            println!("✅ 执行成功!");
            println!("   成功: {}", result.success);
            println!("   输出: {:?}", result.output);
            println!("   退出码: {:?}", result.exit_code);
            println!("   执行时间: {}ms", result.execution_time_ms);
        }
        Err(e) => {
            println!("❌ 执行失败: {}", e);
        }
    }

    Ok(())
}

async fn demo_http_executor() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 HTTP执行器演示");

    let executor = Arc::new(HttpExecutor::new());

    // 创建HTTP任务参数
    let http_params = HttpTaskParams {
        url: "https://httpbin.org/json".to_string(),
        method: Some("GET".to_string()),
        headers: None,
        body: None,
        timeout_seconds: Some(10),
    };

    // 创建任务运行实例
    let task_run = TaskRun {
        id: 2,
        task_id: 2,
        status: TaskRunStatus::Running,
        worker_id: Some("demo-worker".to_string()),
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        result: Some(serde_json::to_string(&http_params)?),
        error_message: None,
        created_at: Utc::now(),
    };

    println!(
        "发送HTTP请求: {} {}",
        http_params.method.as_ref().unwrap(),
        http_params.url
    );

    // 执行任务
    match executor.execute(&task_run).await {
        Ok(result) => {
            println!("✅ 执行成功!");
            println!("   成功: {}", result.success);
            println!("   退出码: {:?}", result.exit_code);
            println!("   执行时间: {}ms", result.execution_time_ms);
            if let Some(output) = &result.output {
                // 只显示前200个字符以避免输出过长
                let preview = if output.len() > 200 {
                    format!("{}...", &output[..200])
                } else {
                    output.clone()
                };
                println!("   输出预览: {}", preview);
            }
        }
        Err(e) => {
            println!("❌ 执行失败: {}", e);
        }
    }

    Ok(())
}
