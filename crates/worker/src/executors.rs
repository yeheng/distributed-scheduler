use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use scheduler_core::{
    Result, SchedulerError, TaskExecutionContextTrait, TaskExecutor, TaskResult, TaskRun,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::sleep;
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
    async fn execute_task(&self, context: &TaskExecutionContextTrait) -> Result<TaskResult> {
        let start_time = Instant::now();

        // 从上下文中获取Shell任务参数
        let shell_params: ShellTaskParams = serde_json::from_value(
            context.parameters.get("shell_params").cloned()
                .unwrap_or_else(|| {
                    // 兼容旧格式
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

        // 合并上下文中的环境变量
        for (key, value) in &context.environment {
            env_vars.insert(key.clone(), value.clone());
        }

        info!(
            "执行Shell任务: task_run_id={}, command={}, args={:?}",
            context.task_run.id, command, args
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
            processes.insert(context.task_run.id, pid);
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

    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult> {
        // 保持向后兼容，委托给新的execute_task方法
        let task_info = task_run
            .result
            .as_ref()
            .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let parameters = task_info
            .get("parameters")
            .and_then(|p| p.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let context = TaskExecutionContextTrait {
            task_run: task_run.clone(),
            task_type: "shell".to_string(),
            parameters,
            timeout_seconds: 300,
            environment: HashMap::new(),
            working_directory: None,
            resource_limits: scheduler_core::ResourceLimits::default(),
        };

        self.execute_task(&context).await
    }

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
    async fn execute_task(&self, context: &TaskExecutionContextTrait) -> Result<TaskResult> {
        let start_time = Instant::now();

        // 从上下文中获取HTTP任务参数
        let http_params: HttpTaskParams = serde_json::from_value(
            context.parameters.get("http_params").cloned()
                .unwrap_or_else(|| {
                    // 兼容旧格式
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
                    context.task_run.id, success, status_code, result.execution_time_ms
                );

                Ok(result)
            }
            Err(e) => {
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
        }
    }

    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult> {
        // 保持向后兼容，委托给新的execute_task方法
        let task_info = task_run
            .result
            .as_ref()
            .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let parameters = task_info
            .get("parameters")
            .and_then(|p| p.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let context = TaskExecutionContextTrait {
            task_run: task_run.clone(),
            task_type: "http".to_string(),
            parameters,
            timeout_seconds: 300,
            environment: HashMap::new(),
            working_directory: None,
            resource_limits: scheduler_core::ResourceLimits::default(),
        };

        self.execute_task(&context).await
    }

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
    async fn execute_task(&self, context: &TaskExecutionContextTrait) -> Result<TaskResult> {
        // 模拟执行时间
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

    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult> {
        // 模拟执行时间
        sleep(Duration::from_millis(self.execution_time_ms)).await;

        if self.should_succeed {
            Ok(TaskResult {
                success: true,
                output: Some("任务执行成功".to_string()),
                error_message: None,
                exit_code: Some(0),
                execution_time_ms: self.execution_time_ms,
            })
        } else {
            Ok(TaskResult {
                success: false,
                output: None,
                error_message: Some("任务执行失败".to_string()),
                exit_code: Some(1),
                execution_time_ms: self.execution_time_ms,
            })
        }
    }

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
        vec![self.name.clone()]
    }

    async fn cancel(&self, _task_run_id: i64) -> Result<()> {
        Ok(())
    }

    async fn is_running(&self, _task_run_id: i64) -> Result<bool> {
        Ok(false)
    }
}
