use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use scheduler_core::{
    Message, MessageQueue, MessageType, Result, SchedulerError, StatusUpdateMessage,
    TaskExecutionMessage, TaskExecutor, TaskResult, TaskRun, TaskRunStatus, TaskStatusUpdate,
    WorkerHeartbeatMessage, WorkerInfo, WorkerStatus,
};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Worker服务接口
#[async_trait]
pub trait WorkerServiceTrait: Send + Sync {
    /// 启动Worker服务
    async fn start(&self) -> Result<()>;

    /// 停止Worker服务
    async fn stop(&self) -> Result<()>;

    /// 轮询并执行任务
    async fn poll_and_execute_tasks(&self) -> Result<()>;

    /// 发送状态更新
    async fn send_status_update(&self, update: TaskStatusUpdate) -> Result<()>;

    /// 获取当前运行的任务数量
    async fn get_current_task_count(&self) -> i32;

    /// 检查是否可以接受新任务
    async fn can_accept_task(&self, task_type: &str) -> bool;

    /// 取消正在运行的任务
    async fn cancel_task(&self, task_run_id: i64) -> Result<()>;

    /// 获取正在运行的任务列表
    async fn get_running_tasks(&self) -> Vec<TaskRun>;

    /// 检查任务是否正在运行
    async fn is_task_running(&self, task_run_id: i64) -> bool;
}

/// Worker服务构建器
pub struct WorkerServiceBuilder {
    worker_id: String,
    message_queue: Arc<dyn MessageQueue>,
    executors: HashMap<String, Arc<dyn TaskExecutor>>,
    max_concurrent_tasks: usize,
    task_queue: String,
    status_queue: String,
    heartbeat_interval_seconds: u64,
    poll_interval_ms: u64,
    dispatcher_url: Option<String>,
    hostname: String,
    ip_address: String,
}

impl WorkerServiceBuilder {
    /// 创建新的构建器
    pub fn new(
        worker_id: String,
        message_queue: Arc<dyn MessageQueue>,
        task_queue: String,
        status_queue: String,
    ) -> Self {
        Self {
            worker_id,
            message_queue,
            executors: HashMap::new(),
            max_concurrent_tasks: 5,
            task_queue,
            status_queue,
            heartbeat_interval_seconds: 30,
            poll_interval_ms: 1000,
            dispatcher_url: None,
            hostname: hostname::get()
                .unwrap_or_else(|_| "unknown".into())
                .to_string_lossy()
                .to_string(),
            ip_address: "127.0.0.1".to_string(),
        }
    }

    /// 设置最大并发任务数
    pub fn max_concurrent_tasks(mut self, max_concurrent_tasks: usize) -> Self {
        self.max_concurrent_tasks = max_concurrent_tasks;
        self
    }

    /// 设置心跳间隔
    pub fn heartbeat_interval_seconds(mut self, heartbeat_interval_seconds: u64) -> Self {
        self.heartbeat_interval_seconds = heartbeat_interval_seconds;
        self
    }

    /// 设置轮询间隔
    pub fn poll_interval_ms(mut self, poll_interval_ms: u64) -> Self {
        self.poll_interval_ms = poll_interval_ms;
        self
    }

    /// 设置Dispatcher URL
    pub fn dispatcher_url(mut self, dispatcher_url: String) -> Self {
        self.dispatcher_url = Some(dispatcher_url);
        self
    }

    /// 设置主机名
    pub fn hostname(mut self, hostname: String) -> Self {
        self.hostname = hostname;
        self
    }

    /// 设置IP地址
    pub fn ip_address(mut self, ip_address: String) -> Self {
        self.ip_address = ip_address;
        self
    }

    /// 注册任务执行器
    pub fn register_executor(mut self, executor: Arc<dyn TaskExecutor>) -> Self {
        let name = executor.name().to_string();
        info!("注册任务执行器: {}", name);
        self.executors.insert(name, executor);
        self
    }

    /// 构建WorkerService
    pub fn build(self) -> WorkerService {
        WorkerService {
            worker_id: self.worker_id,
            message_queue: self.message_queue,
            executors: Arc::new(self.executors),
            max_concurrent_tasks: self.max_concurrent_tasks,
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
            task_queue: self.task_queue,
            status_queue: self.status_queue,
            heartbeat_interval_seconds: self.heartbeat_interval_seconds,
            poll_interval_ms: self.poll_interval_ms,
            dispatcher_url: self.dispatcher_url,
            hostname: self.hostname,
            ip_address: self.ip_address,
            shutdown_tx: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
            http_client: reqwest::Client::new(),
        }
    }
}

/// Worker服务实现
pub struct WorkerService {
    /// Worker唯一标识
    worker_id: String,

    /// 消息队列客户端
    message_queue: Arc<dyn MessageQueue>,

    /// 任务执行器映射
    executors: Arc<HashMap<String, Arc<dyn TaskExecutor>>>,

    /// 最大并发任务数
    max_concurrent_tasks: usize,

    /// 当前运行的任务
    running_tasks: Arc<RwLock<HashMap<i64, TaskRun>>>,

    /// 任务队列名称
    task_queue: String,

    /// 状态更新队列名称
    status_queue: String,

    /// 心跳间隔（秒）
    heartbeat_interval_seconds: u64,

    /// 任务轮询间隔（毫秒）
    poll_interval_ms: u64,

    /// Dispatcher URL
    dispatcher_url: Option<String>,

    /// 主机名
    hostname: String,

    /// IP地址
    ip_address: String,

    /// 停止信号
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,

    /// 是否正在运行
    is_running: Arc<RwLock<bool>>,

    /// HTTP客户端
    http_client: reqwest::Client,
}

impl WorkerService {
    /// 创建构建器
    pub fn builder(
        worker_id: String,
        message_queue: Arc<dyn MessageQueue>,
        task_queue: String,
        status_queue: String,
    ) -> WorkerServiceBuilder {
        WorkerServiceBuilder::new(worker_id, message_queue, task_queue, status_queue)
    }

    /// 获取支持的任务类型列表
    pub fn get_supported_task_types(&self) -> Vec<String> {
        self.executors.keys().cloned().collect()
    }

    /// 获取Dispatcher URL（用于测试）
    #[cfg(test)]
    pub fn get_dispatcher_url(&self) -> &Option<String> {
        &self.dispatcher_url
    }

    /// 获取主机名（用于测试）
    #[cfg(test)]
    pub fn get_hostname(&self) -> &str {
        &self.hostname
    }

    /// 获取IP地址（用于测试）
    #[cfg(test)]
    pub fn get_ip_address(&self) -> &str {
        &self.ip_address
    }

    /// 处理任务执行消息
    async fn handle_task_execution(&self, message: TaskExecutionMessage) -> Result<()> {
        let task_run_id = message.task_run_id;
        let task_type = &message.task_type;

        info!(
            "收到任务执行请求: task_run_id={}, task_type={}, timeout={}s",
            task_run_id, task_type, message.timeout_seconds
        );

        // 检查是否有合适的执行器
        let executor = match self.executors.get(task_type) {
            Some(executor) => Arc::clone(executor),
            None => {
                error!("未找到支持任务类型 '{}' 的执行器", task_type);
                self.send_task_status_update(
                    task_run_id,
                    TaskRunStatus::Failed,
                    None,
                    Some(format!("不支持的任务类型: {task_type}")),
                )
                .await?;
                return Ok(());
            }
        };

        // 检查并发限制
        let current_count = self.get_current_task_count().await;
        if current_count >= self.max_concurrent_tasks as i32 {
            warn!(
                "达到最大并发限制 {}, 拒绝任务 {}",
                self.max_concurrent_tasks, task_run_id
            );
            self.send_task_status_update(
                task_run_id,
                TaskRunStatus::Failed,
                None,
                Some("Worker达到最大并发限制".to_string()),
            )
            .await?;
            return Ok(());
        }

        // 创建TaskRun对象，在result中存储完整的任务信息
        let task_info = serde_json::json!({
            "task_type": message.task_type,
            "parameters": message.parameters
        });

        let task_run = TaskRun {
            id: task_run_id,
            task_id: message.task_id,
            status: TaskRunStatus::Running,
            worker_id: Some(self.worker_id.clone()),
            retry_count: message.retry_count,
            shard_index: message.shard_index,
            shard_total: message.shard_total,
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
            result: Some(task_info.to_string()),
            error_message: None,
            created_at: Utc::now(),
        };

        // 添加到运行任务列表
        {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.insert(task_run_id, task_run.clone());
        }

        // 发送运行状态更新
        self.send_task_status_update(task_run_id, TaskRunStatus::Running, None, None)
            .await?;

        // 异步执行任务，增强超时和错误处理
        let worker_id = self.worker_id.clone();
        let running_tasks = Arc::clone(&self.running_tasks);
        let message_queue = Arc::clone(&self.message_queue);
        let status_queue = self.status_queue.clone();
        let timeout_duration = Duration::from_secs(message.timeout_seconds as u64);
        let task_name = message.task_name.clone();

        tokio::spawn(async move {
            let execution_start = std::time::Instant::now();

            // 使用tokio::select!来处理超时和取消
            let result = tokio::select! {
                // 正常执行任务
                exec_result = executor.execute(&task_run) => {
                    match exec_result {
                        Ok(task_result) => {
                            if task_result.success {
                                info!(
                                    "任务执行成功: task_run_id={}, task_name={}, duration={}ms",
                                    task_run_id, task_name, task_result.execution_time_ms
                                );
                                (TaskRunStatus::Completed, Some(task_result), None)
                            } else {
                                warn!(
                                    "任务执行失败: task_run_id={}, task_name={}, error={:?}",
                                    task_run_id, task_name, task_result.error_message
                                );
                                (
                                    TaskRunStatus::Failed,
                                    Some(task_result.clone()),
                                    task_result.error_message,
                                )
                            }
                        }
                        Err(e) => {
                            error!(
                                "任务执行异常: task_run_id={}, task_name={}, error={}",
                                task_run_id, task_name, e
                            );
                            (
                                TaskRunStatus::Failed,
                                None,
                                Some(format!("任务执行异常: {e}")),
                            )
                        }
                    }
                }
                // 超时处理
                _ = tokio::time::sleep(timeout_duration) => {
                    error!(
                        "任务执行超时: task_run_id={}, task_name={}, timeout={}s",
                        task_run_id, task_name, message.timeout_seconds
                    );

                    // 尝试取消任务
                    if let Err(e) = executor.cancel(task_run_id).await {
                        warn!("取消超时任务失败: task_run_id={}, error={}", task_run_id, e);
                    }

                    (
                        TaskRunStatus::Timeout,
                        None,
                        Some(format!("任务执行超时 ({}s)", message.timeout_seconds)),
                    )
                }
            };

            let execution_duration = execution_start.elapsed();

            // 从运行任务列表中移除
            {
                let mut running_tasks = running_tasks.write().await;
                running_tasks.remove(&task_run_id);
            }

            // 发送状态更新，增加重试机制
            let status_update = StatusUpdateMessage {
                task_run_id,
                status: result.0,
                worker_id: worker_id.clone(),
                result: result.1,
                error_message: result.2,
                timestamp: Utc::now(),
            };

            let message = Message::status_update(status_update);

            // 重试发送状态更新
            let mut retry_count = 0;
            const MAX_STATUS_UPDATE_RETRIES: u32 = 3;

            while retry_count < MAX_STATUS_UPDATE_RETRIES {
                match message_queue.publish_message(&status_queue, &message).await {
                    Ok(_) => {
                        debug!(
                            "状态更新发送成功: task_run_id={}, status={:?}, retry_count={}",
                            task_run_id, result.0, retry_count
                        );
                        break;
                    }
                    Err(e) => {
                        retry_count += 1;
                        error!(
                            "发送状态更新失败 (重试 {}/{}): task_run_id={}, error={}",
                            retry_count, MAX_STATUS_UPDATE_RETRIES, task_run_id, e
                        );

                        if retry_count < MAX_STATUS_UPDATE_RETRIES {
                            // 指数退避重试
                            let delay = Duration::from_millis(100 * (1 << retry_count));
                            tokio::time::sleep(delay).await;
                        }
                    }
                }
            }

            info!(
                "任务执行完成: task_run_id={}, task_name={}, status={:?}, total_duration={}ms",
                task_run_id,
                task_name,
                result.0,
                execution_duration.as_millis()
            );
        });

        Ok(())
    }

    /// 处理任务控制消息
    async fn handle_task_control(
        &self,
        control_message: scheduler_core::TaskControlMessage,
    ) -> Result<()> {
        let task_run_id = control_message.task_run_id;
        let action = control_message.action;

        info!(
            "收到任务控制请求: task_run_id={}, action={:?}, requester={}",
            task_run_id, action, control_message.requester
        );

        match action {
            scheduler_core::TaskControlAction::Cancel => {
                WorkerServiceTrait::cancel_task(self, task_run_id).await?;
            }
            scheduler_core::TaskControlAction::Pause => {
                // 暂停功能需要执行器支持，这里记录日志
                warn!("暂停任务功能尚未实现: task_run_id={}", task_run_id);
            }
            scheduler_core::TaskControlAction::Resume => {
                // 恢复功能需要执行器支持，这里记录日志
                warn!("恢复任务功能尚未实现: task_run_id={}", task_run_id);
            }
            scheduler_core::TaskControlAction::Restart => {
                // 重启功能需要重新创建任务执行，这里记录日志
                warn!("重启任务功能尚未实现: task_run_id={}", task_run_id);
            }
        }

        Ok(())
    }

    /// 发送任务状态更新
    async fn send_task_status_update(
        &self,
        task_run_id: i64,
        status: TaskRunStatus,
        result: Option<TaskResult>,
        error_message: Option<String>,
    ) -> Result<()> {
        let status_update = StatusUpdateMessage {
            task_run_id,
            status,
            worker_id: self.worker_id.clone(),
            result,
            error_message,
            timestamp: Utc::now(),
        };

        let message = Message::status_update(status_update);
        self.message_queue
            .publish_message(&self.status_queue, &message)
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("发送状态更新失败: {e}")))?;

        debug!(
            "发送状态更新: task_run_id={}, status={:?}",
            task_run_id, status
        );
        Ok(())
    }

    /// 发送心跳消息
    async fn send_heartbeat(&self) -> Result<()> {
        let current_task_count = self.get_current_task_count().await;

        let heartbeat = WorkerHeartbeatMessage {
            worker_id: self.worker_id.clone(),
            current_task_count,
            system_load: self.get_system_load().await,
            memory_usage_mb: self.get_memory_usage().await,
            timestamp: Utc::now(),
        };

        let message = Message::worker_heartbeat(heartbeat);
        self.message_queue
            .publish_message("worker_heartbeat", &message)
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("发送心跳失败: {e}")))?;

        debug!(
            "发送心跳: worker_id={}, task_count={}",
            self.worker_id, current_task_count
        );
        Ok(())
    }

    /// 获取系统负载（简化实现）
    async fn get_system_load(&self) -> Option<f64> {
        // TODO: 实现真实的系统负载获取
        None
    }

    /// 获取内存使用量（简化实现）
    async fn get_memory_usage(&self) -> Option<u64> {
        // TODO: 实现真实的内存使用量获取
        None
    }

    /// 向Dispatcher注册Worker
    #[cfg_attr(test, allow(dead_code))]
    pub async fn register_with_dispatcher(&self) -> Result<()> {
        if let Some(ref dispatcher_url) = self.dispatcher_url {
            let worker_info = WorkerInfo {
                id: self.worker_id.clone(),
                hostname: self.hostname.clone(),
                ip_address: self.ip_address.clone(),
                supported_task_types: self.get_supported_task_types(),
                max_concurrent_tasks: self.max_concurrent_tasks as i32,
                current_task_count: 0,
                status: WorkerStatus::Alive,
                last_heartbeat: Utc::now(),
                registered_at: Utc::now(),
            };

            let registration_url = format!("{dispatcher_url}/api/workers");

            match self
                .http_client
                .post(&registration_url)
                .json(&worker_info)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        info!(
                            "Worker {} 成功注册到 Dispatcher: {}",
                            self.worker_id, dispatcher_url
                        );
                        Ok(())
                    } else {
                        let error_msg = format!(
                            "Worker注册失败，状态码: {}, URL: {}",
                            response.status(),
                            registration_url
                        );
                        error!("{}", error_msg);
                        Err(SchedulerError::Internal(error_msg))
                    }
                }
                Err(e) => {
                    let error_msg = format!("Worker注册请求失败: {e}, URL: {registration_url}");
                    error!("{}", error_msg);
                    Err(SchedulerError::Internal(error_msg))
                }
            }
        } else {
            debug!("未配置Dispatcher URL，跳过Worker注册");
            Ok(())
        }
    }

    /// 向Dispatcher发送心跳
    #[cfg_attr(test, allow(dead_code))]
    pub async fn send_heartbeat_to_dispatcher(&self) -> Result<()> {
        if let Some(ref dispatcher_url) = self.dispatcher_url {
            let current_task_count = self.get_current_task_count().await;

            let heartbeat_data = serde_json::json!({
                "current_task_count": current_task_count,
                "system_load": self.get_system_load().await,
                "memory_usage_mb": self.get_memory_usage().await,
                "timestamp": Utc::now()
            });

            let heartbeat_url = format!(
                "{}/api/workers/{}/heartbeat",
                dispatcher_url, self.worker_id
            );

            match self
                .http_client
                .post(&heartbeat_url)
                .json(&heartbeat_data)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!("心跳发送成功: worker_id={}", self.worker_id);
                        Ok(())
                    } else {
                        let error_msg = format!(
                            "心跳发送失败，状态码: {}, URL: {}",
                            response.status(),
                            heartbeat_url
                        );
                        warn!("{}", error_msg);
                        Err(SchedulerError::Internal(error_msg))
                    }
                }
                Err(e) => {
                    let error_msg = format!("心跳请求失败: {e}, URL: {heartbeat_url}");
                    warn!("{}", error_msg);
                    Err(SchedulerError::Internal(error_msg))
                }
            }
        } else {
            Ok(())
        }
    }

    /// 向Dispatcher注销Worker
    #[cfg_attr(test, allow(dead_code))]
    pub async fn unregister_from_dispatcher(&self) -> Result<()> {
        if let Some(ref dispatcher_url) = self.dispatcher_url {
            let unregister_url = format!("{}/api/workers/{}", dispatcher_url, self.worker_id);

            match self.http_client.delete(&unregister_url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Worker {} 成功从 Dispatcher 注销", self.worker_id);
                        Ok(())
                    } else {
                        let error_msg = format!(
                            "Worker注销失败，状态码: {}, URL: {}",
                            response.status(),
                            unregister_url
                        );
                        warn!("{}", error_msg);
                        Err(SchedulerError::Internal(error_msg))
                    }
                }
                Err(e) => {
                    let error_msg = format!("Worker注销请求失败: {e}, URL: {unregister_url}");
                    warn!("{}", error_msg);
                    Err(SchedulerError::Internal(error_msg))
                }
            }
        } else {
            Ok(())
        }
    }

    /// 启动心跳任务
    async fn start_heartbeat_task(&self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        let mut heartbeat_interval = interval(Duration::from_secs(self.heartbeat_interval_seconds));

        loop {
            tokio::select! {
                _ = heartbeat_interval.tick() => {
                    // 发送消息队列心跳
                    if let Err(e) = self.send_heartbeat().await {
                        error!("发送消息队列心跳失败: {}", e);
                    }

                    // 发送HTTP心跳到Dispatcher
                    if let Err(e) = self.send_heartbeat_to_dispatcher().await {
                        error!("发送HTTP心跳失败: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("心跳任务收到停止信号");
                    break;
                }
            }
        }

        Ok(())
    }

    /// 启动任务轮询
    async fn start_task_polling(&self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        let mut poll_interval = interval(Duration::from_millis(self.poll_interval_ms));

        loop {
            tokio::select! {
                _ = poll_interval.tick() => {
                    if let Err(e) = self.poll_and_execute_tasks().await {
                        error!("任务轮询失败: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("任务轮询收到停止信号");
                    break;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl WorkerServiceTrait for WorkerService {
    async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(SchedulerError::Internal("Worker服务已在运行".to_string()));
        }

        info!("启动Worker服务: {}", self.worker_id);

        // 向Dispatcher注册Worker
        if let Err(e) = self.register_with_dispatcher().await {
            warn!("Worker注册失败，但继续启动服务: {}", e);
        }

        // 创建停止信号通道
        let (shutdown_tx, shutdown_rx1) = broadcast::channel(1);
        let shutdown_rx2 = shutdown_tx.subscribe();

        // 保存shutdown_tx
        {
            let mut tx_guard = self.shutdown_tx.write().await;
            *tx_guard = Some(shutdown_tx);
        }

        // 启动心跳任务
        let heartbeat_service = self.clone();
        tokio::spawn(async move {
            if let Err(e) = heartbeat_service.start_heartbeat_task(shutdown_rx1).await {
                error!("心跳任务失败: {}", e);
            }
        });

        // 启动任务轮询
        let polling_service = self.clone();
        tokio::spawn(async move {
            if let Err(e) = polling_service.start_task_polling(shutdown_rx2).await {
                error!("任务轮询失败: {}", e);
            }
        });

        *is_running = true;
        info!("Worker服务启动成功: {}", self.worker_id);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }

        info!("停止Worker服务: {}", self.worker_id);

        // 发送停止信号
        {
            let tx_guard = self.shutdown_tx.read().await;
            if let Some(ref shutdown_tx) = *tx_guard {
                let _ = shutdown_tx.send(());
            }
        }

        // 等待所有运行中的任务完成
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 30; // 最多等待30秒

        while attempts < MAX_ATTEMPTS {
            let task_count = self.get_current_task_count().await;
            if task_count == 0 {
                break;
            }

            info!("等待 {} 个任务完成...", task_count);
            tokio::time::sleep(Duration::from_secs(1)).await;
            attempts += 1;
        }

        // 从Dispatcher注销Worker
        if let Err(e) = self.unregister_from_dispatcher().await {
            warn!("Worker注销失败: {}", e);
        }

        *is_running = false;
        info!("Worker服务已停止: {}", self.worker_id);
        Ok(())
    }

    async fn poll_and_execute_tasks(&self) -> Result<()> {
        // 检查是否可以接受新任务
        let current_count = self.get_current_task_count().await;
        if current_count >= self.max_concurrent_tasks as i32 {
            return Ok(());
        }

        // 从消息队列获取任务
        let messages = self
            .message_queue
            .consume_messages(&self.task_queue)
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("消费消息失败: {e}")))?;

        for message in messages {
            match message.message_type {
                MessageType::TaskExecution(task_message) => {
                    // 确认消息
                    if let Err(e) = self.message_queue.ack_message(&message.id).await {
                        error!("确认消息失败: {}", e);
                        continue;
                    }

                    // 处理任务执行
                    if let Err(e) = self.handle_task_execution(task_message).await {
                        error!("处理任务执行失败: {}", e);
                    }

                    // 检查并发限制
                    let current_count = self.get_current_task_count().await;
                    if current_count >= self.max_concurrent_tasks as i32 {
                        break;
                    }
                }
                MessageType::TaskControl(control_message) => {
                    // 确认消息
                    if let Err(e) = self.message_queue.ack_message(&message.id).await {
                        error!("确认控制消息失败: {}", e);
                        continue;
                    }

                    // 处理任务控制
                    if let Err(e) = self.handle_task_control(control_message).await {
                        error!("处理任务控制失败: {}", e);
                    }
                }
                _ => {
                    // 其他类型的消息，确认但不处理
                    if let Err(e) = self.message_queue.ack_message(&message.id).await {
                        error!("确认消息失败: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn send_status_update(&self, update: TaskStatusUpdate) -> Result<()> {
        let status_update = StatusUpdateMessage {
            task_run_id: update.task_run_id,
            status: update.status,
            worker_id: update.worker_id,
            result: None,
            error_message: update.error_message,
            timestamp: update.timestamp,
        };

        let message = Message::status_update(status_update);
        self.message_queue
            .publish_message(&self.status_queue, &message)
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("发送状态更新失败: {e}")))?;

        Ok(())
    }

    async fn get_current_task_count(&self) -> i32 {
        let running_tasks = self.running_tasks.read().await;
        running_tasks.len() as i32
    }

    async fn can_accept_task(&self, task_type: &str) -> bool {
        let current_count = self.get_current_task_count().await;
        current_count < self.max_concurrent_tasks as i32 && self.executors.contains_key(task_type)
    }

    /// 取消正在运行的任务
    async fn cancel_task(&self, task_run_id: i64) -> Result<()> {
        let running_tasks_guard = self.running_tasks.read().await;

        if let Some(task_run) = running_tasks_guard.get(&task_run_id) {
            // 尝试从任务参数中获取任务类型
            let task_type_str = task_run
                .result
                .as_ref()
                .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
                .and_then(|v| {
                    v.get("task_type")
                        .and_then(|t| t.as_str().map(|s| s.to_string()))
                })
                .unwrap_or_else(|| "unknown".to_string());

            // 释放读锁
            drop(running_tasks_guard);

            if let Some(executor) = self.executors.get(&task_type_str) {
                info!(
                    "取消任务: task_run_id={}, task_type={}",
                    task_run_id, task_type_str
                );

                match executor.cancel(task_run_id).await {
                    Ok(_) => {
                        // 从运行任务列表中移除
                        {
                            let mut running_tasks = self.running_tasks.write().await;
                            running_tasks.remove(&task_run_id);
                        }

                        // 发送取消状态更新
                        self.send_task_status_update(
                            task_run_id,
                            TaskRunStatus::Failed,
                            None,
                            Some("任务被取消".to_string()),
                        )
                        .await?;

                        info!("任务取消成功: task_run_id={}", task_run_id);
                    }
                    Err(e) => {
                        error!("任务取消失败: task_run_id={}, error={}", task_run_id, e);
                        return Err(e);
                    }
                }
            } else {
                warn!(
                    "未找到任务执行器，无法取消任务: task_run_id={}",
                    task_run_id
                );
            }
        } else {
            warn!("任务不在运行列表中: task_run_id={}", task_run_id);
        }

        Ok(())
    }

    /// 获取正在运行的任务列表
    async fn get_running_tasks(&self) -> Vec<TaskRun> {
        let running_tasks = self.running_tasks.read().await;
        running_tasks.values().cloned().collect()
    }

    /// 检查任务是否正在运行
    async fn is_task_running(&self, task_run_id: i64) -> bool {
        let running_tasks = self.running_tasks.read().await;
        running_tasks.contains_key(&task_run_id)
    }
}

// 实现Clone trait以支持在异步任务中使用
impl Clone for WorkerService {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id.clone(),
            message_queue: Arc::clone(&self.message_queue),
            executors: Arc::clone(&self.executors),
            max_concurrent_tasks: self.max_concurrent_tasks,
            running_tasks: Arc::clone(&self.running_tasks),
            task_queue: self.task_queue.clone(),
            status_queue: self.status_queue.clone(),
            heartbeat_interval_seconds: self.heartbeat_interval_seconds,
            poll_interval_ms: self.poll_interval_ms,
            dispatcher_url: self.dispatcher_url.clone(),
            hostname: self.hostname.clone(),
            ip_address: self.ip_address.clone(),
            shutdown_tx: Arc::clone(&self.shutdown_tx),
            is_running: Arc::clone(&self.is_running),
            http_client: self.http_client.clone(),
        }
    }
}
