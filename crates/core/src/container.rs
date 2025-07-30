use std::sync::Arc;

use crate::{
    errors::{Result, SchedulerError},
    traits::{MessageQueue, TaskRepository, TaskRunRepository, WorkerRepository},
};

/// Simplified service container - stores only essential services
/// Follows KISS principle: Simple structure with only required services
pub struct ServiceContainer {
    task_repository: Option<Arc<dyn TaskRepository>>,
    task_run_repository: Option<Arc<dyn TaskRunRepository>>,
    worker_repository: Option<Arc<dyn WorkerRepository>>,
    message_queue: Option<Arc<dyn MessageQueue>>,
}

impl ServiceContainer {
    /// Create new service container
    pub fn new() -> Self {
        Self {
            task_repository: None,
            task_run_repository: None,
            worker_repository: None,
            message_queue: None,
        }
    }

    /// Register task repository
    pub async fn register_task_repository(
        &mut self,
        service: Arc<dyn TaskRepository>,
    ) -> Result<()> {
        self.task_repository = Some(service);
        Ok(())
    }

    /// Register task run repository
    pub async fn register_task_run_repository(
        &mut self,
        service: Arc<dyn TaskRunRepository>,
    ) -> Result<()> {
        self.task_run_repository = Some(service);
        Ok(())
    }

    /// Register worker repository
    pub async fn register_worker_repository(
        &mut self,
        service: Arc<dyn WorkerRepository>,
    ) -> Result<()> {
        self.worker_repository = Some(service);
        Ok(())
    }

    /// Register message queue
    pub async fn register_message_queue(&mut self, service: Arc<dyn MessageQueue>) -> Result<()> {
        self.message_queue = Some(service);
        Ok(())
    }

    /// Get task repository
    pub async fn get_task_repository(&self) -> Result<Arc<dyn TaskRepository>> {
        self.task_repository
            .clone()
            .ok_or_else(|| SchedulerError::Internal("Task repository not registered".to_string()))
    }

    /// Get task run repository
    pub async fn get_task_run_repository(&self) -> Result<Arc<dyn TaskRunRepository>> {
        self.task_run_repository.clone().ok_or_else(|| {
            SchedulerError::Internal("Task run repository not registered".to_string())
        })
    }

    /// Get worker repository
    pub async fn get_worker_repository(&self) -> Result<Arc<dyn WorkerRepository>> {
        self.worker_repository
            .clone()
            .ok_or_else(|| SchedulerError::Internal("Worker repository not registered".to_string()))
    }

    /// Get message queue
    pub async fn get_message_queue(&self) -> Result<Arc<dyn MessageQueue>> {
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

/// Simplified application context - Only manages core dependencies
/// Follows SRP: Single responsibility for dependency management
pub struct ApplicationContext {
    container: ServiceContainer,
}

impl ApplicationContext {
    /// Create new application context
    pub fn new() -> Self {
        Self {
            container: ServiceContainer::new(),
        }
    }

    /// Create with custom container
    pub fn with_container(container: ServiceContainer) -> Self {
        Self { container }
    }

    /// Get service container reference
    pub fn container(&self) -> &ServiceContainer {
        &self.container
    }

    /// Get mutable service container reference
    pub fn container_mut(&mut self) -> &mut ServiceContainer {
        &mut self.container
    }

    /// Register all core services at once - convenience method
    pub async fn register_core_services(
        &mut self,
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
        worker_repo: Arc<dyn WorkerRepository>,
        message_queue: Arc<dyn MessageQueue>,
    ) -> Result<()> {
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

    /// Get task repository
    pub async fn get_task_repository(&self) -> Result<Arc<dyn TaskRepository>> {
        self.container.get_task_repository().await
    }

    /// Get task run repository
    pub async fn get_task_run_repository(&self) -> Result<Arc<dyn TaskRunRepository>> {
        self.container.get_task_run_repository().await
    }

    /// Get worker repository
    pub async fn get_worker_repository(&self) -> Result<Arc<dyn WorkerRepository>> {
        self.container.get_worker_repository().await
    }

    /// Get message queue
    pub async fn get_message_queue(&self) -> Result<Arc<dyn MessageQueue>> {
        self.container.get_message_queue().await
    }
}

impl Default for ApplicationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Service locator - Direct access to services without extra wrapper complexity
/// Follows KISS: Simple wrapper around ApplicationContext for convenient access
pub struct ServiceLocator {
    context: Arc<ApplicationContext>,
}

impl ServiceLocator {
    /// Create new service locator
    pub fn new(context: Arc<ApplicationContext>) -> Self {
        Self { context }
    }

    /// Get task repository
    pub async fn task_repository(&self) -> Result<Arc<dyn TaskRepository>> {
        self.context.get_task_repository().await
    }

    /// Get task run repository
    pub async fn task_run_repository(&self) -> Result<Arc<dyn TaskRunRepository>> {
        self.context.get_task_run_repository().await
    }

    /// Get worker repository
    pub async fn worker_repository(&self) -> Result<Arc<dyn WorkerRepository>> {
        self.context.get_worker_repository().await
    }

    /// Get message queue
    pub async fn message_queue(&self) -> Result<Arc<dyn MessageQueue>> {
        self.context.get_message_queue().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_container_basic_functionality() {
        let container = ServiceContainer::new();

        // Test that accessing unregistered services returns error
        let result = container.get_message_queue().await;
        assert!(result.is_err());
        // Check the error message without relying on Debug
        match result {
            Err(err) => assert!(err.to_string().contains("Message queue not registered")),
            Ok(_) => panic!("Expected error for unregistered service"),
        }
    }

    #[tokio::test]
    async fn test_application_context_creation() {
        let context = ApplicationContext::new();
        let service_locator = ServiceLocator::new(Arc::new(context));

        // Test that unregistered services return errors through ServiceLocator
        let result = service_locator.message_queue().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_container_default() {
        let container = ServiceContainer::default();
        // Test that default container is properly initialized
        let result = container.get_task_repository().await;
        assert!(result.is_err());
    }
}
