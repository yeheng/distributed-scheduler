use std::sync::Arc;

use crate::{traits::MessageQueue, SchedulerResult};
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository, WorkerRepository};
use scheduler_errors::SchedulerError;

pub struct ServiceContainer {
    task_repository: Option<Arc<dyn TaskRepository>>,
    task_run_repository: Option<Arc<dyn TaskRunRepository>>,
    worker_repository: Option<Arc<dyn WorkerRepository>>,
    message_queue: Option<Arc<dyn MessageQueue>>,
}

impl ServiceContainer {
    pub fn new() -> Self {
        Self {
            task_repository: None,
            task_run_repository: None,
            worker_repository: None,
            message_queue: None,
        }
    }
    pub async fn register_task_repository(
        &mut self,
        service: Arc<dyn TaskRepository>,
    ) -> SchedulerResult<()> {
        self.task_repository = Some(service);
        Ok(())
    }
    pub async fn register_task_run_repository(
        &mut self,
        service: Arc<dyn TaskRunRepository>,
    ) -> SchedulerResult<()> {
        self.task_run_repository = Some(service);
        Ok(())
    }
    pub async fn register_worker_repository(
        &mut self,
        service: Arc<dyn WorkerRepository>,
    ) -> SchedulerResult<()> {
        self.worker_repository = Some(service);
        Ok(())
    }
    pub async fn register_message_queue(
        &mut self,
        service: Arc<dyn MessageQueue>,
    ) -> SchedulerResult<()> {
        self.message_queue = Some(service);
        Ok(())
    }
    pub async fn get_task_repository(&self) -> SchedulerResult<Arc<dyn TaskRepository>> {
        self.task_repository
            .clone()
            .ok_or_else(|| SchedulerError::Internal("Task repository not registered".to_string()))
    }
    pub async fn get_task_run_repository(&self) -> SchedulerResult<Arc<dyn TaskRunRepository>> {
        self.task_run_repository.clone().ok_or_else(|| {
            SchedulerError::Internal("Task run repository not registered".to_string())
        })
    }
    pub async fn get_worker_repository(&self) -> SchedulerResult<Arc<dyn WorkerRepository>> {
        self.worker_repository
            .clone()
            .ok_or_else(|| SchedulerError::Internal("Worker repository not registered".to_string()))
    }
    pub async fn get_message_queue(&self) -> SchedulerResult<Arc<dyn MessageQueue>> {
        self.message_queue
            .clone()
            .ok_or_else(|| SchedulerError::Internal("Message queue not registered".to_string()))
    }
}

impl Default for ServiceContainer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ApplicationContext {
    container: ServiceContainer,
}

impl ApplicationContext {
    pub fn new() -> Self {
        Self {
            container: ServiceContainer::new(),
        }
    }
    pub fn with_container(container: ServiceContainer) -> Self {
        Self { container }
    }
    pub fn container(&self) -> &ServiceContainer {
        &self.container
    }
    pub fn container_mut(&mut self) -> &mut ServiceContainer {
        &mut self.container
    }
    pub async fn register_core_services(
        &mut self,
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
        worker_repo: Arc<dyn WorkerRepository>,
        message_queue: Arc<dyn MessageQueue>,
    ) -> SchedulerResult<()> {
        self.container.register_task_repository(task_repo).await?;
        self.container
            .register_task_run_repository(task_run_repo)
            .await?;
        self.container
            .register_worker_repository(worker_repo)
            .await?;
        self.container.register_message_queue(message_queue).await?;
        Ok(())
    }
    pub async fn get_task_repository(&self) -> SchedulerResult<Arc<dyn TaskRepository>> {
        self.container.get_task_repository().await
    }
    pub async fn get_task_run_repository(&self) -> SchedulerResult<Arc<dyn TaskRunRepository>> {
        self.container.get_task_run_repository().await
    }
    pub async fn get_worker_repository(&self) -> SchedulerResult<Arc<dyn WorkerRepository>> {
        self.container.get_worker_repository().await
    }
    pub async fn get_message_queue(&self) -> SchedulerResult<Arc<dyn MessageQueue>> {
        self.container.get_message_queue().await
    }
}

impl Default for ApplicationContext {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ServiceLocator {
    context: Arc<ApplicationContext>,
}

impl ServiceLocator {
    pub fn new(context: Arc<ApplicationContext>) -> Self {
        Self { context }
    }
    pub async fn task_repository(&self) -> SchedulerResult<Arc<dyn TaskRepository>> {
        self.context.get_task_repository().await
    }
    pub async fn task_run_repository(&self) -> SchedulerResult<Arc<dyn TaskRunRepository>> {
        self.context.get_task_run_repository().await
    }
    pub async fn worker_repository(&self) -> SchedulerResult<Arc<dyn WorkerRepository>> {
        self.context.get_worker_repository().await
    }
    pub async fn message_queue(&self) -> SchedulerResult<Arc<dyn MessageQueue>> {
        self.context.get_message_queue().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_container_basic_functionality() {
        let container = ServiceContainer::new();
        let result = container.get_message_queue().await;
        assert!(result.is_err());
        match result {
            Err(err) => assert!(err.to_string().contains("Message queue not registered")),
            Ok(_) => panic!("Expected error for unregistered service"),
        }
    }

    #[tokio::test]
    async fn test_application_context_creation() {
        let context = ApplicationContext::new();
        let service_locator = ServiceLocator::new(Arc::new(context));
        let result = service_locator.message_queue().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_container_default() {
        let container = ServiceContainer::default();
        let result = container.get_task_repository().await;
        assert!(result.is_err());
    }
}
