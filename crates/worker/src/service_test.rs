use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use scheduler_core::{
    Message, MessageQueue, MessageType, Result, TaskExecutor, TaskResult, TaskRun, TaskStatusUpdate,
};
use tokio::sync::RwLock;
use tokio::time::sleep;

use super::{WorkerService, WorkerServiceTrait};

/// 模拟消息队列实现
#[derive(Debug)]
struct MockMessageQueue {
    messages: Arc<RwLock<Vec<Message>>>,
}

impl MockMessageQueue {
    fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn add_message(&self, message: Message) {
        let mut messages = self.messages.write().await;
        messages.push(message);
    }

    async fn get_messages(&self) -> Vec<Message> {
        let messages = self.messages.read().await;
        messages.clone()
    }
}

#[async_trait]
impl MessageQueue for MockMessageQueue {
    async fn publish_message(&self, _queue: &str, message: &Message) -> Result<()> {
        self.add_message(message.clone()).await;
        Ok(())
    }

    async fn consume_messages(&self, _queue: &str) -> Result<Vec<Message>> {
        let mut messages = self.messages.write().await;
        let result = messages.clone();
        messages.clear();
        Ok(result)
    }

    async fn ack_message(&self, _message_id: &str) -> Result<()> {
        Ok(())
    }

    async fn nack_message(&self, _message_id: &str, _requeue: bool) -> Result<()> {
        Ok(())
    }

    async fn create_queue(&self, _queue: &str, _durable: bool) -> Result<()> {
        Ok(())
    }

    async fn delete_queue(&self, _queue: &str) -> Result<()> {
        Ok(())
    }

    async fn get_queue_size(&self, _queue: &str) -> Result<u32> {
        let messages = self.messages.read().await;
        Ok(messages.len() as u32)
    }

    async fn purge_queue(&self, _queue: &str) -> Result<()> {
        let mut messages = self.messages.write().await;
        messages.clear();
        Ok(())
    }
}

/// 模拟任务执行器
#[derive(Debug)]
struct MockTaskExecutor {
    name: String,
    should_succeed: bool,
    execution_time_ms: u64,
}

impl MockTaskExecutor {
    fn new(name: String, should_succeed: bool, execution_time_ms: u64) -> Self {
        Self {
            name,
            should_succeed,
            execution_time_ms,
        }
    }
}

#[async_trait]
impl TaskExecutor for MockTaskExecutor {
    async fn execute(&self, _task_run: &TaskRun) -> Result<TaskResult> {
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

    async fn cancel(&self, _task_run_id: i64) -> Result<()> {
        Ok(())
    }

    async fn is_running(&self, _task_run_id: i64) -> Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_core::{TaskExecutionMessage, TaskRunStatus};
    use serde_json::json;

    #[tokio::test]
    async fn test_worker_service_creation() {
        let message_queue = Arc::new(MockMessageQueue::new());
        let worker_service = WorkerService::builder(
            "test-worker".to_string(),
            message_queue,
            "task_queue".to_string(),
            "status_queue".to_string(),
        )
        .max_concurrent_tasks(3)
        .heartbeat_interval_seconds(10)
        .poll_interval_ms(500)
        .build();

        assert_eq!(worker_service.get_supported_task_types().len(), 0);
        assert_eq!(worker_service.get_current_task_count().await, 0);
    }

    #[tokio::test]
    async fn test_worker_service_with_executor() {
        let message_queue = Arc::new(MockMessageQueue::new());
        let executor = Arc::new(MockTaskExecutor::new("shell".to_string(), true, 100));

        let worker_service = WorkerService::builder(
            "test-worker".to_string(),
            message_queue,
            "task_queue".to_string(),
            "status_queue".to_string(),
        )
        .register_executor(executor)
        .build();

        let supported_types = worker_service.get_supported_task_types();
        assert_eq!(supported_types.len(), 1);
        assert!(supported_types.contains(&"shell".to_string()));
        assert!(worker_service.can_accept_task("shell").await);
        assert!(!worker_service.can_accept_task("http").await);
    }

    #[tokio::test]
    async fn test_worker_service_start_stop() {
        let message_queue = Arc::new(MockMessageQueue::new());
        let worker_service = WorkerService::builder(
            "test-worker".to_string(),
            message_queue,
            "task_queue".to_string(),
            "status_queue".to_string(),
        )
        .build();

        // 测试启动
        assert!(worker_service.start().await.is_ok());

        // 等待一小段时间让服务启动
        sleep(Duration::from_millis(100)).await;

        // 测试停止
        assert!(worker_service.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_worker_service_task_execution() {
        let message_queue = Arc::new(MockMessageQueue::new());
        let executor = Arc::new(MockTaskExecutor::new("shell".to_string(), true, 50));

        let worker_service = WorkerService::builder(
            "test-worker".to_string(),
            message_queue.clone() as Arc<dyn MessageQueue>,
            "task_queue".to_string(),
            "status_queue".to_string(),
        )
        .register_executor(executor)
        .build();

        // 创建任务执行消息
        let task_execution = TaskExecutionMessage {
            task_run_id: 123,
            task_id: 456,
            task_name: "test_task".to_string(),
            task_type: "shell".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: 30,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution);
        message_queue.add_message(message).await;

        // 轮询并执行任务
        assert!(worker_service.poll_and_execute_tasks().await.is_ok());

        // 等待任务执行完成
        sleep(Duration::from_millis(200)).await;

        // 检查状态更新消息
        let messages = message_queue.get_messages().await;
        assert!(!messages.is_empty());

        // 应该有状态更新消息
        let status_messages: Vec<_> = messages
            .iter()
            .filter(|m| matches!(m.message_type, MessageType::StatusUpdate(_)))
            .collect();
        assert!(!status_messages.is_empty());
    }

    #[tokio::test]
    async fn test_worker_service_concurrent_limit() {
        let message_queue = Arc::new(MockMessageQueue::new());
        let executor = Arc::new(MockTaskExecutor::new("shell".to_string(), true, 200));

        let worker_service = WorkerService::builder(
            "test-worker".to_string(),
            message_queue.clone() as Arc<dyn MessageQueue>,
            "task_queue".to_string(),
            "status_queue".to_string(),
        )
        .max_concurrent_tasks(2) // 限制为2个并发任务
        .register_executor(executor)
        .build();

        // 创建3个任务执行消息
        for i in 1..=3 {
            let task_execution = TaskExecutionMessage {
                task_run_id: i,
                task_id: i + 100,
                task_name: format!("test_task_{}", i),
                task_type: "shell".to_string(),
                parameters: json!({"command": format!("echo task{}", i)}),
                timeout_seconds: 30,
                retry_count: 0,
                shard_index: None,
                shard_total: None,
            };

            let message = Message::task_execution(task_execution);
            message_queue.add_message(message).await;
        }

        // 轮询并执行任务
        assert!(worker_service.poll_and_execute_tasks().await.is_ok());

        // 检查当前运行的任务数量应该不超过限制
        let current_count = worker_service.get_current_task_count().await;
        assert!(current_count <= 2);

        // 等待任务执行完成
        sleep(Duration::from_millis(300)).await;

        // 最终所有任务都应该完成
        let final_count = worker_service.get_current_task_count().await;
        assert_eq!(final_count, 0);
    }

    #[tokio::test]
    async fn test_worker_service_status_update() {
        let message_queue = Arc::new(MockMessageQueue::new());
        let worker_service = WorkerService::builder(
            "test-worker".to_string(),
            message_queue.clone() as Arc<dyn MessageQueue>,
            "task_queue".to_string(),
            "status_queue".to_string(),
        )
        .build();

        let status_update = TaskStatusUpdate {
            task_run_id: 789,
            status: TaskRunStatus::Completed,
            worker_id: "test-worker".to_string(),
            result: Some("任务完成".to_string()),
            error_message: None,
            timestamp: Utc::now(),
        };

        assert!(worker_service
            .send_status_update(status_update)
            .await
            .is_ok());

        let messages = message_queue.get_messages().await;
        assert_eq!(messages.len(), 1);

        if let MessageType::StatusUpdate(status_msg) = &messages[0].message_type {
            assert_eq!(status_msg.task_run_id, 789);
            assert_eq!(status_msg.status, TaskRunStatus::Completed);
            assert_eq!(status_msg.worker_id, "test-worker");
        } else {
            panic!("Expected StatusUpdate message");
        }
    }

    #[tokio::test]
    async fn test_worker_service_with_dispatcher_config() {
        let message_queue = Arc::new(MockMessageQueue::new());
        let worker_service = WorkerService::builder(
            "test-worker".to_string(),
            message_queue,
            "task_queue".to_string(),
            "status_queue".to_string(),
        )
        .dispatcher_url("http://localhost:8080".to_string())
        .hostname("test-host".to_string())
        .ip_address("192.168.1.100".to_string())
        .build();

        // 测试配置是否正确设置
        assert_eq!(
            worker_service.get_dispatcher_url(),
            &Some("http://localhost:8080".to_string())
        );
        assert_eq!(worker_service.get_hostname(), "test-host");
        assert_eq!(worker_service.get_ip_address(), "192.168.1.100");
    }

    #[tokio::test]
    async fn test_worker_service_registration_without_dispatcher() {
        let message_queue = Arc::new(MockMessageQueue::new());
        let worker_service = WorkerService::builder(
            "test-worker".to_string(),
            message_queue,
            "task_queue".to_string(),
            "status_queue".to_string(),
        )
        .build();

        // 没有配置Dispatcher URL时，注册应该成功但不执行实际注册
        assert!(worker_service.register_with_dispatcher().await.is_ok());
        assert!(worker_service.send_heartbeat_to_dispatcher().await.is_ok());
        assert!(worker_service.unregister_from_dispatcher().await.is_ok());
    }
}
