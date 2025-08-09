use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use scheduler_core::{
    Message, MessageQueue, MessageType, Result, SchedulerError, TaskControlAction,
    TaskControlMessage, TaskExecutionMessage, TaskExecutor, TaskResult, TaskRun, TaskRunStatus,
};
use scheduler_worker::{WorkerService, WorkerServiceTrait};
use serde_json::json;
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;

#[derive(Debug)]
struct InMemoryMessageQueue {
    queues: Arc<RwLock<HashMap<String, Vec<Message>>>>,
    ack_messages: Arc<Mutex<Vec<String>>>,
}

impl InMemoryMessageQueue {
    fn new() -> Self {
        Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            ack_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn add_message_to_queue(&self, queue: &str, message: Message) {
        let mut queues = self.queues.write().await;
        queues.entry(queue.to_string()).or_default().push(message);
    }

    async fn get_queue_messages(&self, queue: &str) -> Vec<Message> {
        let queues = self.queues.read().await;
        queues.get(queue).cloned().unwrap_or_default()
    }

    async fn clear_queue(&self, queue: &str) {
        let mut queues = self.queues.write().await;
        queues.insert(queue.to_string(), Vec::new());
    }
}

#[async_trait]
impl MessageQueue for InMemoryMessageQueue {
    async fn publish_message(&self, queue: &str, message: &Message) -> Result<()> {
        self.add_message_to_queue(queue, message.clone()).await;
        Ok(())
    }

    async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>> {
        let mut queues = self.queues.write().await;
        let queue_messages = queues.entry(queue.to_string()).or_default();
        if let Some(message) = queue_messages.pop() {
            Ok(vec![message])
        } else {
            Ok(vec![])
        }
    }

    async fn ack_message(&self, message_id: &str) -> Result<()> {
        let mut ack_messages = self.ack_messages.lock().await;
        ack_messages.push(message_id.to_string());
        Ok(())
    }

    async fn nack_message(&self, _message_id: &str, _requeue: bool) -> Result<()> {
        Ok(())
    }

    async fn create_queue(&self, queue: &str, _durable: bool) -> Result<()> {
        let mut queues = self.queues.write().await;
        queues.insert(queue.to_string(), Vec::new());
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> Result<()> {
        let mut queues = self.queues.write().await;
        queues.remove(queue);
        Ok(())
    }

    async fn get_queue_size(&self, queue: &str) -> Result<u32> {
        let queues = self.queues.read().await;
        Ok(queues.get(queue).map(|q| q.len()).unwrap_or(0) as u32)
    }

    async fn purge_queue(&self, queue: &str) -> Result<()> {
        self.clear_queue(queue).await;
        Ok(())
    }
}

#[derive(Debug)]
struct TestTaskExecutor {
    name: String,
    execution_behavior: TestExecutionBehavior,
    execution_delay_ms: u64,
    running_tasks: Arc<RwLock<HashMap<i64, bool>>>,
}

#[derive(Debug, Clone)]
enum TestExecutionBehavior {
    Success,
    Failure,
    Timeout,
}

impl TestTaskExecutor {
    fn new(name: String, behavior: TestExecutionBehavior, delay_ms: u64) -> Self {
        Self {
            name,
            execution_behavior: behavior,
            execution_delay_ms: delay_ms,
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn success(name: String) -> Self {
        Self::new(name, TestExecutionBehavior::Success, 100)
    }

    fn failure(name: String) -> Self {
        Self::new(name, TestExecutionBehavior::Failure, 100)
    }

    fn slow_success(name: String, delay_ms: u64) -> Self {
        Self::new(name, TestExecutionBehavior::Success, delay_ms)
    }

    fn timeout(name: String) -> Self {
        Self::new(name, TestExecutionBehavior::Timeout, 5000) // 5秒延迟，用于测试超时
    }
}

#[async_trait]
impl TaskExecutor for TestTaskExecutor {
    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult> {
        let start_time = std::time::Instant::now();
        {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.insert(task_run.id, true);
        }
        sleep(Duration::from_millis(self.execution_delay_ms)).await;
        {
            let running_tasks = self.running_tasks.read().await;
            if !running_tasks.get(&task_run.id).unwrap_or(&true) {
                return Ok(TaskResult {
                    success: false,
                    output: None,
                    error_message: Some("任务被取消".to_string()),
                    exit_code: Some(-1),
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                });
            }
        }

        let execution_time = start_time.elapsed().as_millis() as u64;

        let result = match self.execution_behavior {
            TestExecutionBehavior::Success => TaskResult {
                success: true,
                output: Some(format!("任务 {} 执行成功", task_run.id)),
                error_message: None,
                exit_code: Some(0),
                execution_time_ms: execution_time,
            },
            TestExecutionBehavior::Failure => TaskResult {
                success: false,
                output: None,
                error_message: Some(format!("任务 {} 执行失败", task_run.id)),
                exit_code: Some(1),
                execution_time_ms: execution_time,
            },
            TestExecutionBehavior::Timeout => {
                sleep(Duration::from_secs(10)).await;
                TaskResult {
                    success: true,
                    output: Some("不应该到达这里".to_string()),
                    error_message: None,
                    exit_code: Some(0),
                    execution_time_ms: execution_time,
                }
            }
        };
        {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.remove(&task_run.id);
        }

        Ok(result)
    }

    fn supports_task_type(&self, task_type: &str) -> bool {
        task_type == self.name
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn cancel(&self, task_run_id: i64) -> Result<()> {
        let mut running_tasks = self.running_tasks.write().await;
        running_tasks.insert(task_run_id, false); // 标记为取消
        Ok(())
    }

    async fn is_running(&self, task_run_id: i64) -> Result<bool> {
        let running_tasks = self.running_tasks.read().await;
        Ok(running_tasks.get(&task_run_id).unwrap_or(&false).clone())
    }
}

async fn create_test_worker_service(
    worker_id: &str,
    message_queue: Arc<dyn MessageQueue>,
    max_concurrent_tasks: usize,
) -> WorkerService {
    let shell_executor = Arc::new(TestTaskExecutor::success("shell".to_string()));
    let http_executor = Arc::new(TestTaskExecutor::success("http".to_string()));

    WorkerService::builder(
        worker_id.to_string(),
        message_queue,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .max_concurrent_tasks(max_concurrent_tasks)
    .heartbeat_interval_seconds(1) // 快速心跳用于测试
    .poll_interval_ms(100) // 快速轮询用于测试
    .register_executor(shell_executor)
    .register_executor(http_executor)
    .build()
}

fn create_test_task_execution_message(
    task_run_id: i64,
    task_id: i64,
    task_type: &str,
    timeout_seconds: i32,
) -> TaskExecutionMessage {
    TaskExecutionMessage {
        task_run_id,
        task_id,
        task_name: format!("test_task_{}", task_run_id),
        task_type: task_type.to_string(),
        parameters: json!({"command": "echo hello"}),
        timeout_seconds,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    }
}

#[tokio::test]
async fn test_worker_service_task_execution_integration() {
    let message_queue = Arc::new(InMemoryMessageQueue::new());
    let worker_service = create_test_worker_service("test-worker", message_queue.clone(), 5).await;
    let task_message = create_test_task_execution_message(1, 100, "shell", 30);
    let message = Message::task_execution(task_message);
    message_queue
        .add_message_to_queue("task_queue", message)
        .await;
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());
    sleep(Duration::from_millis(300)).await;
    let status_messages = message_queue.get_queue_messages("status_queue").await;
    assert!(!status_messages.is_empty());
    let mut running_found = false;
    let mut completed_found = false;

    for msg in status_messages {
        if let MessageType::StatusUpdate(status_update) = msg.message_type {
            assert_eq!(status_update.task_run_id, 1);
            assert_eq!(status_update.worker_id, "test-worker");

            match status_update.status {
                TaskRunStatus::Running => running_found = true,
                TaskRunStatus::Completed => {
                    completed_found = true;
                    assert!(status_update.result.is_some());
                    assert!(status_update.error_message.is_none());
                }
                _ => {}
            }
        }
    }

    assert!(running_found, "应该有运行状态更新");
    assert!(completed_found, "应该有完成状态更新");
    assert_eq!(worker_service.get_current_task_count().await, 0);
}

#[tokio::test]
async fn test_worker_service_concurrent_task_execution() {
    let message_queue = Arc::new(InMemoryMessageQueue::new());
    let worker_service = create_test_worker_service("test-worker", message_queue.clone(), 2).await;
    for i in 1..=3 {
        let task_message = create_test_task_execution_message(i, 100 + i, "shell", 30);
        let message = Message::task_execution(task_message);
        message_queue
            .add_message_to_queue("task_queue", message)
            .await;
    }
    for _ in 0..5 {
        assert!(worker_service.poll_and_execute_tasks().await.is_ok());
        sleep(Duration::from_millis(50)).await;
        let current_count = worker_service.get_current_task_count().await;
        assert!(
            current_count <= 2,
            "并发任务数不应超过限制: {}",
            current_count
        );
        if current_count == 0 {
            break;
        }

        sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(worker_service.get_current_task_count().await, 0);
    let status_messages = message_queue.get_queue_messages("status_queue").await;

    let completed_count = status_messages
        .iter()
        .filter(|msg| {
            if let MessageType::StatusUpdate(status_update) = &msg.message_type {
                status_update.status == TaskRunStatus::Completed
            } else {
                false
            }
        })
        .count();

    assert_eq!(
        completed_count, 3,
        "应该有3个完成的任务，实际有{}个",
        completed_count
    );
}

#[tokio::test]
async fn test_worker_service_task_timeout_handling() {
    let message_queue = Arc::new(InMemoryMessageQueue::new());
    let timeout_executor = Arc::new(TestTaskExecutor::timeout("shell".to_string()));

    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue.clone(),
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .max_concurrent_tasks(1)
    .register_executor(timeout_executor)
    .build();
    let task_message = create_test_task_execution_message(1, 100, "shell", 1);
    let message = Message::task_execution(task_message);
    message_queue
        .add_message_to_queue("task_queue", message)
        .await;
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());
    sleep(Duration::from_millis(1500)).await;
    let status_messages = message_queue.get_queue_messages("status_queue").await;

    let timeout_found = status_messages.iter().any(|msg| {
        if let MessageType::StatusUpdate(status_update) = &msg.message_type {
            status_update.status == TaskRunStatus::Timeout
                && status_update
                    .error_message
                    .as_ref()
                    .map_or(false, |e| e.contains("超时"))
        } else {
            false
        }
    });

    assert!(timeout_found, "应该有超时状态更新");
    assert_eq!(worker_service.get_current_task_count().await, 0);
}

#[tokio::test]
async fn test_worker_service_task_failure_handling() {
    let message_queue = Arc::new(InMemoryMessageQueue::new());
    let failure_executor = Arc::new(TestTaskExecutor::failure("shell".to_string()));

    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue.clone(),
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .register_executor(failure_executor)
    .build();
    let task_message = create_test_task_execution_message(1, 100, "shell", 30);
    let message = Message::task_execution(task_message);
    message_queue
        .add_message_to_queue("task_queue", message)
        .await;
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());
    sleep(Duration::from_millis(300)).await;
    let status_messages = message_queue.get_queue_messages("status_queue").await;

    let failure_found = status_messages.iter().any(|msg| {
        if let MessageType::StatusUpdate(status_update) = &msg.message_type {
            status_update.status == TaskRunStatus::Failed && status_update.error_message.is_some()
        } else {
            false
        }
    });

    assert!(failure_found, "应该有失败状态更新");
    assert_eq!(worker_service.get_current_task_count().await, 0);
}

#[tokio::test]
async fn test_worker_service_unsupported_task_type() {
    let message_queue = Arc::new(InMemoryMessageQueue::new());
    let shell_executor = Arc::new(TestTaskExecutor::success("shell".to_string()));

    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue.clone(),
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .register_executor(shell_executor)
    .build();
    let task_message = create_test_task_execution_message(1, 100, "python", 30);
    let message = Message::task_execution(task_message);
    message_queue
        .add_message_to_queue("task_queue", message)
        .await;
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());
    sleep(Duration::from_millis(100)).await;
    let status_messages = message_queue.get_queue_messages("status_queue").await;

    let unsupported_found = status_messages.iter().any(|msg| {
        if let MessageType::StatusUpdate(status_update) = &msg.message_type {
            status_update.status == TaskRunStatus::Failed
                && status_update
                    .error_message
                    .as_ref()
                    .map_or(false, |e| e.contains("不支持的任务类型"))
        } else {
            false
        }
    });

    assert!(unsupported_found, "应该有不支持任务类型的失败状态更新");
}

#[tokio::test]
async fn test_worker_service_task_control_cancel() {
    let message_queue = Arc::new(InMemoryMessageQueue::new());
    let slow_executor = Arc::new(TestTaskExecutor::slow_success("shell".to_string(), 2000));

    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        message_queue.clone(),
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .register_executor(slow_executor)
    .build();
    let task_message = create_test_task_execution_message(1, 100, "shell", 30);
    let message = Message::task_execution(task_message);
    message_queue
        .add_message_to_queue("task_queue", message)
        .await;
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());
    sleep(Duration::from_millis(100)).await;
    assert_eq!(worker_service.get_current_task_count().await, 1);
    let control_message = TaskControlMessage {
        task_run_id: 1,
        action: TaskControlAction::Cancel,
        requester: "test".to_string(),
        timestamp: Utc::now(),
    };
    let control_msg = Message::task_control(control_message);
    message_queue
        .add_message_to_queue("task_queue", control_msg)
        .await;
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());
    sleep(Duration::from_millis(500)).await;
    assert_eq!(worker_service.get_current_task_count().await, 0);
    let status_messages = message_queue.get_queue_messages("status_queue").await;

    let cancel_found = status_messages.iter().any(|msg| {
        if let MessageType::StatusUpdate(status_update) = &msg.message_type {
            status_update.status == TaskRunStatus::Failed
                && status_update
                    .error_message
                    .as_ref()
                    .map_or(false, |e| e.contains("取消"))
        } else {
            false
        }
    });

    assert!(cancel_found, "应该有任务取消的状态更新");
}

#[tokio::test]
async fn test_worker_service_status_update_retry() {
    #[derive(Debug)]
    struct FailingMessageQueue {
        inner: InMemoryMessageQueue,
        fail_count: Arc<Mutex<u32>>,
        max_failures: u32,
    }

    impl FailingMessageQueue {
        fn new(max_failures: u32) -> Self {
            Self {
                inner: InMemoryMessageQueue::new(),
                fail_count: Arc::new(Mutex::new(0)),
                max_failures,
            }
        }
    }

    #[async_trait]
    impl MessageQueue for FailingMessageQueue {
        async fn publish_message(&self, queue: &str, message: &Message) -> Result<()> {
            if queue == "status_queue" {
                if let MessageType::StatusUpdate(status_update) = &message.message_type {
                    if status_update.status == TaskRunStatus::Completed {
                        let mut count = self.fail_count.lock().await;
                        if *count < self.max_failures {
                            *count += 1;

                            return Err(SchedulerError::MessageQueue("模拟发送失败".to_string()));
                        }
                    }
                }
            }
            self.inner.publish_message(queue, message).await
        }

        async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>> {
            self.inner.consume_messages(queue).await
        }

        async fn ack_message(&self, message_id: &str) -> Result<()> {
            self.inner.ack_message(message_id).await
        }

        async fn nack_message(&self, message_id: &str, requeue: bool) -> Result<()> {
            self.inner.nack_message(message_id, requeue).await
        }

        async fn create_queue(&self, queue: &str, durable: bool) -> Result<()> {
            self.inner.create_queue(queue, durable).await
        }

        async fn delete_queue(&self, queue: &str) -> Result<()> {
            self.inner.delete_queue(queue).await
        }

        async fn get_queue_size(&self, queue: &str) -> Result<u32> {
            self.inner.get_queue_size(queue).await
        }

        async fn purge_queue(&self, queue: &str) -> Result<()> {
            self.inner.purge_queue(queue).await
        }
    }
    let message_queue = Arc::new(FailingMessageQueue::new(2));
    let worker_service = create_test_worker_service("test-worker", message_queue.clone(), 1).await;
    let task_message = create_test_task_execution_message(1, 100, "shell", 30);
    let message = Message::task_execution(task_message);
    message_queue
        .inner
        .add_message_to_queue("task_queue", message)
        .await;
    assert!(worker_service.poll_and_execute_tasks().await.is_ok());
    sleep(Duration::from_millis(2000)).await;
    let status_messages = message_queue.inner.get_queue_messages("status_queue").await;
    let fail_count = *message_queue.fail_count.lock().await;

    assert!(!status_messages.is_empty(), "重试后应该有状态更新消息");
    assert!(fail_count >= 2, "应该至少失败2次，实际失败{}次", fail_count);
}

#[tokio::test]
async fn test_worker_service_start_stop_lifecycle() {
    let message_queue = Arc::new(InMemoryMessageQueue::new());
    let worker_service = create_test_worker_service("test-worker", message_queue.clone(), 5).await;
    assert!(worker_service.start().await.is_ok());
    sleep(Duration::from_millis(100)).await;
    let task_message = create_test_task_execution_message(1, 100, "shell", 30);
    let message = Message::task_execution(task_message);
    message_queue
        .add_message_to_queue("task_queue", message)
        .await;
    sleep(Duration::from_millis(500)).await;
    let status_messages = message_queue.get_queue_messages("status_queue").await;
    assert!(!status_messages.is_empty(), "任务应该被执行");
    assert!(worker_service.stop().await.is_ok());
    message_queue.clear_queue("status_queue").await;
    let task_message2 = create_test_task_execution_message(2, 101, "shell", 30);
    let message2 = Message::task_execution(task_message2);
    message_queue
        .add_message_to_queue("task_queue", message2)
        .await;
    sleep(Duration::from_millis(200)).await;
    let _status_messages_after_stop = message_queue.get_queue_messages("status_queue").await;
    assert_eq!(
        worker_service.get_current_task_count().await,
        0,
        "停止后不应该有运行中的任务"
    );
}
