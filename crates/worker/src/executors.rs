use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use scheduler_core::{Result, SchedulerError, TaskExecutor, TaskResult, TaskRun};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Shell任务执行器
pub struct ShellExecutor {
    /// 正在运行的任务进程ID
    running_processes: Arc<RwLock<HashMap<i64, u32>>>,
}

impl ShellExecutor {
    /// 创建新的Shell执行器
    pub fn new() -> Self {
        Self {
            running_processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ShellExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for ShellExecutor {
    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult> {
        let start_time = Instant::now();

        // 解析任务参数
        let shell_params: ShellTaskParams =
            serde_json::from_str(task_run.result.as_deref().unwrap_or("{}")).map_err(|e| {
                SchedulerError::InvalidTaskParams(format!("解析Shell任务参数失败: {e}"))
            })?;

        let command = shell_params.command;
        let args = shell_params.args.unwrap_or_default();
        let working_dir = shell_params.working_dir;
        let env_vars = shell_params.env_vars.unwrap_or_default();

        info!(
            "执行Shell任务: task_run_id={}, command={}, args={:?}",
            task_run.id, command, args
        );

        // 创建命令
        let mut cmd = Command::new(&command);
        cmd.args(&args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // 设置工作目录
        if let Some(ref dir) = working_dir {
            cmd.current_dir(dir);
        }

        // 设置环境变量
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // 启动进程
        let mut child = cmd
            .spawn()
            .map_err(|e| SchedulerError::TaskExecution(format!("启动Shell命令失败: {e}")))?;

        // 读取输出
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SchedulerError::TaskExecution("无法获取stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| SchedulerError::TaskExecution("无法获取stderr".to_string()))?;

        // 保存进程ID以便取消
        if let Some(pid) = child.id() {
            let mut processes = self.running_processes.write().await;
            processes.insert(task_run.id, pid);
        }

        let mut stdout_reader = BufReader::new(stdout);
        let mut stderr_reader = BufReader::new(stderr);
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();

        // 异步读取stdout和stderr
        let stdout_task = async {
            let mut line = String::new();
            while stdout_reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                stdout_lines.push(line.trim_end().to_string());
                line.clear();
            }
        };

        let stderr_task = async {
            let mut line = String::new();
            while stderr_reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                stderr_lines.push(line.trim_end().to_string());
                line.clear();
            }
        };

        // 等待输出读取完成
        tokio::join!(stdout_task, stderr_task);

        // 等待进程结束
        let exit_status = child
            .wait()
            .await
            .map_err(|e| SchedulerError::TaskExecution(format!("等待进程结束失败: {e}")))?;

        // 从运行进程列表中移除
        {
            let mut processes = self.running_processes.write().await;
            processes.remove(&task_run.id);
        }

        let execution_time = start_time.elapsed();
        let exit_code = exit_status.code();
        let success = exit_status.success();

        let output = if !stdout_lines.is_empty() {
            Some(stdout_lines.join("\n"))
        } else {
            None
        };

        let error_message = if !stderr_lines.is_empty() {
            Some(stderr_lines.join("\n"))
        } else if !success {
            Some(format!("命令执行失败，退出码: {exit_code:?}"))
        } else {
            None
        };

        let result = TaskResult {
            success,
            output,
            error_message,
            exit_code,
            execution_time_ms: execution_time.as_millis() as u64,
        };

        info!(
            "Shell任务执行完成: task_run_id={}, success={}, exit_code={:?}, duration={}ms",
            task_run.id, success, exit_code, result.execution_time_ms
        );

        Ok(result)
    }

    fn supports_task_type(&self, task_type: &str) -> bool {
        task_type == "shell"
    }

    fn name(&self) -> &str {
        "shell"
    }

    async fn cancel(&self, task_run_id: i64) -> Result<()> {
        let mut processes = self.running_processes.write().await;
        if let Some(pid) = processes.remove(&task_run_id) {
            // 使用系统调用终止进程
            #[cfg(unix)]
            {
                use std::process::Command;
                match Command::new("kill").arg(pid.to_string()).output() {
                    Ok(output) => {
                        if output.status.success() {
                            info!(
                                "成功取消Shell任务: task_run_id={}, pid={}",
                                task_run_id, pid
                            );
                            Ok(())
                        } else {
                            let error_msg = String::from_utf8_lossy(&output.stderr);
                            error!(
                                "取消Shell任务失败: task_run_id={}, pid={}, error={}",
                                task_run_id, pid, error_msg
                            );
                            Err(SchedulerError::TaskExecution(format!(
                                "取消任务失败: {error_msg}"
                            )))
                        }
                    }
                    Err(e) => {
                        error!(
                            "执行kill命令失败: task_run_id={}, pid={}, error={}",
                            task_run_id, pid, e
                        );
                        Err(SchedulerError::TaskExecution(format!(
                            "取消任务失败: {e}"
                        )))
                    }
                }
            }
            #[cfg(windows)]
            {
                use std::process::Command;
                match Command::new("taskkill")
                    .args(&["/PID", &pid.to_string(), "/F"])
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            info!(
                                "成功取消Shell任务: task_run_id={}, pid={}",
                                task_run_id, pid
                            );
                            Ok(())
                        } else {
                            let error_msg = String::from_utf8_lossy(&output.stderr);
                            error!(
                                "取消Shell任务失败: task_run_id={}, pid={}, error={}",
                                task_run_id, pid, error_msg
                            );
                            Err(SchedulerError::TaskExecution(format!(
                                "取消任务失败: {}",
                                error_msg
                            )))
                        }
                    }
                    Err(e) => {
                        error!(
                            "执行taskkill命令失败: task_run_id={}, pid={}, error={}",
                            task_run_id, pid, e
                        );
                        Err(SchedulerError::TaskExecution(format!(
                            "取消任务失败: {}",
                            e
                        )))
                    }
                }
            }
        } else {
            warn!("未找到要取消的Shell任务: task_run_id={}", task_run_id);
            Ok(())
        }
    }

    async fn is_running(&self, task_run_id: i64) -> Result<bool> {
        let processes = self.running_processes.read().await;
        Ok(processes.contains_key(&task_run_id))
    }
}

/// HTTP任务执行器
pub struct HttpExecutor {
    /// HTTP客户端
    client: reqwest::Client,
    /// 正在运行的任务
    running_tasks: Arc<RwLock<HashMap<i64, tokio::task::JoinHandle<()>>>>,
}

impl HttpExecutor {
    /// 创建新的HTTP执行器
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for HttpExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for HttpExecutor {
    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult> {
        let start_time = Instant::now();

        // 解析任务参数
        let http_params: HttpTaskParams =
            serde_json::from_str(task_run.result.as_deref().unwrap_or("{}")).map_err(|e| {
                SchedulerError::InvalidTaskParams(format!("解析HTTP任务参数失败: {e}"))
            })?;

        let url = http_params.url;
        let method = http_params.method.unwrap_or_else(|| "GET".to_string());
        let headers = http_params.headers.unwrap_or_default();
        let body = http_params.body;
        let timeout_seconds = http_params.timeout_seconds.unwrap_or(30);

        info!(
            "执行HTTP任务: task_run_id={}, method={}, url={}",
            task_run.id, method, url
        );

        // 构建请求
        let mut request_builder = match method.to_uppercase().as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            "HEAD" => self.client.head(&url),
            _ => {
                return Err(SchedulerError::InvalidTaskParams(format!(
                    "不支持的HTTP方法: {method}"
                )));
            }
        };

        // 设置超时
        request_builder = request_builder.timeout(std::time::Duration::from_secs(timeout_seconds));

        // 设置请求头
        for (key, value) in headers {
            request_builder = request_builder.header(&key, &value);
        }

        // 设置请求体
        if let Some(body_content) = body {
            request_builder = request_builder.body(body_content);
        }

        // 发送请求
        let response_result = request_builder.send().await;

        let execution_time = start_time.elapsed();

        match response_result {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let success = response.status().is_success();

                // 读取响应体
                let response_body = response
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("读取响应体失败: {e}"));

                let result = TaskResult {
                    success,
                    output: Some(format!(
                        "HTTP {method} {url}\nStatus: {status_code}\nResponse:\n{response_body}"
                    )),
                    error_message: if success {
                        None
                    } else {
                        Some(format!("HTTP请求失败，状态码: {status_code}"))
                    },
                    exit_code: Some(status_code as i32),
                    execution_time_ms: execution_time.as_millis() as u64,
                };

                info!(
                    "HTTP任务执行完成: task_run_id={}, success={}, status={}, duration={}ms",
                    task_run.id, success, status_code, result.execution_time_ms
                );

                Ok(result)
            }
            Err(e) => {
                let error_message = format!("HTTP请求失败: {e}");
                error!(
                    "HTTP任务执行失败: task_run_id={}, error={}",
                    task_run.id, error_message
                );

                let result = TaskResult {
                    success: false,
                    output: None,
                    error_message: Some(error_message),
                    exit_code: None,
                    execution_time_ms: execution_time.as_millis() as u64,
                };

                Ok(result)
            }
        }
    }

    fn supports_task_type(&self, task_type: &str) -> bool {
        task_type == "http"
    }

    fn name(&self) -> &str {
        "http"
    }

    async fn cancel(&self, task_run_id: i64) -> Result<()> {
        let mut tasks = self.running_tasks.write().await;
        if let Some(handle) = tasks.remove(&task_run_id) {
            handle.abort();
            info!("成功取消HTTP任务: task_run_id={}", task_run_id);
        } else {
            warn!("未找到要取消的HTTP任务: task_run_id={}", task_run_id);
        }
        Ok(())
    }

    async fn is_running(&self, task_run_id: i64) -> Result<bool> {
        let tasks = self.running_tasks.read().await;
        if let Some(handle) = tasks.get(&task_run_id) {
            Ok(!handle.is_finished())
        } else {
            Ok(false)
        }
    }
}

/// Shell任务参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellTaskParams {
    /// 要执行的命令
    pub command: String,
    /// 命令参数
    pub args: Option<Vec<String>>,
    /// 工作目录
    pub working_dir: Option<String>,
    /// 环境变量
    pub env_vars: Option<HashMap<String, String>>,
}

/// HTTP任务参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpTaskParams {
    /// 请求URL
    pub url: String,
    /// HTTP方法
    pub method: Option<String>,
    /// 请求头
    pub headers: Option<HashMap<String, String>>,
    /// 请求体
    pub body: Option<String>,
    /// 超时时间（秒）
    pub timeout_seconds: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use scheduler_core::{TaskRun, TaskRunStatus};

    fn create_test_task_run(params: &str) -> TaskRun {
        TaskRun {
            id: 1,
            task_id: 1,
            status: TaskRunStatus::Running,
            worker_id: Some("test-worker".to_string()),
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
            result: Some(params.to_string()),
            error_message: None,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_shell_executor_echo() {
        let executor = ShellExecutor::new();

        let params = serde_json::json!({
            "command": "echo",
            "args": ["Hello, World!"]
        });

        let task_run = create_test_task_run(&params.to_string());
        let result = executor.execute(&task_run).await.unwrap();

        assert!(result.success);
        assert_eq!(result.output, Some("Hello, World!".to_string()));
        assert!(result.error_message.is_none());
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_shell_executor_invalid_command() {
        let executor = ShellExecutor::new();

        let params = serde_json::json!({
            "command": "nonexistent_command_12345"
        });

        let task_run = create_test_task_run(&params.to_string());
        let result = executor.execute(&task_run).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shell_executor_supports_task_type() {
        let executor = ShellExecutor::new();

        assert!(executor.supports_task_type("shell"));
        assert!(!executor.supports_task_type("http"));
        assert!(!executor.supports_task_type("other"));
    }

    #[tokio::test]
    async fn test_http_executor_supports_task_type() {
        let executor = HttpExecutor::new();

        assert!(executor.supports_task_type("http"));
        assert!(!executor.supports_task_type("shell"));
        assert!(!executor.supports_task_type("other"));
    }

    #[tokio::test]
    async fn test_http_executor_get_request() {
        let executor = HttpExecutor::new();

        let params = serde_json::json!({
            "url": "https://httpbin.org/get",
            "method": "GET"
        });

        let task_run = create_test_task_run(&params.to_string());
        let result = executor.execute(&task_run).await.unwrap();

        assert!(result.success);
        assert!(result.output.is_some());
        assert!(result.error_message.is_none());
        assert_eq!(result.exit_code, Some(200));
    }

    #[tokio::test]
    async fn test_executor_names() {
        let shell_executor = ShellExecutor::new();
        let http_executor = HttpExecutor::new();

        assert_eq!(shell_executor.name(), "shell");
        assert_eq!(http_executor.name(), "http");
    }
}
