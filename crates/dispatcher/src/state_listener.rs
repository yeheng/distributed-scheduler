use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::{debug, error, info, warn};

use scheduler_core::{
    models::{Message, MessageType, StatusUpdateMessage, TaskRunStatus},
    traits::{MessageQueue, StateListenerService, TaskRunRepository, WorkerRepository},
    SchedulerError, SchedulerResult,
};

use crate::retry_service::RetryService;

/// 状态监听器实现
pub struct StateListener {
    task_run_repo: Arc<dyn TaskRunRepository>,
    worker_repo: Arc<dyn WorkerRepository>,
    message_queue: Arc<dyn MessageQueue>,
    status_queue_name: String,
    heartbeat_queue_name: String,
    running: Arc<tokio::sync::RwLock<bool>>,
    retry_service: Option<Arc<dyn RetryService>>,
}

impl StateListener {
    /// 创建新的状态监听器
    pub fn new(
        task_run_repo: Arc<dyn TaskRunRepository>,
        worker_repo: Arc<dyn WorkerRepository>,
        message_queue: Arc<dyn MessageQueue>,
        status_queue_name: String,
        heartbeat_queue_name: String,
    ) -> Self {
        Self {
            task_run_repo,
            worker_repo,
            message_queue,
            status_queue_name,
            heartbeat_queue_name,
            running: Arc::new(tokio::sync::RwLock::new(false)),
            retry_service: None,
        }
    }

    /// 创建带重试服务的状态监听器
    pub fn with_retry_service(
        task_run_repo: Arc<dyn TaskRunRepository>,
        worker_repo: Arc<dyn WorkerRepository>,
        message_queue: Arc<dyn MessageQueue>,
        status_queue_name: String,
        heartbeat_queue_name: String,
        retry_service: Arc<dyn RetryService>,
    ) -> Self {
        Self {
            task_run_repo,
            worker_repo,
            message_queue,
            status_queue_name,
            heartbeat_queue_name,
            running: Arc::new(tokio::sync::RwLock::new(false)),
            retry_service: Some(retry_service),
        }
    }

    /// 停止监听器
    pub async fn stop(&self) -> SchedulerResult<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("状态监听器停止信号已发送");
        Ok(())
    }

    /// 检查监听器是否正在运行
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// 处理心跳消息
    async fn process_heartbeat_message(&self, message: &Message) -> SchedulerResult<()> {
        if let MessageType::WorkerHeartbeat(heartbeat) = &message.message_type {
            debug!("处理来自 Worker {} 的心跳消息", heartbeat.worker_id);

            // 更新Worker状态
            match self.worker_repo.get_by_id(&heartbeat.worker_id).await {
                Ok(Some(mut worker)) => {
                    // 更新现有Worker信息
                    worker.last_heartbeat = heartbeat.timestamp;
                    worker.current_task_count = heartbeat.current_task_count;
                    worker.status = scheduler_core::models::WorkerStatus::Alive;

                    self.worker_repo.update(&worker).await?;
                    debug!("更新了 Worker {} 的心跳信息", heartbeat.worker_id);
                }
                Ok(None) => {
                    warn!(
                        "收到未知 Worker {} 的心跳，Worker 可能需要重新注册",
                        heartbeat.worker_id
                    );
                }
                Err(e) => {
                    error!("获取 Worker {} 信息时出错: {}", heartbeat.worker_id, e);
                }
            }
        } else {
            warn!(
                "收到非心跳类型的消息，消息类型: {}",
                message.message_type_str()
            );
        }

        Ok(())
    }

    /// 处理消息队列中的消息
    async fn process_message(&self, message: &Message) -> SchedulerResult<()> {
        match &message.message_type {
            MessageType::StatusUpdate(status_msg) => {
                self.process_status_update_message(status_msg).await?;
            }
            MessageType::WorkerHeartbeat(_) => {
                self.process_heartbeat_message(message).await?;
            }
            _ => {
                debug!("忽略不支持的消息类型: {}", message.message_type_str());
            }
        }

        Ok(())
    }

    /// 处理状态更新消息的内部实现
    async fn process_status_update_message(
        &self,
        status_msg: &StatusUpdateMessage,
    ) -> SchedulerResult<()> {
        debug!(
            "处理任务运行 {} 的状态更新: {:?}",
            status_msg.task_run_id, status_msg.status
        );

        // 检查任务运行是否存在
        let task_run = self
            .task_run_repo
            .get_by_id(status_msg.task_run_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskRunNotFound {
                id: status_msg.task_run_id,
            })?;

        // 验证状态转换的合法性
        if !self.is_valid_status_transition(&task_run.status, &status_msg.status) {
            warn!(
                "任务运行 {} 的状态转换无效: {:?} -> {:?}",
                status_msg.task_run_id, task_run.status, status_msg.status
            );
            return Ok(()); // 不是错误，只是忽略无效的状态转换
        }

        // 根据状态更新相应的字段
        match status_msg.status {
            TaskRunStatus::Running => {
                self.task_run_repo
                    .update_status(
                        status_msg.task_run_id,
                        status_msg.status,
                        Some(&status_msg.worker_id),
                    )
                    .await?;

                info!(
                    "任务运行 {} 开始在 Worker {} 上执行",
                    status_msg.task_run_id, status_msg.worker_id
                );
            }
            TaskRunStatus::Completed => {
                // 提取任务结果
                let result_str = status_msg
                    .result
                    .as_ref()
                    .and_then(|r| serde_json::to_string(r).ok());

                self.task_run_repo
                    .update_result(status_msg.task_run_id, result_str.as_deref(), None)
                    .await?;

                self.task_run_repo
                    .update_status(status_msg.task_run_id, status_msg.status, None)
                    .await?;

                info!(
                    "任务运行 {} 在 Worker {} 上成功完成",
                    status_msg.task_run_id, status_msg.worker_id
                );
            }
            TaskRunStatus::Failed => {
                self.task_run_repo
                    .update_result(
                        status_msg.task_run_id,
                        None,
                        status_msg.error_message.as_deref(),
                    )
                    .await?;

                self.task_run_repo
                    .update_status(status_msg.task_run_id, status_msg.status, None)
                    .await?;

                warn!(
                    "任务运行 {} 在 Worker {} 上执行失败: {}",
                    status_msg.task_run_id,
                    status_msg.worker_id,
                    status_msg.error_message.as_deref().unwrap_or("未知错误")
                );

                // 处理失败任务的重试逻辑
                if let Some(retry_service) = &self.retry_service {
                    match retry_service
                        .handle_failed_task(status_msg.task_run_id)
                        .await
                    {
                        Ok(true) => {
                            info!("任务运行 {} 已安排重试", status_msg.task_run_id);
                        }
                        Ok(false) => {
                            debug!("任务运行 {} 不满足重试条件", status_msg.task_run_id);
                        }
                        Err(e) => {
                            error!("处理失败任务 {} 重试时出错: {}", status_msg.task_run_id, e);
                        }
                    }
                }
            }
            TaskRunStatus::Timeout => {
                self.task_run_repo
                    .update_result(status_msg.task_run_id, None, Some("任务执行超时"))
                    .await?;

                self.task_run_repo
                    .update_status(status_msg.task_run_id, status_msg.status, None)
                    .await?;

                warn!(
                    "任务运行 {} 在 Worker {} 上执行超时",
                    status_msg.task_run_id, status_msg.worker_id
                );

                // 处理超时任务的重试逻辑
                if let Some(retry_service) = &self.retry_service {
                    match retry_service
                        .handle_timeout_task(status_msg.task_run_id)
                        .await
                    {
                        Ok(true) => {
                            info!("超时任务运行 {} 已安排重试", status_msg.task_run_id);
                        }
                        Ok(false) => {
                            debug!("超时任务运行 {} 不满足重试条件", status_msg.task_run_id);
                        }
                        Err(e) => {
                            error!("处理超时任务 {} 重试时出错: {}", status_msg.task_run_id, e);
                        }
                    }
                }
            }
            // 其他状态的基本更新
            _ => {
                // 其他状态的基本更新
                self.task_run_repo
                    .update_status(status_msg.task_run_id, status_msg.status, None)
                    .await?;

                debug!(
                    "任务运行 {} 状态更新为 {:?}",
                    status_msg.task_run_id, status_msg.status
                );
            }
        }

        Ok(())
    }

    /// 验证状态转换是否合法
    fn is_valid_status_transition(
        &self,
        from_status: &TaskRunStatus,
        to_status: &TaskRunStatus,
    ) -> bool {
        use TaskRunStatus::*;

        match (from_status, to_status) {
            // 正常的状态转换
            (Pending, Dispatched) => true,
            (Dispatched, Running) => true,
            (Running, Completed) => true,
            (Running, Failed) => true,
            (Running, Timeout) => true,

            // 重试相关的转换
            (Failed, Pending) => true,     // 重试时回到Pending
            (Failed, Dispatched) => true,  // 重试时回到Dispatched
            (Timeout, Pending) => true,    // 超时后重试
            (Timeout, Dispatched) => true, // 超时后重试

            // 任务控制操作导致的转换
            (Pending, Failed) => true,    // 任务可以被取消或失败
            (Dispatched, Failed) => true, // 任务可以被取消或失败

            // 同状态更新（幂等操作）
            (status1, status2) if status1 == status2 => true,

            // 其他转换都是无效的
            _ => false,
        }
    }

    /// 监听指定队列的消息
    async fn listen_queue(&self, queue_name: &str) -> SchedulerResult<()> {
        info!("开始监听队列: {}", queue_name);

        loop {
            // 检查是否应该停止运行
            if !self.is_running().await {
                info!("收到停止信号，退出队列 {} 的监听", queue_name);
                break;
            }

            match self.message_queue.consume_messages(queue_name).await {
                Ok(messages) => {
                    if messages.is_empty() {
                        // 队列为空，等待一段时间后重试
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    } else {
                        // 处理所有消息
                        for message in messages {
                            if let Err(e) = self.process_message(&message).await {
                                error!("处理来自队列 {} 的消息时出错: {}", queue_name, e);

                                // 如果消息处理失败，可以考虑将消息放回队列或者记录到死信队列
                                // 这里简单记录错误并继续处理下一条消息
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("从队列 {} 消费消息时出错: {}", queue_name, e);

                    // 等待一段时间后重试，避免连续错误导致CPU占用过高
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl StateListenerService for StateListener {
    /// 监听状态更新
    async fn listen_for_updates(&self) -> SchedulerResult<()> {
        info!("启动状态监听服务");

        // 设置运行状态
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        // 同时监听状态更新队列和心跳队列
        let status_queue_name_clone = self.status_queue_name.clone();
        let heartbeat_queue_name_clone = self.heartbeat_queue_name.clone();

        // 克隆必要的字段来避免借用检查问题
        let task_run_repo = self.task_run_repo.clone();
        let worker_repo = self.worker_repo.clone();
        let message_queue = self.message_queue.clone();
        let running = self.running.clone();

        // 创建状态监听器的克隆
        let status_state_listener = StateListener {
            task_run_repo: task_run_repo.clone(),
            worker_repo: worker_repo.clone(),
            message_queue: message_queue.clone(),
            status_queue_name: status_queue_name_clone.clone(),
            heartbeat_queue_name: heartbeat_queue_name_clone.clone(),
            running: running.clone(),
            retry_service: self.retry_service.clone(),
        };

        let heartbeat_state_listener = StateListener {
            task_run_repo,
            worker_repo,
            message_queue,
            status_queue_name: status_queue_name_clone.clone(),
            heartbeat_queue_name: heartbeat_queue_name_clone.clone(),
            running,
            retry_service: self.retry_service.clone(),
        };

        let status_listener = {
            let queue_name = status_queue_name_clone.clone();
            tokio::spawn(async move {
                if let Err(e) = status_state_listener.listen_queue(&queue_name).await {
                    error!("状态更新队列监听出错: {}", e);
                }
            })
        };

        let heartbeat_listener = {
            let queue_name = heartbeat_queue_name_clone.clone();
            tokio::spawn(async move {
                if let Err(e) = heartbeat_state_listener.listen_queue(&queue_name).await {
                    error!("心跳队列监听出错: {}", e);
                }
            })
        };

        // 等待所有监听任务完成
        let (status_result, heartbeat_result) = tokio::join!(status_listener, heartbeat_listener);

        if let Err(e) = status_result {
            error!("状态监听任务执行出错: {}", e);
        }

        if let Err(e) = heartbeat_result {
            error!("心跳监听任务执行出错: {}", e);
        }

        info!("状态监听服务已停止");
        Ok(())
    }

    /// 处理状态更新
    async fn process_status_update(
        &self,
        task_run_id: i64,
        status: TaskRunStatus,
        result: Option<String>,
        error_message: Option<String>,
    ) -> SchedulerResult<()> {
        debug!("处理任务运行 {} 的状态更新: {:?}", task_run_id, status);

        // 检查任务运行是否存在
        let task_run = self
            .task_run_repo
            .get_by_id(task_run_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskRunNotFound { id: task_run_id })?;

        // 验证状态转换的合法性
        if !self.is_valid_status_transition(&task_run.status, &status) {
            warn!(
                "任务运行 {} 的状态转换无效: {:?} -> {:?}",
                task_run_id, task_run.status, status
            );
            return Ok(()); // 不是错误，只是忽略无效的状态转换
        }

        // 根据提供的参数更新状态
        match (result, error_message) {
            (Some(result_str), None) => {
                // 有结果，无错误 - 通常是成功完成
                self.task_run_repo
                    .update_result(task_run_id, Some(&result_str), None)
                    .await?;
                self.task_run_repo
                    .update_status(task_run_id, status, None)
                    .await?;
            }
            (None, Some(error_str)) => {
                // 无结果，有错误 - 通常是失败
                self.task_run_repo
                    .update_result(task_run_id, None, Some(&error_str))
                    .await?;
                self.task_run_repo
                    .update_status(task_run_id, status, None)
                    .await?;
            }
            (Some(result_str), Some(error_str)) => {
                // 既有结果又有错误 - 部分成功的情况
                warn!("任务运行 {} 同时包含结果和错误信息", task_run_id);
                self.task_run_repo
                    .update_result(task_run_id, Some(&result_str), Some(&error_str))
                    .await?;
                self.task_run_repo
                    .update_status(task_run_id, status, None)
                    .await?;
            }
            (None, None) => {
                // 既无结果也无错误 - 基本状态更新
                self.task_run_repo
                    .update_status(task_run_id, status, None)
                    .await?;
            }
        }

        info!("成功更新任务运行 {} 的状态为 {:?}", task_run_id, status);
        Ok(())
    }
}
