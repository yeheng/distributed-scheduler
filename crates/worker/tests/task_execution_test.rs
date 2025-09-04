use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use scheduler_application::{ExecutorRegistry, ExecutorStatus, TaskExecutionContext, TaskExecutor};
use scheduler_domain::entities::{TaskExecutionMessage, TaskResult, TaskRun, TaskRunStatus};
use scheduler_errors::{SchedulerError, SchedulerResult};
use scheduler_worker::components::TaskExecutionManager;
use serde_json::json;
use tokio::sync::{Mutex, RwLock};

// Mock执行器用于测试
#[derive(Debug)]
struct MockExecutor {
    name: String,
    should_succeed: bool,
    execution_delay_ms: u64,
    cancel_calls: Arc<Mutex<Vec<i64>>>,
}

impl MockExecutor {
    fn new(name: String, should_succeed: bool, execution_delay_ms: u64) -> Self {
        Self {
            name,
            should_succeed,
            execution_delay_ms,
            cancel_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn get_cancel_calls(&self) -> Vec<i64> {
        self.cancel_calls.lock().await.clone()
    }
}

#[async_trait]
impl TaskExecutor for MockExecutor {
    async fn execute_task(&self, _context: &TaskExecutionContext) -> SchedulerResult<TaskResult> {
        tokio::time::sleep(Duration::from_millis(self.execution_delay_ms)).await;

        if self.should_succeed {
            Ok(TaskResult {
                success: true,
                output: Some(format!("Mock {} 执行成功", self.name)),
                error_message: None,
                exit_code: Some(0),
                execution_time_ms: self.execution_delay_ms,
            })
        } else {
            Err(SchedulerError::TaskExecution(format!(
                "Mock {} 执行失败",
                self.name
            )))
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
        "Mock executor for testing"
    }

    fn supported_task_types(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()> {
        self.cancel_calls.lock().await.push(task_run_id);
        Ok(())
    }

    async fn is_running(&self, _task_run_id: i64) -> SchedulerResult<bool> {
        Ok(false)
    }

    async fn get_status(&self) -> SchedulerResult<ExecutorStatus> {
        Ok(ExecutorStatus {
            name: self.name.clone(),
            version: self.version().to_string(),
            healthy: true,
            running_tasks: 0,
            supported_task_types: self.supported_task_types(),
            last_health_check: Utc::now(),
            metadata: HashMap::new(),
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

// Mock执行器注册表
struct MockExecutorRegistry {
    executors: Arc<RwLock<HashMap<String, Arc<dyn TaskExecutor>>>>,
}

impl MockExecutorRegistry {
    fn new() -> Self {
        Self {
            executors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn add_executor(&self, executor: Arc<dyn TaskExecutor>) {
        let mut executors = self.executors.write().await;
        for task_type in executor.supported_task_types() {
            executors.insert(task_type, Arc::clone(&executor));
        }
    }
}

#[async_trait]
impl ExecutorRegistry for MockExecutorRegistry {
    async fn register(
        &mut self,
        name: String,
        executor: Arc<dyn TaskExecutor>,
    ) -> SchedulerResult<()> {
        let mut executors = self.executors.write().await;
        executors.insert(name, executor);
        Ok(())
    }

    async fn get(&self, name: &str) -> Option<Arc<dyn TaskExecutor>> {
        let executors = self.executors.read().await;
        executors.get(name).cloned()
    }

    async fn list_executors(&self) -> Vec<String> {
        let executors = self.executors.read().await;
        executors.keys().cloned().collect()
    }

    async fn unregister(&mut self, name: &str) -> SchedulerResult<bool> {
        let mut executors = self.executors.write().await;
        Ok(executors.remove(name).is_some())
    }

    async fn clear(&mut self) {
        let mut executors = self.executors.write().await;
        executors.clear();
    }

    async fn contains(&self, name: &str) -> bool {
        let executors = self.executors.read().await;
        executors.contains_key(name)
    }

    async fn count(&self) -> usize {
        let executors = self.executors.read().await;
        executors.len()
    }

    async fn get_all_status(&self) -> SchedulerResult<HashMap<String, ExecutorStatus>> {
        let mut status_map = HashMap::new();
        let executors = self.executors.read().await;

        for (name, executor) in executors.iter() {
            let status = executor.get_status().await?;
            status_map.insert(name.clone(), status);
        }

        Ok(status_map)
    }

    async fn health_check_all(&self) -> SchedulerResult<HashMap<String, bool>> {
        let mut health_map = HashMap::new();
        let executors = self.executors.read().await;

        for (name, executor) in executors.iter() {
            let healthy = executor.health_check().await?;
            health_map.insert(name.clone(), healthy);
        }

        Ok(health_map)
    }

    async fn get_by_task_type(
        &self,
        task_type: &str,
    ) -> SchedulerResult<Vec<Arc<dyn TaskExecutor>>> {
        let executors = self.executors.read().await;
        let mut matching = Vec::new();

        for executor in executors.values() {
            if executor.supports_task_type(task_type) {
                matching.push(Arc::clone(executor));
            }
        }

        Ok(matching)
    }
}

// 创建测试TaskRun
fn create_test_task_run(id: i64, task_id: i64, _task_type: String) -> TaskRun {
    TaskRun {
        id,
        task_id,
        status: TaskRunStatus::Pending,
        worker_id: Some("test-worker".to_string()),
        retry_count: 0,
        shard_index: Some(0),
        shard_total: Some(1),
        scheduled_at: Utc::now(),
        started_at: None,
        completed_at: None,
        result: None,
        error_message: None,
        created_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_execution_manager_creation() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let manager = TaskExecutionManager::new(
            "test-worker".to_string(),
            registry,
            2, // max_concurrent_tasks
        );

        assert_eq!(manager.get_current_task_count().await, 0);
        assert_eq!(manager.get_supported_task_types().await.len(), 0);
    }

    #[tokio::test]
    async fn test_can_accept_task_with_supported_executor() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let executor = Arc::new(MockExecutor::new("test".to_string(), true, 100));
        registry.add_executor(executor).await;

        let manager = TaskExecutionManager::new("test-worker".to_string(), registry, 2);

        assert!(manager.can_accept_task("test").await);
        assert!(!manager.can_accept_task("unsupported").await);
    }

    #[tokio::test]
    async fn test_can_accept_task_with_max_concurrency_limit() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let executor = Arc::new(MockExecutor::new("test".to_string(), true, 1000));
        registry.add_executor(executor).await;

        let manager = Arc::new(TaskExecutionManager::new(
            "test-worker".to_string(),
            registry,
            1, // 最大并发数为1
        ));

        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        // 第一个任务应该可以被接受
        assert!(manager.can_accept_task("test").await);

        // 启动一个长时间运行的任务
        let message = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_type: "test".to_string(),
            parameters: json!({"command":"echo","args":["hello"]}),
            timeout_seconds: 60,
            retry_count: 0,
            shard_index: Some(0),
            shard_total: Some(1),
            task_name: "todo".to_string(),
        };

        let status_callback = move |task_run_id, status, result, error_message| {
            let tx = tx.clone();
            async move {
                tx.send((task_run_id, status, result, error_message))
                    .await
                    .unwrap();
                Ok(())
            }
        };

        manager
            .handle_task_execution(message, status_callback)
            .await
            .unwrap();

        // 等待任务开始运行
        let (_task_run_id, status, _result, _error) = rx.recv().await.unwrap();
        assert_eq!(status, TaskRunStatus::Running);

        // 现在应该达到最大并发数，无法接受新任务
        assert!(!manager.can_accept_task("test").await);
        assert_eq!(manager.get_current_task_count().await, 1);

        // 等待任务完成
        let (_task_run_id, status, _result, _error) = rx.recv().await.unwrap();
        assert!(matches!(
            status,
            TaskRunStatus::Completed | TaskRunStatus::Failed
        ));

        // 任务完成后，应该能再次接受任务
        assert!(manager.can_accept_task("test").await);
        assert_eq!(manager.get_current_task_count().await, 0);
    }

    #[tokio::test]
    async fn test_handle_task_execution_success() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let executor = Arc::new(MockExecutor::new("test".to_string(), true, 10));
        registry.add_executor(executor).await;

        let manager = TaskExecutionManager::new("test-worker".to_string(), registry, 5);

        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        let message = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_type: "test".to_string(),
            parameters: json!({"test": "params"}),
            timeout_seconds: 30,
            retry_count: 0,
            shard_index: Some(0),
            shard_total: Some(1),
            task_name: "todo".to_string(),
        };

        let status_callback = move |task_run_id, status, result, error_message| {
            let tx = tx.clone();
            async move {
                tx.send((task_run_id, status, result, error_message))
                    .await
                    .unwrap();
                Ok(())
            }
        };

        let result = manager
            .handle_task_execution(message, status_callback)
            .await;
        assert!(result.is_ok());

        // 验证状态更新回调被正确调用
        let (task_run_id, status, _result, _error) = rx.recv().await.unwrap();
        assert_eq!(task_run_id, 1);
        assert_eq!(status, TaskRunStatus::Running);

        let (task_run_id, status, result, error) = rx.recv().await.unwrap();
        assert_eq!(task_run_id, 1);
        assert_eq!(status, TaskRunStatus::Completed);
        assert!(result.is_some());
        assert!(error.is_none());
    }

    #[tokio::test]
    async fn test_handle_task_execution_failure() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let executor = Arc::new(MockExecutor::new("test".to_string(), false, 10));
        registry.add_executor(executor).await;

        let manager = TaskExecutionManager::new("test-worker".to_string(), registry, 5);

        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        let message = TaskExecutionMessage {
            task_run_id: 2,
            task_id: 2,
            task_type: "test".to_string(),
            parameters: json!({"test": "params"}),
            timeout_seconds: 30,
            retry_count: 0,
            shard_index: Some(0),
            shard_total: Some(1),
            task_name: "todo".to_string(),
        };

        let status_callback = move |task_run_id, status, result, error_message| {
            let tx = tx.clone();
            async move {
                tx.send((task_run_id, status, result, error_message))
                    .await
                    .unwrap();
                Ok(())
            }
        };

        let result = manager
            .handle_task_execution(message, status_callback)
            .await;
        assert!(result.is_ok());

        // 验证状态更新回调
        let (task_run_id, status, _result, _error) = rx.recv().await.unwrap();
        assert_eq!(task_run_id, 2);
        assert_eq!(status, TaskRunStatus::Running);

        let (task_run_id, status, result, error) = rx.recv().await.unwrap();
        assert_eq!(task_run_id, 2);
        assert_eq!(status, TaskRunStatus::Failed);
        assert!(result.is_none());
        assert!(error.is_some());
    }

    #[tokio::test]
    async fn test_handle_task_execution_unsupported_type() {
        let registry = Arc::new(MockExecutorRegistry::new());

        let manager = TaskExecutionManager::new("test-worker".to_string(), registry, 5);

        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        let message = TaskExecutionMessage {
            task_run_id: 3,
            task_id: 3,
            task_type: "unsupported".to_string(),
            parameters: json!({}),
            timeout_seconds: 30,
            retry_count: 0,
            shard_index: Some(0),
            shard_total: Some(1),
            task_name: "todo".to_string(),
        };

        let status_callback = move |task_run_id, status, result, error_message| {
            let tx = tx.clone();
            async move {
                tx.send((task_run_id, status, result, error_message))
                    .await
                    .unwrap();
                Ok(())
            }
        };

        let result = manager
            .handle_task_execution(message, status_callback)
            .await;
        assert!(result.is_ok());

        // 验证不支持的任务类型直接失败
        let (task_run_id, status, result, error) = rx.recv().await.unwrap();
        assert_eq!(task_run_id, 3);
        assert_eq!(status, TaskRunStatus::Failed);
        assert!(result.is_none());
        assert!(error.is_some());
        assert!(error.unwrap().contains("Unsupported task type"));
    }

    #[tokio::test]
    async fn test_cancel_task_success() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let executor = Arc::new(MockExecutor::new("test".to_string(), true, 1000));
        registry.add_executor(executor.clone() as Arc<dyn TaskExecutor>).await;

        let manager = Arc::new(TaskExecutionManager::new(
            "test-worker".to_string(),
            registry,
            5,
        ));

        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        // 启动一个长时间运行的任务
        let message = TaskExecutionMessage {
            task_run_id: 4,
            task_id: 4,
            task_type: "test".to_string(),
            parameters: json!({}),
            timeout_seconds: 60,
            retry_count: 0,
            shard_index: Some(0),
            shard_total: Some(1),
            task_name: "todo".to_string(),
        };

        let status_callback = move |task_run_id, status, result, error_message| {
            let tx = tx.clone();
            async move {
                tx.send((task_run_id, status, result, error_message))
                    .await
                    .unwrap();
                Ok(())
            }
        };

        manager
            .handle_task_execution(message, status_callback)
            .await
            .unwrap();

        // 等待任务开始运行
        let (_task_run_id, status, _result, _error) = rx.recv().await.unwrap();
        assert_eq!(status, TaskRunStatus::Running);

        // 取消任务
        let cancel_result = manager.cancel_task(4).await;
        assert!(cancel_result.is_ok());

        // 验证执行器的cancel方法被调用
        let cancel_calls = executor.get_cancel_calls().await;
        assert!(cancel_calls.contains(&4));

        // 任务应该从运行列表中移除
        assert_eq!(manager.get_current_task_count().await, 0);
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_task() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let manager = TaskExecutionManager::new("test-worker".to_string(), registry, 5);

        let cancel_result = manager.cancel_task(999).await;
        assert!(cancel_result.is_err());

        let error = cancel_result.unwrap_err();
        assert!(error.to_string().contains("未找到或未在运行中"));
    }

    #[tokio::test]
    async fn test_concurrent_task_execution() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let executor = Arc::new(MockExecutor::new("test".to_string(), true, 100));
        registry.add_executor(executor).await;

        let manager = Arc::new(TaskExecutionManager::new(
            "test-worker".to_string(),
            registry,
            3, // 允许3个并发任务
        ));

        let (tx, mut rx) = tokio::sync::mpsc::channel(20);

        // 并发启动3个任务
        for i in 1..=3 {
            let message = TaskExecutionMessage {
                task_run_id: i,
                task_id: i,
                task_type: "test".to_string(),
                parameters: json!({"task_id": i}),
                timeout_seconds: 30,
                retry_count: 0,
                shard_index: Some(0),
                shard_total: Some(1),
                task_name: "todo".to_string(),
            };

            let tx = tx.clone();
            let status_callback = move |task_run_id, status, result, error_message| {
                let tx = tx.clone();
                async move {
                    tx.send((task_run_id, status, result, error_message))
                        .await
                        .unwrap();
                    Ok(())
                }
            };

            let result = manager
                .handle_task_execution(message, status_callback)
                .await;
            assert!(result.is_ok());
        }

        // 验证3个任务都开始运行
        let mut running_statuses = 0;
        let mut completed_statuses = 0;

        for _ in 0..6 {
            // 3个任务 × 2个状态更新（Running + Completed）
            let (_task_run_id, status, _result, _error) = rx.recv().await.unwrap();
            match status {
                TaskRunStatus::Running => running_statuses += 1,
                TaskRunStatus::Completed => completed_statuses += 1,
                _ => {}
            }
        }

        assert_eq!(running_statuses, 3);
        assert_eq!(completed_statuses, 3);
        assert_eq!(manager.get_current_task_count().await, 0);
    }

    #[tokio::test]
    async fn test_max_concurrency_rejection() {
        let registry = Arc::new(MockExecutorRegistry::new());
        let executor = Arc::new(MockExecutor::new("test".to_string(), true, 500));
        registry.add_executor(executor).await;

        let manager = Arc::new(TaskExecutionManager::new(
            "test-worker".to_string(),
            registry,
            1, // 只允许1个并发任务
        ));

        let (tx1, mut rx1) = tokio::sync::mpsc::channel(10);
        let (tx2, mut rx2) = tokio::sync::mpsc::channel(10);

        // 启动第一个任务
        let message1 = TaskExecutionMessage {
            task_run_id: 1,
            task_id: 1,
            task_type: "test".to_string(),
            parameters: json!({}),
            timeout_seconds: 30,
            retry_count: 0,
            shard_index: Some(0),
            shard_total: Some(1),
            task_name: "todo".to_string(),
        };

        let status_callback1 = move |task_run_id, status, result, error_message| {
            let tx1 = tx1.clone();
            async move {
                tx1.send((task_run_id, status, result, error_message))
                    .await
                    .unwrap();
                Ok(())
            }
        };

        manager
            .handle_task_execution(message1, status_callback1)
            .await
            .unwrap();

        // 等待第一个任务开始运行
        let (_task_run_id, status, _result, _error) = rx1.recv().await.unwrap();
        assert_eq!(status, TaskRunStatus::Running);
        assert_eq!(manager.get_current_task_count().await, 1);

        // 尝试启动第二个任务，应该被拒绝
        let message2 = TaskExecutionMessage {
            task_run_id: 2,
            task_id: 2,
            task_type: "test".to_string(),
            parameters: json!({}),
            timeout_seconds: 30,
            retry_count: 0,
            shard_index: Some(0),
            shard_total: Some(1),
            task_name: "todo".to_string(),
        };

        let status_callback2 = move |task_run_id, status, result, error_message| {
            let tx2 = tx2.clone();
            async move {
                tx2.send((task_run_id, status, result, error_message))
                    .await
                    .unwrap();
                Ok(())
            }
        };

        manager
            .handle_task_execution(message2, status_callback2)
            .await
            .unwrap();

        // 第二个任务应该立即失败（达到并发限制）
        let (task_run_id, status, _result, error) = rx2.recv().await.unwrap();
        assert_eq!(task_run_id, 2);
        assert_eq!(status, TaskRunStatus::Failed);
        assert!(error.is_some());
        assert!(error.unwrap().contains("concurrency limit"));

        // 等待第一个任务完成
        let (_task_run_id, status, _result, _error) = rx1.recv().await.unwrap();
        assert_eq!(status, TaskRunStatus::Completed);
        assert_eq!(manager.get_current_task_count().await, 0);
    }
}
