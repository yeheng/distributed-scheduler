use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use scheduler_core::models::*;
use scheduler_core::*;
use scheduler_infrastructure::{MetricsCollector, StructuredLogger, TaskTracer};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Worker服务构建器 - 使用依赖注入
pub struct WorkerServiceBuilder {
    worker_id: String,
    service_locator: Arc<ServiceLocator>,
    executor_registry: Option<Arc<dyn ExecutorRegistry>>,
    max_concurrent_tasks: usize,
    task_queue: String,
    status_queue: String,
    heartbeat_interval_seconds: u64,
    poll_interval_ms: u64,
    dispatcher_url: Option<String>,
    hostname: String,
    ip_address: String,
    metrics: Option<Arc<MetricsCollector>>,
}

impl WorkerServiceBuilder {
    /// 创建新的构建器
    pub fn new(
        worker_id: String,
        service_locator: Arc<ServiceLocator>,
        task_queue: String,
        status_queue: String,
    ) -> Self {
        Self {
            worker_id,
            service_locator,
            executor_registry: None,
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
            metrics: None,
        }
    }

    /// 设置执行器注册表
    pub fn with_executor_registry(mut self, registry: Arc<dyn ExecutorRegistry>) -> Self {
        self.executor_registry = Some(registry);
        self
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

    /// 设置指标收集器
    pub fn with_metrics(mut self, metrics: Arc<MetricsCollector>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// 构建WorkerService
    pub async fn build(self) -> SchedulerResult<WorkerService> {
        let executor_registry = self
            .executor_registry
            .ok_or_else(|| SchedulerError::Internal("Executor registry is required".to_string()))?;

        let metrics = self.metrics.unwrap_or_else(|| {
            Arc::new(MetricsCollector::new().unwrap_or_else(|e| {
                tracing::warn!("Failed to create metrics collector: {}", e);
                // Return a dummy metrics collector that does nothing
                MetricsCollector::new().unwrap()
            }))
        });

        Ok(WorkerService {
            worker_id: self.worker_id,
            service_locator: self.service_locator,
            executor_registry,
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
            metrics,
        })
    }
}

/// Worker服务实现 - 使用依赖注入和抽象
pub struct WorkerService {
    /// Worker唯一标识
    worker_id: String,

    /// 服务定位器
    service_locator: Arc<ServiceLocator>,

    /// 执行器注册表
    executor_registry: Arc<dyn ExecutorRegistry>,

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

    /// 指标收集器
    metrics: Arc<MetricsCollector>,
}

impl WorkerService {
    /// 创建构建器
    pub fn builder(
        worker_id: String,
        service_locator: Arc<ServiceLocator>,
        task_queue: String,
        status_queue: String,
    ) -> WorkerServiceBuilder {
        WorkerServiceBuilder::new(worker_id, service_locator, task_queue, status_queue)
    }

    /// 获取支持的任务类型列表
    pub async fn get_supported_task_types(&self) -> Vec<String> {
        self.executor_registry.list_executors().await
    }

    /// 获取消息队列
    async fn get_message_queue(&self) -> SchedulerResult<Arc<dyn MessageQueue>> {
        self.service_locator.message_queue().await
    }

    /// 处理任务执行消息 - 使用新的执行上下文
    async fn handle_task_execution(&self, message: TaskExecutionMessage) -> SchedulerResult<()> {
        let task_run_id = message.task_run_id;
        let task_type = message.task_type.clone();

        let span = TaskTracer::execute_task_span(
            task_run_id,
            message.task_id,
            &message.task_name,
            &task_type,
            &self.worker_id,
        );
        let _guard = span.enter();

        info!(
            "收到任务执行请求: task_run_id={}, task_type={}, timeout={}s",
            task_run_id, task_type, message.timeout_seconds
        );

        // 从执行器注册表获取合适的执行器
        let executor = match self.executor_registry.get(&task_type).await {
            Some(executor) => executor,
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

        // 创建TaskRun对象
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
            result: None,
            error_message: None,
            created_at: Utc::now(),
        };

        // 创建任务执行上下文
        let mut parameters = HashMap::new();
        if let Ok(params) =
            serde_json::from_value::<HashMap<String, serde_json::Value>>(message.parameters)
        {
            parameters = params;
        }

        let context = TaskExecutionContextTrait {
            task_run: task_run.clone(),
            task_type: task_type.clone(),
            parameters,
            timeout_seconds: 300,
            environment: HashMap::new(),
            working_directory: None,
            resource_limits: scheduler_core::ResourceLimits::default(),
        };

        // 添加到运行任务列表
        {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.insert(task_run_id, task_run);
        }

        // Log task execution start
        StructuredLogger::log_task_execution_start(
            task_run_id,
            message.task_id,
            &message.task_name,
            &task_type,
            &self.worker_id,
        );

        // 发送运行状态更新
        self.send_task_status_update(task_run_id, TaskRunStatus::Running, None, None)
            .await?;

        // 异步执行任务
        let worker_id = self.worker_id.clone();
        let running_tasks = Arc::clone(&self.running_tasks);
        let service_locator = Arc::clone(&self.service_locator);
        let status_queue = self.status_queue.clone();
        let timeout_duration = Duration::from_secs(message.timeout_seconds as u64);
        let task_name = message.task_name.clone();
        let metrics = Arc::clone(&self.metrics);

        tokio::spawn(async move {
            let execution_start = std::time::Instant::now();

            // 使用新的执行接口
            let result = tokio::select! {
                exec_result = executor.execute_task(&context) => {
                    match exec_result {
                        Ok(task_result) => {
                            if task_result.success {
                                // Log structured completion
                                StructuredLogger::log_task_execution_complete(
                                    task_run_id,
                                    &task_name,
                                    &task_type,
                                    &worker_id,
                                    true,
                                    task_result.execution_time_ms,
                                    None,
                                );

                                // Record successful task execution metrics
                                metrics.record_task_execution(
                                    &task_type,
                                    "completed",
                                    task_result.execution_time_ms as f64 / 1000.0
                                );

                                // Record tracing information
                                TaskTracer::record_task_result(
                                    true,
                                    task_result.execution_time_ms,
                                    None
                                );

                                (TaskRunStatus::Completed, Some(task_result), None)
                            } else {
                                // Log structured failure
                                StructuredLogger::log_task_execution_complete(
                                    task_run_id,
                                    &task_name,
                                    &task_type,
                                    &worker_id,
                                    false,
                                    task_result.execution_time_ms,
                                    task_result.error_message.as_deref(),
                                );

                                // Record failed task execution metrics
                                metrics.record_task_failure(&task_type, "execution_error");

                                // Record tracing information
                                TaskTracer::record_task_result(
                                    false,
                                    task_result.execution_time_ms,
                                    task_result.error_message.as_deref()
                                );

                                (
                                    TaskRunStatus::Failed,
                                    Some(task_result.clone()),
                                    task_result.error_message,
                                )
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("任务执行异常: {e}");

                            // Log structured error
                            StructuredLogger::log_system_error("worker", "execute_task", &e);

                            // Record task execution exception metrics
                            metrics.record_task_failure(&task_type, "execution_exception");

                            // Record tracing information
                            TaskTracer::record_error(&e);

                            (
                                TaskRunStatus::Failed,
                                None,
                                Some(error_msg),
                            )
                        }
                    }
                }
                _ = tokio::time::sleep(timeout_duration) => {
                    error!(
                        "任务执行超时: task_run_id={}, task_name={}, timeout={}s",
                        task_run_id, task_name, message.timeout_seconds
                    );

                    if let Err(e) = executor.cancel(task_run_id).await {
                        warn!("取消超时任务失败: task_run_id={}, error={}", task_run_id, e);
                    }

                    // Record task timeout metrics
                    metrics.record_task_failure(&task_type, "timeout");

                    // Record tracing information
                    TaskTracer::record_task_result(
                        false,
                        message.timeout_seconds as u64 * 1000,
                        Some(&format!("任务执行超时 ({}s)", message.timeout_seconds))
                    );

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

            // 发送状态更新
            let status_update = StatusUpdateMessage {
                task_run_id,
                status: result.0,
                worker_id: worker_id.clone(),
                result: result.1,
                error_message: result.2,
                timestamp: Utc::now(),
            };

            let message = Message::status_update(status_update);

            // 获取消息队列并发送状态更新
            if let Ok(message_queue) = service_locator.message_queue().await {
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
                                let delay = Duration::from_millis(100 * (1 << retry_count));
                                tokio::time::sleep(delay).await;
                            }
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
        control_message: TaskControlMessage,
    ) -> SchedulerResult<()> {
        let task_run_id = control_message.task_run_id;
        let action = control_message.action;

        info!(
            "收到任务控制请求: task_run_id={}, action={:?}, requester={}",
            task_run_id, action, control_message.requester
        );

        match action {
            TaskControlAction::Cancel => {
                WorkerServiceTrait::cancel_task(self, task_run_id).await?;
            }
            TaskControlAction::Pause => {
                warn!("暂停任务功能尚未实现: task_run_id={}", task_run_id);
            }
            TaskControlAction::Resume => {
                warn!("恢复任务功能尚未实现: task_run_id={}", task_run_id);
            }
            TaskControlAction::Restart => {
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
    ) -> SchedulerResult<()> {
        let status_update = StatusUpdateMessage {
            task_run_id,
            status,
            worker_id: self.worker_id.clone(),
            result,
            error_message,
            timestamp: Utc::now(),
        };

        let message = Message::status_update(status_update);
        let message_queue = self.get_message_queue().await?;

        message_queue
            .publish_message(&self.status_queue, &message)
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("发送状态更新失败: {e}")))?;

        debug!(
            "发送状态更新: task_run_id={}, status={:?}",
            task_run_id, status
        );
        Ok(())
    }

    /// 获取系统负载（简化实现）
    async fn get_system_load(&self) -> Option<f64> {
        None
    }

    /// 获取内存使用量（简化实现）
    async fn get_memory_usage(&self) -> Option<u64> {
        None
    }

    /// 向Dispatcher注册Worker
    #[cfg_attr(test, allow(dead_code))]
    pub async fn register_with_dispatcher(&self) -> SchedulerResult<()> {
        if let Some(ref dispatcher_url) = self.dispatcher_url {
            let worker_info = WorkerInfo {
                id: self.worker_id.clone(),
                hostname: self.hostname.clone(),
                ip_address: self.ip_address.clone(),
                supported_task_types: self.get_supported_task_types().await,
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
    pub async fn send_heartbeat_to_dispatcher(&self) -> SchedulerResult<()> {
        if let Some(ref dispatcher_url) = self.dispatcher_url {
            let current_task_count = self.get_current_task_count().await;

            // Update worker metrics
            self.metrics
                .update_worker_capacity(&self.worker_id, self.max_concurrent_tasks as f64);
            self.metrics
                .update_worker_load(&self.worker_id, current_task_count as f64);

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
    pub async fn unregister_from_dispatcher(&self) -> SchedulerResult<()> {
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
    async fn start_heartbeat_task(
        &self,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> SchedulerResult<()> {
        let mut heartbeat_interval = interval(Duration::from_secs(self.heartbeat_interval_seconds));

        loop {
            tokio::select! {
                _ = heartbeat_interval.tick() => {
                    if let Err(e) = self.send_heartbeat().await {
                        error!("发送消息队列心跳失败: {}", e);
                    }

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
    async fn start_task_polling(
        &self,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> SchedulerResult<()> {
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
    async fn start(&self) -> SchedulerResult<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(SchedulerError::Internal("Worker服务已在运行".to_string()));
        }

        info!("启动Worker服务: {}", self.worker_id);

        if let Err(e) = self.register_with_dispatcher().await {
            warn!("Worker注册失败，但继续启动服务: {}", e);
        }

        let (shutdown_tx, shutdown_rx1) = broadcast::channel(1);
        let shutdown_rx2 = shutdown_tx.subscribe();

        {
            let mut tx_guard = self.shutdown_tx.write().await;
            *tx_guard = Some(shutdown_tx);
        }

        let heartbeat_service = self.clone();
        tokio::spawn(async move {
            if let Err(e) = heartbeat_service.start_heartbeat_task(shutdown_rx1).await {
                error!("心跳任务失败: {}", e);
            }
        });

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

    async fn stop(&self) -> SchedulerResult<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }

        info!("停止Worker服务: {}", self.worker_id);

        {
            let tx_guard = self.shutdown_tx.read().await;
            if let Some(ref shutdown_tx) = *tx_guard {
                let _ = shutdown_tx.send(());
            }
        }

        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 30;

        while attempts < MAX_ATTEMPTS {
            let task_count = self.get_current_task_count().await;
            if task_count == 0 {
                break;
            }

            info!("等待 {} 个任务完成...", task_count);
            tokio::time::sleep(Duration::from_secs(1)).await;
            attempts += 1;
        }

        if let Err(e) = self.unregister_from_dispatcher().await {
            warn!("Worker注销失败: {}", e);
        }

        *is_running = false;
        info!("Worker服务已停止: {}", self.worker_id);
        Ok(())
    }

    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()> {
        let current_count = self.get_current_task_count().await;
        if current_count >= self.max_concurrent_tasks as i32 {
            return Ok(());
        }

        let message_queue = self.get_message_queue().await?;
        let messages = message_queue
            .consume_messages(&self.task_queue)
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("消费消息失败: {e}")))?;

        for message in messages {
            match message.message_type {
                MessageType::TaskExecution(task_message) => {
                    if let Err(e) = message_queue.ack_message(&message.id).await {
                        error!("确认消息失败: {}", e);
                        continue;
                    }

                    if let Err(e) = self.handle_task_execution(task_message).await {
                        error!("处理任务执行失败: {}", e);
                    }

                    let current_count = self.get_current_task_count().await;
                    if current_count >= self.max_concurrent_tasks as i32 {
                        break;
                    }
                }
                MessageType::TaskControl(control_message) => {
                    if let Err(e) = message_queue.ack_message(&message.id).await {
                        error!("确认控制消息失败: {}", e);
                        continue;
                    }

                    if let Err(e) = self.handle_task_control(control_message).await {
                        error!("处理任务控制失败: {}", e);
                    }
                }
                _ => {
                    if let Err(e) = message_queue.ack_message(&message.id).await {
                        error!("确认消息失败: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()> {
        let status_update = StatusUpdateMessage {
            task_run_id: update.task_run_id,
            status: update.status,
            worker_id: update.worker_id,
            result: None,
            error_message: update.error_message,
            timestamp: update.timestamp,
        };

        let message = Message::status_update(status_update);
        let message_queue = self.get_message_queue().await?;

        message_queue
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
        let has_executor = self.executor_registry.contains(task_type).await;
        current_count < self.max_concurrent_tasks as i32 && has_executor
    }

    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()> {
        let running_tasks_guard = self.running_tasks.read().await;

        if let Some(task_run) = running_tasks_guard.get(&task_run_id) {
            // 获取任务类型并查找执行器
            let task_info = task_run
                .result
                .as_ref()
                .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
                .unwrap_or_else(|| serde_json::json!({}));

            let task_type = task_info
                .get("task_type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");

            drop(running_tasks_guard);

            if let Some(executor) = self.executor_registry.get(task_type).await {
                info!(
                    "取消任务: task_run_id={}, task_type={}",
                    task_run_id, task_type
                );

                match executor.cancel(task_run_id).await {
                    Ok(_) => {
                        {
                            let mut running_tasks = self.running_tasks.write().await;
                            running_tasks.remove(&task_run_id);
                        }

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

    async fn get_running_tasks(&self) -> Vec<TaskRun> {
        let running_tasks = self.running_tasks.read().await;
        running_tasks.values().cloned().collect()
    }

    async fn is_task_running(&self, task_run_id: i64) -> bool {
        let running_tasks = self.running_tasks.read().await;
        running_tasks.contains_key(&task_run_id)
    }

    async fn send_heartbeat(&self) -> SchedulerResult<()> {
        let current_task_count = self.get_current_task_count().await;

        let heartbeat = WorkerHeartbeatMessage {
            worker_id: self.worker_id.clone(),
            current_task_count,
            system_load: self.get_system_load().await,
            memory_usage_mb: self.get_memory_usage().await,
            timestamp: Utc::now(),
        };

        let message = Message::worker_heartbeat(heartbeat);
        let message_queue = self.get_message_queue().await?;

        message_queue
            .publish_message("worker_heartbeat", &message)
            .await
            .map_err(|e| SchedulerError::MessageQueue(format!("发送心跳失败: {e}")))?;

        debug!(
            "发送心跳: worker_id={}, task_count={}",
            self.worker_id, current_task_count
        );
        Ok(())
    }
}

impl Clone for WorkerService {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id.clone(),
            service_locator: Arc::clone(&self.service_locator),
            executor_registry: Arc::clone(&self.executor_registry),
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
            metrics: Arc::clone(&self.metrics),
        }
    }
}
