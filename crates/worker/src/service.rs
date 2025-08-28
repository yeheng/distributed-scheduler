use std::sync::Arc;

use scheduler_application::{scheduler::WorkerService, ExecutorRegistry};
use scheduler_core::ServiceLocator;
use scheduler_domain::entities::TaskRun;
use scheduler_domain::events::TaskStatusUpdate;
use scheduler_errors::SchedulerResult;

use crate::components::{
    DispatcherClient, HeartbeatManager, TaskExecutionManager, WorkerLifecycle,
};

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub worker_id: String,
    pub task_queue: String,
    pub status_queue: String,
    pub max_concurrent_tasks: usize,
    pub heartbeat_interval_seconds: u64,
    pub poll_interval_ms: u64,
    pub dispatcher_url: Option<String>,
    pub hostname: String,
    pub ip_address: String,
}

impl WorkerConfig {
    pub fn builder(
        worker_id: String,
        task_queue: String,
        status_queue: String,
    ) -> WorkerConfigBuilder {
        WorkerConfigBuilder::new(worker_id, task_queue, status_queue)
    }
}

pub struct WorkerConfigBuilder {
    config: WorkerConfig,
}

impl WorkerConfigBuilder {
    pub fn new(worker_id: String, task_queue: String, status_queue: String) -> Self {
        Self {
            config: WorkerConfig {
                worker_id,
                task_queue,
                status_queue,
                max_concurrent_tasks: 5,
                heartbeat_interval_seconds: 30,
                poll_interval_ms: 1000,
                dispatcher_url: None,
                hostname: hostname::get()
                    .unwrap_or_else(|_| "unknown".into())
                    .to_string_lossy()
                    .to_string(),
                ip_address: "127.0.0.1".to_string(),
            },
        }
    }

    pub fn max_concurrent_tasks(mut self, max_concurrent_tasks: usize) -> Self {
        self.config.max_concurrent_tasks = max_concurrent_tasks;
        self
    }

    pub fn heartbeat_interval_seconds(mut self, heartbeat_interval_seconds: u64) -> Self {
        self.config.heartbeat_interval_seconds = heartbeat_interval_seconds;
        self
    }

    pub fn poll_interval_ms(mut self, poll_interval_ms: u64) -> Self {
        self.config.poll_interval_ms = poll_interval_ms;
        self
    }

    pub fn dispatcher_url(mut self, dispatcher_url: String) -> Self {
        self.config.dispatcher_url = Some(dispatcher_url);
        self
    }

    pub fn hostname(mut self, hostname: String) -> Self {
        self.config.hostname = hostname;
        self
    }

    pub fn ip_address(mut self, ip_address: String) -> Self {
        self.config.ip_address = ip_address;
        self
    }

    pub fn build(self) -> WorkerConfig {
        self.config
    }
}

pub struct WorkerServiceImpl {
    task_execution_manager: Arc<TaskExecutionManager>,
    dispatcher_client: Arc<DispatcherClient>,
    heartbeat_manager: Arc<HeartbeatManager>,
    worker_lifecycle: Arc<WorkerLifecycle>,
}

impl WorkerServiceImpl {
    pub fn new(
        config: WorkerConfig,
        service_locator: Arc<ServiceLocator>,
        executor_registry: Arc<dyn ExecutorRegistry>,
    ) -> Self {
        let task_execution_manager = Arc::new(TaskExecutionManager::new(
            config.worker_id.clone(),
            executor_registry,
            config.max_concurrent_tasks,
        ));

        let dispatcher_client = Arc::new(DispatcherClient::new(
            config.dispatcher_url,
            config.worker_id.clone(),
            config.hostname,
            config.ip_address,
        ));

        let heartbeat_manager = Arc::new(HeartbeatManager::new(
            config.worker_id.clone(),
            Arc::clone(&service_locator),
            config.status_queue,
            config.heartbeat_interval_seconds,
            Arc::clone(&dispatcher_client),
        ));

        let worker_lifecycle = Arc::new(WorkerLifecycle::new(
            config.worker_id,
            service_locator,
            config.task_queue,
            config.poll_interval_ms,
            Arc::clone(&task_execution_manager),
            Arc::clone(&dispatcher_client),
            Arc::clone(&heartbeat_manager),
        ));

        Self {
            task_execution_manager,
            dispatcher_client,
            heartbeat_manager,
            worker_lifecycle,
        }
    }
    pub fn builder(
        worker_id: String,
        service_locator: Arc<ServiceLocator>,
        task_queue: String,
        status_queue: String,
    ) -> WorkerServiceBuilder {
        WorkerServiceBuilder::new(worker_id, service_locator, task_queue, status_queue)
    }
}

#[async_trait::async_trait]
impl WorkerService for WorkerServiceImpl {
    async fn start(&self) -> SchedulerResult<()> {
        self.worker_lifecycle.start().await
    }

    async fn stop(&self) -> SchedulerResult<()> {
        self.worker_lifecycle.stop().await
    }

    async fn poll_and_execute_tasks(&self) -> SchedulerResult<()> {
        Ok(())
    }

    async fn send_status_update(&self, update: TaskStatusUpdate) -> SchedulerResult<()> {
        self.heartbeat_manager.send_status_update(update).await
    }

    async fn get_current_task_count(&self) -> i32 {
        self.task_execution_manager.get_current_task_count().await
    }

    async fn can_accept_task(&self, task_type: &str) -> bool {
        self.task_execution_manager.can_accept_task(task_type).await
    }

    async fn cancel_task(&self, task_run_id: i64) -> SchedulerResult<()> {
        self.task_execution_manager.cancel_task(task_run_id).await
    }

    async fn get_running_tasks(&self) -> Vec<TaskRun> {
        Vec::new()
    }

    async fn is_task_running(&self, _task_run_id: i64) -> bool {
        false
    }

    async fn send_heartbeat(&self) -> SchedulerResult<()> {
        let current_count = self.get_current_task_count().await;
        self.dispatcher_client.send_heartbeat(current_count).await
    }
}
impl WorkerServiceImpl {
    pub async fn get_supported_task_types(&self) -> Vec<String> {
        self.task_execution_manager.get_supported_task_types().await
    }
    pub async fn register_with_dispatcher(&self) -> SchedulerResult<()> {
        let supported_types = self.task_execution_manager.get_supported_task_types().await;
        self.dispatcher_client.register(supported_types).await
    }
    pub async fn send_heartbeat_to_dispatcher(&self) -> SchedulerResult<()> {
        let current_count = self.get_current_task_count().await;
        self.dispatcher_client.send_heartbeat(current_count).await
    }
    pub async fn unregister_from_dispatcher(&self) -> SchedulerResult<()> {
        self.dispatcher_client.unregister().await
    }
    pub async fn poll_and_execute_tasks(&self) -> SchedulerResult<()> {
        Ok(())
    }
}

pub struct WorkerServiceBuilder {
    config: WorkerConfig,
    service_locator: Arc<ServiceLocator>,
    executor_registry: Option<Arc<dyn ExecutorRegistry>>,
}

impl WorkerServiceBuilder {
    pub fn new(
        worker_id: String,
        service_locator: Arc<ServiceLocator>,
        task_queue: String,
        status_queue: String,
    ) -> Self {
        Self {
            config: WorkerConfig::builder(worker_id, task_queue, status_queue).build(),
            service_locator,
            executor_registry: None,
        }
    }

    pub fn with_executor_registry(mut self, registry: Arc<dyn ExecutorRegistry>) -> Self {
        self.executor_registry = Some(registry);
        self
    }

    pub fn max_concurrent_tasks(mut self, max_concurrent_tasks: usize) -> Self {
        self.config.max_concurrent_tasks = max_concurrent_tasks;
        self
    }

    pub fn heartbeat_interval_seconds(mut self, heartbeat_interval_seconds: u64) -> Self {
        self.config.heartbeat_interval_seconds = heartbeat_interval_seconds;
        self
    }

    pub fn poll_interval_ms(mut self, poll_interval_ms: u64) -> Self {
        self.config.poll_interval_ms = poll_interval_ms;
        self
    }

    pub fn dispatcher_url(mut self, dispatcher_url: String) -> Self {
        self.config.dispatcher_url = Some(dispatcher_url);
        self
    }

    pub fn hostname(mut self, hostname: String) -> Self {
        self.config.hostname = hostname;
        self
    }

    pub fn ip_address(mut self, ip_address: String) -> Self {
        self.config.ip_address = ip_address;
        self
    }

    pub async fn build(self) -> SchedulerResult<WorkerServiceImpl> {
        let executor_registry = self.executor_registry.ok_or_else(|| {
            scheduler_errors::SchedulerError::Internal("Executor registry is required".to_string())
        })?;

        Ok(WorkerServiceImpl::new(
            self.config,
            self.service_locator,
            executor_registry,
        ))
    }
}
