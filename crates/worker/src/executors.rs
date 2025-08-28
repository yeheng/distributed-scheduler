use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use scheduler_application::{ExecutorStatus, TaskExecutor};
use scheduler_application::TaskExecutionContext;
use scheduler_domain::entities::TaskResult;
use scheduler_errors::{SchedulerError, SchedulerResult};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub struct ShellExecutor {
    running_processes: Arc<RwLock<HashMap<i64, u32>>>,
    /// Maximum time to wait for process termination during cleanup (in milliseconds)
    cleanup_timeout_ms: u64,
}

impl ShellExecutor {
    pub fn new() -> Self {
        Self {
            running_processes: Arc::new(RwLock::new(HashMap::new())),
            cleanup_timeout_ms: 5000, // Default 5 second timeout for graceful termination
        }
    }

    /// Create a new ShellExecutor with custom cleanup timeout
    pub fn with_cleanup_timeout(cleanup_timeout_ms: u64) -> Self {
        Self {
            running_processes: Arc::new(RwLock::new(HashMap::new())),
            cleanup_timeout_ms,
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
    async fn execute_task(&self, context: &TaskExecutionContext) -> SchedulerResult<TaskResult> {
        let start_time = Instant::now();
        let shell_params: ShellTaskParams = serde_json::from_value(
            context.parameters.get("shell_params").cloned()
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "command": context.parameters.get("command").and_then(|v| v.as_str()).unwrap_or("echo"),
                        "args": context.parameters.get("args").and_then(|v| v.as_array()).map(|arr| {
                            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>()
                        })
                    })
                })
        ).map_err(|e| {
            SchedulerError::InvalidTaskParams(format!("解析Shell任务参数失败: {e}"))
        })?;

        let command = shell_params.command;
        let args = shell_params.args.unwrap_or_default();
        let working_dir = shell_params
            .working_dir
            .or_else(|| context.working_directory.clone());
        let mut env_vars = shell_params.env_vars.unwrap_or_default();
        for (key, value) in &context.environment {
            env_vars.insert(key.to_string(), value.to_string());
        }

        info!(
            "执行Shell任务: task_run_id={}, command={}, args={:?}",
            context.task_run.id, command, args
        );
        let mut cmd = Command::new(&command);
        cmd.args(&args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        if let Some(ref dir) = working_dir {
            cmd.current_dir(dir);
        }
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| SchedulerError::TaskExecution(format!("启动Shell命令失败: {e}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SchedulerError::TaskExecution("无法获取stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| SchedulerError::TaskExecution("无法获取stderr".to_string()))?;
        if let Some(pid) = child.id() {
            let mut processes = self.running_processes.write().await;
            processes.insert(context.task_run.id, pid);
        }

        let mut stdout_reader = BufReader::new(stdout);
        let mut stderr_reader = BufReader::new(stderr);
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();
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
        tokio::join!(stdout_task, stderr_task);
        let exit_status = child
            .wait()
            .await
            .map_err(|e| SchedulerError::TaskExecution(format!("等待进程结束失败: {e}")))?;
        {
            let mut processes = self.running_processes.write().await;
            processes.remove(&context.task_run.id);
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
            context.task_run.id, success, exit_code, result.execution_time_ms
        );

        Ok(result)
    }

    // async fn execute(&self, task_run: &TaskRun) -> SchedulerResult<TaskResult> {
    //     let task_info = task_run
    //         .result
    //         .as_ref()
    //         .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
    //         .unwrap_or_else(|| serde_json::json!({}));

    //     let parameters = task_info
    //         .get("parameters")
    //         .and_then(|p| p.as_object())
    //         .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    //         .unwrap_or_default();

    //     let context = TaskExecutionContext {
    //         task_run: task_run.clone(),
    //         task_type: "shell".to_string(),
    //         parameters,
    //         timeout_seconds: 300,
    //         environment: HashMap::new(),
    //         working_directory: None,
    //         resource_limits: ResourceLimits::default(),
    //     };

    //     self.execute_task(&context).await
    // }

    fn supports_task_type(&self, task_type: &str) -> bool {
        task_type == "shell"
    }

    fn name(&self) -> &str {
        "shell"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "Enhanced Shell task executor with improved parameter handling"
    }

    fn supported_task_types(&self) -> Vec<String> {
        vec!["shell".to_string()]
    }

    async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()> {
        let mut processes = self.running_processes.write().await;
        if let Some(pid) = processes.remove(&task_run_id) {
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
                        Err(SchedulerError::TaskExecution(format!("取消任务失败: {e}")))
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

    async fn is_running(&self, task_run_id: i64) -> SchedulerResult<bool> {
        let processes = self.running_processes.read().await;
        Ok(processes.contains_key(&task_run_id))
    }

    async fn get_status(&self) -> SchedulerResult<ExecutorStatus> {
        Ok(ExecutorStatus {
            name: self.name().to_string(),
            version: self.version().to_string(),
            healthy: true,
            running_tasks: self.running_processes.read().await.len() as i32,
            supported_task_types: self.supported_task_types(),
            last_health_check: Utc::now(),
            metadata: std::collections::HashMap::new(),
        })
    }

    async fn health_check(&self) -> SchedulerResult<bool> {
        Ok(true)
    }

    async fn warm_up(&self) -> SchedulerResult<()> {
        Ok(())
    }

    async fn cleanup(&self) -> SchedulerResult<()> {
        info!("开始清理ShellExecutor资源");

        let mut processes = self.running_processes.write().await;
        let running_process_count = processes.len();

        if running_process_count > 0 {
            warn!(
                "发现 {} 个未完成的Shell进程，开始强制终止",
                running_process_count
            );

            // Force kill all running processes
            for (task_run_id, pid) in processes.iter() {
                info!("强制终止进程: task_run_id={}, pid={}", task_run_id, pid);

                #[cfg(unix)]
                {
                    // Try SIGTERM first, then SIGKILL if needed
                    let term_result = std::process::Command::new("kill")
                        .arg("-TERM")
                        .arg(pid.to_string())
                        .output();

                    if term_result.is_ok() {
                        // Wait with timeout for graceful termination
                        let timeout_duration = Duration::from_millis(self.cleanup_timeout_ms);
                        let start_time = Instant::now();

                        // Check if process is still running
                        let mut process_terminated = false;
                        while start_time.elapsed() < timeout_duration {
                            // Check if process still exists (sending signal 0)
                            let check_result = std::process::Command::new("kill")
                                .arg("-0")
                                .arg(pid.to_string())
                                .output();

                            if check_result.is_err() || !check_result.unwrap().status.success() {
                                process_terminated = true;
                                break;
                            }

                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }

                        // Force kill if still running after timeout
                        if !process_terminated {
                            warn!(
                                "进程 {} 在 {}ms 内未响应SIGTERM，发送SIGKILL",
                                pid, self.cleanup_timeout_ms
                            );
                            let _ = std::process::Command::new("kill")
                                .arg("-KILL")
                                .arg(pid.to_string())
                                .output();
                        } else {
                            info!("进程 {} 已优雅终止", pid);
                        }
                    } else {
                        // If SIGTERM failed, try SIGKILL immediately
                        let _ = std::process::Command::new("kill")
                            .arg("-KILL")
                            .arg(pid.to_string())
                            .output();
                    }
                }

                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(&["/PID", &pid.to_string(), "/F"])
                        .output();
                }
            }

            // Clear the tracking map
            processes.clear();
            info!("已清理所有Shell进程追踪记录");
        }

        info!("ShellExecutor资源清理完成");
        Ok(())
    }
}

impl Drop for ShellExecutor {
    fn drop(&mut self) {
        // Emergency cleanup if executor is dropped without proper cleanup
        if let Ok(processes) = self.running_processes.try_read() {
            if !processes.is_empty() {
                warn!(
                    "ShellExecutor被丢弃时发现 {} 个未清理的进程，执行紧急清理",
                    processes.len()
                );
                for (task_run_id, pid) in processes.iter() {
                    warn!("紧急终止进程: task_run_id={}, pid={}", task_run_id, pid);

                    #[cfg(unix)]
                    {
                        let _ = std::process::Command::new("kill")
                            .arg("-KILL")
                            .arg(pid.to_string())
                            .output();
                    }

                    #[cfg(windows)]
                    {
                        let _ = std::process::Command::new("taskkill")
                            .args(&["/PID", &pid.to_string(), "/F"])
                            .output();
                    }
                }
            }
        }
    }
}

pub struct HttpExecutor {
    client: reqwest::Client,
    running_tasks: Arc<RwLock<HashMap<i64, tokio::task::JoinHandle<()>>>>,
}

impl HttpExecutor {
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
    async fn execute_task(&self, context: &TaskExecutionContext) -> SchedulerResult<TaskResult> {
        let start_time = Instant::now();
        let http_params: HttpTaskParams = serde_json::from_value(
            context.parameters.get("http_params").cloned()
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "url": context.parameters.get("url").and_then(|v| v.as_str()).unwrap_or("http://example.com"),
                        "method": context.parameters.get("method").and_then(|v| v.as_str()).unwrap_or("GET"),
                        "headers": context.parameters.get("headers"),
                        "body": context.parameters.get("body").and_then(|v| v.as_str()),
                        "timeout_seconds": context.timeout_seconds
                    })
                })
        ).map_err(|e| {
            SchedulerError::InvalidTaskParams(format!("解析HTTP任务参数失败: {e}"))
        })?;

        let url = http_params.url;
        let method = http_params.method.unwrap_or_else(|| "GET".to_string());
        let headers = http_params.headers.unwrap_or_default();
        let body = http_params.body;
        let timeout_seconds = http_params
            .timeout_seconds
            .unwrap_or(context.timeout_seconds);

        info!(
            "执行HTTP任务: task_run_id={}, method={}, url={}",
            context.task_run.id, method, url
        );
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
        request_builder = request_builder.timeout(std::time::Duration::from_secs(timeout_seconds));
        for (key, value) in headers {
            request_builder = request_builder.header(&key, &value);
        }
        if let Some(body_content) = body {
            request_builder = request_builder.body(body_content);
        }

        // Track the running task
        let _task_run_id = context.task_run.id;

        // Execute the request with timeout
        let response_result =
            tokio::time::timeout(Duration::from_secs(timeout_seconds), request_builder.send())
                .await;

        let execution_time = start_time.elapsed();

        match response_result {
            Ok(Ok(response)) => {
                let status_code = response.status().as_u16();
                let success = response.status().is_success();
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
                    context.task_run.id, success, status_code, result.execution_time_ms
                );

                Ok(result)
            }
            Ok(Err(e)) => {
                // HTTP request failed
                let error_message = format!("HTTP请求失败: {e}");
                error!(
                    "HTTP任务执行失败: task_run_id={}, error={}",
                    context.task_run.id, error_message
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
            Err(_) => {
                // Timeout occurred
                let error_message = format!("HTTP请求超时: 请求在 {timeout_seconds} 秒内未完成");
                error!(
                    "HTTP任务执行超时: task_run_id={}, timeout={}s",
                    context.task_run.id, timeout_seconds
                );

                let result = TaskResult {
                    success: false,
                    output: None,
                    error_message: Some(error_message),
                    exit_code: Some(408), // HTTP 408 Request Timeout
                    execution_time_ms: execution_time.as_millis() as u64,
                };

                Ok(result)
            }
        }
    }

    // async fn execute(&self, task_run: &TaskRun) -> SchedulerResult<TaskResult> {
    //     let task_info = task_run
    //         .result
    //         .as_ref()
    //         .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
    //         .unwrap_or_else(|| serde_json::json!({}));

    //     let parameters = task_info
    //         .get("parameters")
    //         .and_then(|p| p.as_object())
    //         .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    //         .unwrap_or_default();

    //     let context = TaskExecutionContext {
    //         task_run: task_run.clone(),
    //         task_type: "http".to_string(),
    //         parameters,
    //         timeout_seconds: 300,
    //         environment: HashMap::new(),
    //         working_directory: None,
    //         resource_limits: ResourceLimits::default(),
    //     };

    //     self.execute_task(&context).await
    // }

    fn supports_task_type(&self, task_type: &str) -> bool {
        task_type == "http"
    }

    fn name(&self) -> &str {
        "http"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "Enhanced HTTP task executor with improved parameter handling"
    }

    fn supported_task_types(&self) -> Vec<String> {
        vec!["http".to_string()]
    }

    async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()> {
        let mut tasks = self.running_tasks.write().await;
        if let Some(handle) = tasks.remove(&task_run_id) {
            handle.abort();
            info!("成功取消HTTP任务: task_run_id={}", task_run_id);
        } else {
            warn!("未找到要取消的HTTP任务: task_run_id={}", task_run_id);
        }
        Ok(())
    }

    async fn is_running(&self, task_run_id: i64) -> SchedulerResult<bool> {
        let tasks = self.running_tasks.read().await;
        if let Some(handle) = tasks.get(&task_run_id) {
            Ok(!handle.is_finished())
        } else {
            Ok(false)
        }
    }

    async fn get_status(&self) -> SchedulerResult<ExecutorStatus> {
        Ok(ExecutorStatus {
            name: self.name().to_string(),
            version: self.version().to_string(),
            healthy: true,
            running_tasks: self.running_tasks.read().await.len() as i32,
            supported_task_types: self.supported_task_types(),
            last_health_check: Utc::now(),
            metadata: std::collections::HashMap::new(),
        })
    }

    async fn health_check(&self) -> SchedulerResult<bool> {
        Ok(true)
    }

    async fn warm_up(&self) -> SchedulerResult<()> {
        Ok(())
    }

    async fn cleanup(&self) -> SchedulerResult<()> {
        info!("开始清理HttpExecutor资源");

        let mut tasks = self.running_tasks.write().await;
        let running_task_count = tasks.len();

        if running_task_count > 0 {
            warn!("发现 {} 个未完成的HTTP任务，开始取消", running_task_count);

            // Cancel all running HTTP tasks
            for (task_run_id, handle) in tasks.drain() {
                info!("取消HTTP任务: task_run_id={}", task_run_id);
                handle.abort();

                // Wait a brief moment for task to abort
                tokio::time::sleep(Duration::from_millis(10)).await;
            }

            info!("已取消所有HTTP任务");
        }

        info!("HttpExecutor资源清理完成");
        Ok(())
    }
}

impl Drop for HttpExecutor {
    fn drop(&mut self) {
        // Emergency cleanup if executor is dropped without proper cleanup
        if let Ok(tasks) = self.running_tasks.try_read() {
            if !tasks.is_empty() {
                warn!(
                    "HttpExecutor被丢弃时发现 {} 个未清理的HTTP任务，执行紧急清理",
                    tasks.len()
                );
                for (task_run_id, handle) in tasks.iter() {
                    warn!("紧急取消HTTP任务: task_run_id={}", task_run_id);
                    handle.abort();
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellTaskParams {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub env_vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpTaskParams {
    pub url: String,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug)]
pub struct MockTaskExecutor {
    name: String,
    should_succeed: bool,
    execution_time_ms: u64,
}

impl MockTaskExecutor {
    pub fn new(name: String, should_succeed: bool, execution_time_ms: u64) -> Self {
        Self {
            name,
            should_succeed,
            execution_time_ms,
        }
    }
}

#[async_trait]
impl TaskExecutor for MockTaskExecutor {
    async fn execute_task(&self, _context: &TaskExecutionContext) -> SchedulerResult<TaskResult> {
        sleep(Duration::from_millis(self.execution_time_ms)).await;

        if self.should_succeed {
            Ok(TaskResult {
                success: true,
                output: Some(format!("Mock task {} executed successfully", self.name)),
                error_message: None,
                exit_code: Some(0),
                execution_time_ms: self.execution_time_ms,
            })
        } else {
            Ok(TaskResult {
                success: false,
                output: None,
                error_message: Some(format!("Mock task {} execution failed", self.name)),
                exit_code: Some(1),
                execution_time_ms: self.execution_time_ms,
            })
        }
    }

    // async fn execute(&self, _task_run: &TaskRun) -> SchedulerResult<TaskResult> {
    //     sleep(Duration::from_millis(self.execution_time_ms)).await;

    //     if self.should_succeed {
    //         Ok(TaskResult {
    //             success: true,
    //             output: Some("任务执行成功".to_string()),
    //             error_message: None,
    //             exit_code: Some(0),
    //             execution_time_ms: self.execution_time_ms,
    //         })
    //     } else {
    //         Ok(TaskResult {
    //             success: false,
    //             output: None,
    //             error_message: Some("任务执行失败".to_string()),
    //             exit_code: Some(1),
    //             execution_time_ms: self.execution_time_ms,
    //         })
    //     }
    // }

    fn supports_task_type(&self, task_type: &str) -> bool {
        task_type == self.name
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Mock task executor for testing"
    }

    fn supported_task_types(&self) -> Vec<String> {
        vec![self.name.to_string()]
    }

    async fn cancel(&self, _task_run_id: i64) -> SchedulerResult<()> {
        Ok(())
    }

    async fn is_running(&self, _task_run_id: i64) -> SchedulerResult<bool> {
        Ok(false)
    }

    async fn get_status(&self) -> SchedulerResult<ExecutorStatus> {
        Ok(ExecutorStatus {
            name: self.name().to_string(),
            version: self.version().to_string(),
            healthy: true,
            running_tasks: 0,
            supported_task_types: self.supported_task_types(),
            last_health_check: Utc::now(),
            metadata: std::collections::HashMap::new(),
        })
    }

    async fn health_check(&self) -> SchedulerResult<bool> {
        Ok(true)
    }

    async fn warm_up(&self) -> SchedulerResult<()> {
        Ok(())
    }

    async fn cleanup(&self) -> SchedulerResult<()> {
        Ok(())
    }
}
