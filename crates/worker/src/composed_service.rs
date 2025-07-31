use std::sync::Arc;

use scheduler_core::{
    models::{TaskRun, TaskStatusUpdate},
    traits::ExecutorRegistry,
    SchedulerResult, SchedulerError, ServiceLocator,
};

use super::components::{
    DispatcherClient, HeartbeatManager, TaskExecutionManager, WorkerLifecycle,
};

/// Worker service builder - Uses dependency injection
/// Follows Builder pattern for complex object construction
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
}

impl WorkerServiceBuilder {
    /// Create new builder
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
        }
    }

    /// Set executor registry
    pub fn with_executor_registry(mut self, registry: Arc<dyn ExecutorRegistry>) -> Self {
        self.executor_registry = Some(registry);
        self
    }

    /// Set max concurrent tasks
    pub fn max_concurrent_tasks(mut self, max_concurrent_tasks: usize) -> Self {
        self.max_concurrent_tasks = max_concurrent_tasks;
        self
    }

    /// Set heartbeat interval
    pub fn heartbeat_interval_seconds(mut self, heartbeat_interval_seconds: u64) -> Self {
        self.heartbeat_interval_seconds = heartbeat_interval_seconds;
        self
    }

    /// Set poll interval
    pub fn poll_interval_ms(mut self, poll_interval_ms: u64) -> Self {
        self.poll_interval_ms = poll_interval_ms;
        self
    }

    /// Set dispatcher URL
    pub fn dispatcher_url(mut self, dispatcher_url: String) -> Self {
        self.dispatcher_url = Some(dispatcher_url);
        self
    }

    /// Set hostname
    pub fn hostname(mut self, hostname: String) -> Self {
        self.hostname = hostname;
        self
    }

    /// Set IP address
    pub fn ip_address(mut self, ip_address: String) -> Self {
        self.ip_address = ip_address;
        self
    }

    /// Build WorkerService with composition pattern
    pub async fn build(self) -> SchedulerResult<WorkerService> {
        let executor_registry = self
            .executor_registry
            .ok_or_else(|| SchedulerError::Internal("Executor registry is required".to_string()))?;

        // Create components using composition
        let task_execution_manager = Arc::new(TaskExecutionManager::new(
            self.worker_id.clone(),
            Arc::clone(&executor_registry),
            self.max_concurrent_tasks,
        ));

        let dispatcher_client = Arc::new(DispatcherClient::new(
            self.dispatcher_url.clone(),
            self.worker_id.clone(),
            self.hostname.clone(),
            self.ip_address.clone(),
        ));

        let heartbeat_manager = Arc::new(HeartbeatManager::new(
            self.worker_id.clone(),
            Arc::clone(&self.service_locator),
            self.status_queue.clone(),
            self.heartbeat_interval_seconds,
            Arc::clone(&dispatcher_client),
        ));

        let worker_lifecycle = Arc::new(WorkerLifecycle::new(
            self.worker_id.clone(),
            Arc::clone(&self.service_locator),
            self.task_queue.clone(),
            self.poll_interval_ms,
            Arc::clone(&task_execution_manager),
            Arc::clone(&dispatcher_client),
            Arc::clone(&heartbeat_manager),
        ));

        Ok(WorkerService {
            _worker_id: self.worker_id,
            _service_locator: self.service_locator,
            _executor_registry: executor_registry,
            _max_concurrent_tasks: self.max_concurrent_tasks,
            task_execution_manager,
            dispatcher_client,
            heartbeat_manager,
            worker_lifecycle,
        })
    }
}

/// Worker service - Composition-based implementation
/// Follows SRP: Only orchestrates components and provides unified interface
pub struct WorkerService {
    /// Worker unique identifier
    _worker_id: String,

    /// Service locator for dependency access
    _service_locator: Arc<ServiceLocator>,

    /// Executor registry for task execution
    _executor_registry: Arc<dyn ExecutorRegistry>,

    /// Maximum concurrent tasks
    _max_concurrent_tasks: usize,

    /// Component: Task execution manager
    task_execution_manager: Arc<TaskExecutionManager>,

    /// Component: Dispatcher client
    dispatcher_client: Arc<DispatcherClient>,

    /// Component: Heartbeat manager
    heartbeat_manager: Arc<HeartbeatManager>,

    /// Component: Worker lifecycle manager
    worker_lifecycle: Arc<WorkerLifecycle>,
}

impl WorkerService {
    /// Create builder
    pub fn builder(
        worker_id: String,
        service_locator: Arc<ServiceLocator>,
        task_queue: String,
        status_queue: String,
    ) -> WorkerServiceBuilder {
        WorkerServiceBuilder::new(worker_id, service_locator, task_queue, status_queue)
    }

    /// Get supported task types
    pub async fn get_supported_task_types(&self) -> Vec<String> {
        self.task_execution_manager.get_supported_task_types().await
    }

    /// Get current task count
    pub async fn get_current_task_count(&self) -> i32 {
        self.task_execution_manager.get_current_task_count().await
    }

    /// Check if worker can accept task
    pub async fn can_accept_task(&self, task_type: &str) -> bool {
        self.task_execution_manager.can_accept_task(task_type).await
    }

    /// Get running tasks
    pub async fn get_running_tasks(&self) -> Vec<TaskRun> {
        // This would need to be added to TaskExecutionManager
        // For now, return empty vector
        Vec::new()
    }

    /// Check if task is running
    pub async fn is_task_running(&self, _task_run_id: i64) -> bool {
        // This would need to be added to TaskExecutionManager
        // For now, return false
        false
    }

    /// Register with dispatcher
    pub async fn register_with_dispatcher(&self) -> SchedulerResult<()> {
        let supported_types = self.get_supported_task_types().await;
        self.dispatcher_client.register(supported_types).await
    }

    /// Send heartbeat to dispatcher
    pub async fn send_heartbeat_to_dispatcher(&self) -> SchedulerResult<()> {
        let current_task_count = self.get_current_task_count().await;
        self.dispatcher_client
            .send_heartbeat(current_task_count)
            .await
    }

    /// Unregister from dispatcher
    pub async fn unregister_from_dispatcher(&self) -> SchedulerResult<()> {
        self.dispatcher_client.unregister().await
    }

    /// Check if dispatcher is configured
    pub fn is_dispatcher_configured(&self) -> bool {
        self.dispatcher_client.is_configured()
    }

    /// Send status update
    pub async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()> {
        self.heartbeat_manager.send_status_update(update).await
    }

    /// Cancel a running task
    pub async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()> {
        self.task_execution_manager.cancel_task(task_run_id).await
    }

    /// Check if service is running
    pub async fn is_running(&self) -> bool {
        self.worker_lifecycle.is_running().await
    }
}

/// Worker service trait for abstract interface
#[async_trait::async_trait]
pub trait WorkerServiceTrait: Send + Sync {
    /// Start the worker service
    async fn start(&self) -> SchedulerResult<()>;

    /// Stop the worker service
    async fn stop(&self) -> SchedulerResult<()>;

    /// Poll and execute tasks
    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()>;

    /// Send status update
    async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()>;

    /// Get current task count
    async fn get_current_task_count(&self) -> i32;

    /// Check if worker can accept task
    async fn can_accept_task(&self, task_type: &str) -> bool;

    /// Cancel a running task
    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()>;

    /// Get running tasks
    async fn get_running_tasks(&self) -> Vec<TaskRun>;

    /// Check if task is running
    async fn is_task_running(&self, task_run_id: i64) -> bool;

    /// Send heartbeat
    async fn send_heartbeat(&self) -> SchedulerResult<()>;
}

#[async_trait::async_trait]
impl WorkerServiceTrait for WorkerService {
    async fn start(&self) -> SchedulerResult<()> {
        self.worker_lifecycle.start().await
    }

    async fn stop(&self) -> SchedulerResult<()> {
        self.worker_lifecycle.stop().await
    }

    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()> {
        // Delegate to lifecycle manager
        // This would need to be exposed as a public method
        Ok(())
    }

    async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()> {
        self.send_status_update(update).await
    }

    async fn get_current_task_count(&self) -> i32 {
        self.get_current_task_count().await
    }

    async fn can_accept_task(&self, task_type: &str) -> bool {
        self.can_accept_task(task_type).await
    }

    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()> {
        self.cancel_task(task_run_id).await
    }

    async fn get_running_tasks(&self) -> Vec<TaskRun> {
        self.get_running_tasks().await
    }

    async fn is_task_running(&self, task_run_id: i64) -> bool {
        self.is_task_running(task_run_id).await
    }

    async fn send_heartbeat(&self) -> SchedulerResult<()> {
        // This would need to be exposed through heartbeat manager
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_core::ApplicationContext;

    #[tokio::test]
    async fn test_worker_service_builder() {
        let service_locator = Arc::new(ServiceLocator::new(Arc::new(ApplicationContext::new())));
        let worker_id = "test_worker".to_string();
        let task_queue = "test_tasks".to_string();
        let status_queue = "test_status".to_string();

        // This test would need a mock executor registry
        // For now, just test the builder structure
        let builder =
            WorkerServiceBuilder::new(worker_id.clone(), service_locator, task_queue, status_queue);

        assert_eq!(builder.worker_id, worker_id);
        assert_eq!(builder.max_concurrent_tasks, 5);
        assert_eq!(builder.heartbeat_interval_seconds, 30);
    }

    #[tokio::test]
    async fn test_worker_service_creation() {
        // This would need proper mocking of dependencies
        // For now, just verify the structure exists
        let service_locator = Arc::new(ServiceLocator::new(Arc::new(ApplicationContext::new())));
        let worker_id = "test_worker".to_string();
        let task_queue = "test_tasks".to_string();
        let status_queue = "test_status".to_string();

        // Test that we can create a builder
        let _builder =
            WorkerServiceBuilder::new(worker_id, service_locator, task_queue, status_queue);
    }

    #[tokio::test]
    async fn test_worker_service_interface() {
        // Test that the trait is implemented correctly
        // This would need proper mocking
        let service_locator = Arc::new(ServiceLocator::new(Arc::new(ApplicationContext::new())));
        let worker_id = "test_worker".to_string();
        let task_queue = "test_tasks".to_string();
        let status_queue = "test_status".to_string();

        // Create a mock service
        let builder =
            WorkerServiceBuilder::new(worker_id, service_locator, task_queue, status_queue);

        // Test builder configuration
        assert_eq!(builder.max_concurrent_tasks, 5);
        assert_eq!(builder.heartbeat_interval_seconds, 30);
        assert_eq!(builder.poll_interval_ms, 1000);
    }
}
