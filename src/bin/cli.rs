use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use reqwest::Client;
use scheduler_config::AppConfig;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub api_base_url: String,
    pub api_key: Option<String>,
    pub config_path: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化简单的日志系统
    tracing_subscriber::fmt::init();

    let cli = CliApp::parse();
    cli.run().await
}

/// CLI应用程序主结构
#[derive(clap::Parser, Debug)]
#[command(name = "scheduler-cli")]
#[command(version = "1.0.0")]
#[command(about = "分布式任务调度系统 - 命令行管理工具")]
#[command(long_about = "提供任务管理、Worker管理、系统监控等功能的命令行接口")]
struct CliApp {
    #[command(subcommand)]
    command: Commands,

    /// API服务器基础URL
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    api_url: String,

    /// API认证密钥
    #[arg(long)]
    api_key: Option<String>,

    /// 配置文件路径
    #[arg(short, long, default_value = "config/scheduler.toml")]
    config: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 任务管理
    Task(TaskCommands),
    /// Worker管理
    Worker(WorkerCommands),
    /// 系统状态
    Status(StatusCommands),
    /// 配置管理
    Config(ConfigCommands),
}

#[derive(Args, Debug)]
struct TaskCommands {
    #[command(subcommand)]
    action: TaskActions,
}

#[derive(Subcommand, Debug)]
enum TaskActions {
    /// 创建新任务
    Create {
        /// 任务名称
        #[arg(short, long)]
        name: String,
        /// 任务描述
        #[arg(short, long)]
        description: Option<String>,
        /// 执行命令
        #[arg(short, long)]
        command: String,
        /// 任务参数 (JSON格式)
        #[arg(short, long)]
        args: Option<String>,
        /// Cron表达式 (定时任务)
        #[arg(long)]
        cron: Option<String>,
        /// 任务优先级 (1-10)
        #[arg(short, long, default_value = "5")]
        priority: u8,
        /// 最大重试次数
        #[arg(short, long, default_value = "3")]
        retries: u32,
        /// 超时时间 (秒)
        #[arg(short, long, default_value = "3600")]
        timeout: u64,
    },
    /// 列出任务
    List {
        /// 任务状态过滤
        #[arg(short, long)]
        status: Option<String>,
        /// 每页显示数量
        #[arg(short, long, default_value = "20")]
        limit: u32,
        /// 页偏移
        #[arg(short, long, default_value = "0")]
        offset: u32,
    },
    /// 查看任务详情
    Get {
        /// 任务ID
        task_id: String,
    },
    /// 更新任务
    Update {
        /// 任务ID
        task_id: String,
        /// 新的任务名称
        #[arg(short, long)]
        name: Option<String>,
        /// 新的任务描述
        #[arg(short, long)]
        description: Option<String>,
        /// 新的执行命令
        #[arg(short, long)]
        command: Option<String>,
        /// 新的任务参数
        #[arg(short, long)]
        args: Option<String>,
    },
    /// 删除任务
    Delete {
        /// 任务ID
        task_id: String,
        /// 强制删除 (不询问确认)
        #[arg(short, long)]
        force: bool,
    },
    /// 启动任务
    Start {
        /// 任务ID
        task_id: String,
    },
    /// 停止任务
    Stop {
        /// 任务ID
        task_id: String,
    },
    /// 查看任务运行历史
    History {
        /// 任务ID
        task_id: String,
        /// 显示最近N条记录
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },
}

#[derive(Args, Debug)]
struct WorkerCommands {
    #[command(subcommand)]
    action: WorkerActions,
}

#[derive(Subcommand, Debug)]
enum WorkerActions {
    /// 列出Worker节点
    List {
        /// Worker状态过滤
        #[arg(short, long)]
        status: Option<String>,
    },
    /// 查看Worker详情
    Get {
        /// Worker ID
        worker_id: String,
    },
    /// 注销Worker节点
    Unregister {
        /// Worker ID
        worker_id: String,
        /// 强制注销
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Args, Debug)]
struct StatusCommands {
    #[command(subcommand)]
    action: StatusActions,
}

#[derive(Subcommand, Debug)]
enum StatusActions {
    /// 显示系统整体状态
    Overall,
    /// 显示任务统计
    Tasks,
    /// 显示Worker统计
    Workers,
    /// 显示系统健康状况
    Health,
}

#[derive(Args, Debug)]
struct ConfigCommands {
    #[command(subcommand)]
    action: ConfigActions,
}

#[derive(Subcommand, Debug)]
enum ConfigActions {
    /// 显示当前配置
    Show,
    /// 验证配置文件
    Validate,
    /// 生成示例配置
    Example,
}

impl CliApp {
    fn parse() -> Self {
        <Self as clap::Parser>::parse()
    }

    async fn run(self) -> Result<()> {
        let config = CliConfig {
            api_base_url: self.api_url,
            api_key: self.api_key,
            config_path: self.config,
        };

        match self.command {
            Commands::Task(task_cmd) => handle_task_commands(task_cmd, &config).await,
            Commands::Worker(worker_cmd) => handle_worker_commands(worker_cmd, &config).await,
            Commands::Status(status_cmd) => handle_status_commands(status_cmd, &config).await,
            Commands::Config(config_cmd) => handle_config_commands(config_cmd, &config).await,
        }
    }
}

// 任务管理命令处理
async fn handle_task_commands(task_cmd: TaskCommands, config: &CliConfig) -> Result<()> {
    let client = create_http_client(config)?;

    match task_cmd.action {
        TaskActions::Create {
            name,
            description,
            command,
            args,
            cron,
            priority,
            retries,
            timeout,
        } => {
            let task_args = match args {
                Some(args_str) => serde_json::from_str::<Value>(&args_str)
                    .context("解析任务参数失败，请确保是有效的JSON格式")?,
                None => json!({}),
            };

            let task_data = json!({
                "name": name,
                "description": description,
                "command": command,
                "args": task_args,
                "cron_expression": cron,
                "priority": priority,
                "max_retries": retries,
                "timeout_seconds": timeout
            });

            let response = client
                .post(&format!("{}/api/v1/tasks", config.api_base_url))
                .json(&task_data)
                .send()
                .await
                .context("发送任务创建请求失败")?;

            if response.status().is_success() {
                let task: Value = response.json().await?;
                println!("任务创建成功!");
                println!("任务ID: {}", task["id"].as_str().unwrap_or("N/A"));
                println!("任务名称: {}", task["name"].as_str().unwrap_or("N/A"));
            } else {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("创建任务失败: {}", error_text));
            }
        }
        TaskActions::List {
            status,
            limit,
            offset,
        } => {
            let mut url = format!("{}/api/v1/tasks?limit={}&offset={}", config.api_base_url, limit, offset);
            if let Some(status) = status {
                url.push_str(&format!("&status={}", status));
            }

            let response = client.get(&url).send().await.context("获取任务列表失败")?;

            if response.status().is_success() {
                let tasks: Value = response.json().await?;
                print_tasks_table(&tasks)?;
            } else {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("获取任务列表失败: {}", error_text));
            }
        }
        TaskActions::Get { task_id } => {
            let response = client
                .get(&format!("{}/api/v1/tasks/{}", config.api_base_url, task_id))
                .send()
                .await
                .context("获取任务详情失败")?;

            if response.status().is_success() {
                let task: Value = response.json().await?;
                print_task_details(&task)?;
            } else {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("获取任务详情失败: {}", error_text));
            }
        }
        TaskActions::Delete { task_id, force } => {
            if !force {
                println!("确定要删除任务 {} 吗? (y/N)", task_id);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("已取消删除操作");
                    return Ok(());
                }
            }

            let response = client
                .delete(&format!("{}/api/v1/tasks/{}", config.api_base_url, task_id))
                .send()
                .await
                .context("删除任务失败")?;

            if response.status().is_success() {
                println!("任务删除成功: {}", task_id);
            } else {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("删除任务失败: {}", error_text));
            }
        }
        _ => {
            println!("该功能暂未实现");
        }
    }

    Ok(())
}

// Worker管理命令处理
async fn handle_worker_commands(worker_cmd: WorkerCommands, config: &CliConfig) -> Result<()> {
    let client = create_http_client(config)?;

    match worker_cmd.action {
        WorkerActions::List { status } => {
            let mut url = format!("{}/api/v1/workers", config.api_base_url);
            if let Some(status) = status {
                url.push_str(&format!("?status={}", status));
            }

            let response = client.get(&url).send().await.context("获取Worker列表失败")?;

            if response.status().is_success() {
                let workers: Value = response.json().await?;
                print_workers_table(&workers)?;
            } else {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("获取Worker列表失败: {}", error_text));
            }
        }
        WorkerActions::Get { worker_id } => {
            let response = client
                .get(&format!("{}/api/v1/workers/{}", config.api_base_url, worker_id))
                .send()
                .await
                .context("获取Worker详情失败")?;

            if response.status().is_success() {
                let worker: Value = response.json().await?;
                print_worker_details(&worker)?;
            } else {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("获取Worker详情失败: {}", error_text));
            }
        }
        _ => {
            println!("该功能暂未实现");
        }
    }

    Ok(())
}

// 状态命令处理
async fn handle_status_commands(status_cmd: StatusCommands, config: &CliConfig) -> Result<()> {
    let client = create_http_client(config)?;

    match status_cmd.action {
        StatusActions::Overall => {
            let response = client
                .get(&format!("{}/api/v1/status", config.api_base_url))
                .send()
                .await
                .context("获取系统状态失败")?;

            if response.status().is_success() {
                let status: Value = response.json().await?;
                print_system_status(&status)?;
            } else {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("获取系统状态失败: {}", error_text));
            }
        }
        StatusActions::Health => {
            let response = client
                .get(&format!("{}/api/v1/health", config.api_base_url))
                .send()
                .await
                .context("获取健康状态失败")?;

            if response.status().is_success() {
                let health: Value = response.json().await?;
                print_health_status(&health)?;
            } else {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("获取健康状态失败: {}", error_text));
            }
        }
        _ => {
            println!("该功能暂未实现");
        }
    }

    Ok(())
}

// 配置命令处理
async fn handle_config_commands(config_cmd: ConfigCommands, config: &CliConfig) -> Result<()> {
    match config_cmd.action {
        ConfigActions::Show => {
            let app_config = AppConfig::load(Some(&config.config_path))
                .context("加载配置文件失败")?;
            println!("配置文件路径: {}", config.config_path);
            println!("配置内容:");
            println!("{:#?}", app_config);
        }
        ConfigActions::Validate => {
            match AppConfig::load(Some(&config.config_path)) {
                Ok(_) => println!("✓ 配置文件验证通过"),
                Err(e) => return Err(anyhow::anyhow!("✗ 配置文件验证失败: {}", e)),
            }
        }
        ConfigActions::Example => {
            println!("示例配置文件内容:");
            println!("{}", generate_example_config());
        }
    }

    Ok(())
}

// 辅助函数
fn create_http_client(_config: &CliConfig) -> Result<Client> {
    let builder = Client::builder().timeout(std::time::Duration::from_secs(30));

    let client = builder.build().context("创建HTTP客户端失败")?;
    Ok(client)
}

fn print_tasks_table(tasks: &Value) -> Result<()> {
    println!("{:<36} {:<20} {:<15} {:<20} {:<12}", "ID", "名称", "状态", "创建时间", "优先级");
    println!("{}", "-".repeat(100));

    if let Some(tasks_array) = tasks.as_array() {
        for task in tasks_array {
            let id = task["id"].as_str().unwrap_or("N/A");
            let name = task["name"].as_str().unwrap_or("N/A");
            let status = task["status"].as_str().unwrap_or("N/A");
            let created_at = task["created_at"].as_str().unwrap_or("N/A");
            let priority = task["priority"].as_u64().unwrap_or(0);

            println!("{:<36} {:<20} {:<15} {:<20} {:<12}", id, name, status, created_at, priority);
        }
    }

    Ok(())
}

fn print_task_details(task: &Value) -> Result<()> {
    println!("任务详情:");
    println!("  ID: {}", task["id"].as_str().unwrap_or("N/A"));
    println!("  名称: {}", task["name"].as_str().unwrap_or("N/A"));
    println!("  描述: {}", task["description"].as_str().unwrap_or("N/A"));
    println!("  状态: {}", task["status"].as_str().unwrap_or("N/A"));
    println!("  命令: {}", task["command"].as_str().unwrap_or("N/A"));
    println!("  优先级: {}", task["priority"].as_u64().unwrap_or(0));
    println!("  最大重试次数: {}", task["max_retries"].as_u64().unwrap_or(0));
    println!("  超时时间: {} 秒", task["timeout_seconds"].as_u64().unwrap_or(0));
    println!("  创建时间: {}", task["created_at"].as_str().unwrap_or("N/A"));
    println!("  更新时间: {}", task["updated_at"].as_str().unwrap_or("N/A"));

    if let Some(cron) = task["cron_expression"].as_str() {
        println!("  Cron表达式: {}", cron);
    }

    if let Some(args) = task.get("args") {
        println!("  参数: {}", serde_json::to_string_pretty(args)?);
    }

    Ok(())
}

fn print_workers_table(workers: &Value) -> Result<()> {
    println!("{:<20} {:<15} {:<15} {:<20} {:<10}", "Worker ID", "状态", "IP地址", "最后心跳", "任务数");
    println!("{}", "-".repeat(85));

    if let Some(workers_array) = workers.as_array() {
        for worker in workers_array {
            let id = worker["worker_id"].as_str().unwrap_or("N/A");
            let status = worker["status"].as_str().unwrap_or("N/A");
            let ip = worker["ip_address"].as_str().unwrap_or("N/A");
            let last_heartbeat = worker["last_heartbeat"].as_str().unwrap_or("N/A");
            let task_count = worker["current_task_count"].as_u64().unwrap_or(0);

            println!("{:<20} {:<15} {:<15} {:<20} {:<10}", id, status, ip, last_heartbeat, task_count);
        }
    }

    Ok(())
}

fn print_worker_details(worker: &Value) -> Result<()> {
    println!("Worker节点详情:");
    println!("  Worker ID: {}", worker["worker_id"].as_str().unwrap_or("N/A"));
    println!("  状态: {}", worker["status"].as_str().unwrap_or("N/A"));
    println!("  主机名: {}", worker["hostname"].as_str().unwrap_or("N/A"));
    println!("  IP地址: {}", worker["ip_address"].as_str().unwrap_or("N/A"));
    println!("  最大并发任务数: {}", worker["max_concurrent_tasks"].as_u64().unwrap_or(0));
    println!("  当前任务数: {}", worker["current_task_count"].as_u64().unwrap_or(0));
    println!("  注册时间: {}", worker["registered_at"].as_str().unwrap_or("N/A"));
    println!("  最后心跳: {}", worker["last_heartbeat"].as_str().unwrap_or("N/A"));

    Ok(())
}

fn print_system_status(status: &Value) -> Result<()> {
    println!("系统状态概览:");
    println!("  系统启动时间: {}", status["uptime"].as_str().unwrap_or("N/A"));
    println!("  总任务数: {}", status["total_tasks"].as_u64().unwrap_or(0));
    println!("  活跃任务数: {}", status["active_tasks"].as_u64().unwrap_or(0));
    println!("  完成任务数: {}", status["completed_tasks"].as_u64().unwrap_or(0));
    println!("  失败任务数: {}", status["failed_tasks"].as_u64().unwrap_or(0));
    println!("  在线Worker数: {}", status["online_workers"].as_u64().unwrap_or(0));
    println!("  离线Worker数: {}", status["offline_workers"].as_u64().unwrap_or(0));

    Ok(())
}

fn print_health_status(health: &Value) -> Result<()> {
    println!("系统健康状态:");
    println!("  整体状态: {}", health["status"].as_str().unwrap_or("N/A"));

    if let Some(checks) = health["checks"].as_object() {
        for (name, check) in checks {
            let status = check["status"].as_str().unwrap_or("N/A");
            let message = check["message"].as_str().unwrap_or("N/A");
            println!("  {}: {} - {}", name, status, message);
        }
    }

    Ok(())
}

fn generate_example_config() -> &'static str {
    r#"
# 分布式任务调度系统配置文件示例
[database]
url = "postgresql://scheduler:password@localhost:5432/scheduler"
max_connections = 20
min_connections = 5

[message_queue]
type = "rabbitmq"
url = "amqp://guest:guest@localhost:5672"
task_queue = "scheduler.tasks"
status_queue = "scheduler.status"
heartbeat_queue = "scheduler.heartbeat"

[api]
enabled = true
host = "127.0.0.1"
port = 8080
cors_enabled = true

[dispatcher]
enabled = true
schedule_interval_seconds = 10
max_retries = 3

[worker]
enabled = true
worker_id = "worker-001"
hostname = "localhost"
ip_address = "127.0.0.1"
max_concurrent_tasks = 10
heartbeat_interval_seconds = 30

[logging]
level = "info"
format = "json"
"#
}