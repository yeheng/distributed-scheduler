use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    traits::{
        ExecutorFactory, ExecutorRegistry, MessageQueue, TaskRepository, TaskRunRepository,
        WorkerRepository,
    },
    Result, SchedulerError,
};

/// 依赖注入容器接口
#[async_trait]
pub trait Container: Send + Sync {
    /// 注册服务
    async fn register<T: Send + Sync + 'static>(
        &mut self,
        name: &str,
        service: Arc<T>,
    ) -> Result<()>;

    /// 获取服务
    async fn get<T: Send + Sync + 'static>(&self, name: &str) -> Result<Arc<T>>;

    /// 检查服务是否存在
    fn contains(&self, name: &str) -> bool;

    /// 获取所有服务名称
    fn list_services(&self) -> Vec<String>;

    /// 移除服务
    async fn remove(&mut self, name: &str) -> Result<bool>;

    /// 清空所有服务
    async fn clear(&mut self) -> Result<()>;
}

/// 服务容器实现
pub struct ServiceContainer {
    /// 核心服务存储
    task_repository: Option<Arc<dyn TaskRepository>>,
    task_run_repository: Option<Arc<dyn TaskRunRepository>>,
    worker_repository: Option<Arc<dyn WorkerRepository>>,
    message_queue: Option<Arc<dyn MessageQueue>>,
    /// 其他服务存储
    other_services: HashMap<String, Arc<dyn std::any::Any + Send + Sync>>,
    service_types: HashMap<String, &'static str>,
}

impl ServiceContainer {
    /// 创建新的服务容器
    pub fn new() -> Self {
        Self {
            task_repository: None,
            task_run_repository: None,
            worker_repository: None,
            message_queue: None,
            other_services: HashMap::new(),
            service_types: HashMap::new(),
        }
    }

    /// 使用默认服务创建容器
    pub async fn with_defaults() -> Result<Self> {
        let container = Self::new();
        // 这里可以注册默认服务
        Ok(container)
    }

    /// 注册任务仓库
    pub async fn register_task_repository(
        &mut self,
        service: Arc<dyn TaskRepository>,
    ) -> Result<()> {
        self.task_repository = Some(service);
        Ok(())
    }

    /// 注册任务运行仓库
    pub async fn register_task_run_repository(
        &mut self,
        service: Arc<dyn TaskRunRepository>,
    ) -> Result<()> {
        self.task_run_repository = Some(service);
        Ok(())
    }

    /// 注册Worker仓库
    pub async fn register_worker_repository(
        &mut self,
        service: Arc<dyn WorkerRepository>,
    ) -> Result<()> {
        self.worker_repository = Some(service);
        Ok(())
    }

    /// 注册消息队列
    pub async fn register_message_queue(&mut self, service: Arc<dyn MessageQueue>) -> Result<()> {
        self.message_queue = Some(service);
        Ok(())
    }

    /// 获取任务仓库
    pub async fn get_task_repository(&self) -> Result<Arc<dyn TaskRepository>> {
        self.task_repository
            .clone()
            .ok_or_else(|| SchedulerError::Internal("Task repository not registered".to_string()))
    }

    /// 获取任务运行仓库
    pub async fn get_task_run_repository(&self) -> Result<Arc<dyn TaskRunRepository>> {
        self.task_run_repository.clone().ok_or_else(|| {
            SchedulerError::Internal("Task run repository not registered".to_string())
        })
    }

    /// 获取Worker仓库
    pub async fn get_worker_repository(&self) -> Result<Arc<dyn WorkerRepository>> {
        self.worker_repository
            .clone()
            .ok_or_else(|| SchedulerError::Internal("Worker repository not registered".to_string()))
    }

    /// 获取消息队列
    pub async fn get_message_queue(&self) -> Result<Arc<dyn MessageQueue>> {
        self.message_queue
            .clone()
            .ok_or_else(|| SchedulerError::Internal("Message queue not registered".to_string()))
    }

    /// 通用注册方法（用于其他服务）
    pub async fn register<T: Send + Sync + 'static>(
        &mut self,
        name: &str,
        service: Arc<T>,
    ) -> Result<()> {
        let type_name = std::any::type_name::<T>();
        self.service_types.insert(name.to_string(), type_name);
        self.other_services.insert(
            name.to_string(),
            service as Arc<dyn std::any::Any + Send + Sync>,
        );
        Ok(())
    }

    /// 通用获取方法（用于其他服务）
    pub async fn get<T: Send + Sync + 'static>(&self, name: &str) -> Result<Arc<T>> {
        let service = self
            .other_services
            .get(name)
            .ok_or_else(|| SchedulerError::Internal(format!("Service '{name}' not found")))?;

        service.clone().downcast::<T>().map_err(|_| {
            let expected_type = std::any::type_name::<T>();
            let actual_type = self.service_types.get(name).unwrap_or(&"unknown");
            SchedulerError::Internal(format!(
                "Service '{name}' type mismatch: expected {expected_type}, got {actual_type}"
            ))
        })
    }

    /// 检查服务是否存在
    pub fn contains(&self, name: &str) -> bool {
        self.other_services.contains_key(name)
            || (name == "task_repository" && self.task_repository.is_some())
            || (name == "task_run_repository" && self.task_run_repository.is_some())
            || (name == "worker_repository" && self.worker_repository.is_some())
            || (name == "message_queue" && self.message_queue.is_some())
    }

    /// 获取所有服务名称
    pub fn list_services(&self) -> Vec<String> {
        let mut services = self.other_services.keys().cloned().collect::<Vec<_>>();
        if self.task_repository.is_some() {
            services.push("task_repository".to_string());
        }
        if self.task_run_repository.is_some() {
            services.push("task_run_repository".to_string());
        }
        if self.worker_repository.is_some() {
            services.push("worker_repository".to_string());
        }
        if self.message_queue.is_some() {
            services.push("message_queue".to_string());
        }
        services
    }

    /// 移除服务
    pub async fn remove(&mut self, name: &str) -> Result<bool> {
        // 检查并移除核心服务
        match name {
            "task_repository" => {
                if self.task_repository.is_some() {
                    self.task_repository = None;
                    return Ok(true);
                }
            }
            "task_run_repository" => {
                if self.task_run_repository.is_some() {
                    self.task_run_repository = None;
                    return Ok(true);
                }
            }
            "worker_repository" => {
                if self.worker_repository.is_some() {
                    self.worker_repository = None;
                    return Ok(true);
                }
            }
            "message_queue" => {
                if self.message_queue.is_some() {
                    self.message_queue = None;
                    return Ok(true);
                }
            }
            _ => {
                // 移除其他服务
                let removed = self.other_services.remove(name).is_some();
                if removed {
                    self.service_types.remove(name);
                }
                return Ok(removed);
            }
        }
        Ok(false)
    }

    /// 清空所有服务
    pub async fn clear(&mut self) -> Result<()> {
        self.task_repository = None;
        self.task_run_repository = None;
        self.worker_repository = None;
        self.message_queue = None;
        self.other_services.clear();
        self.service_types.clear();
        Ok(())
    }
}

impl Default for ServiceContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Container for ServiceContainer {
    /// 注册服务
    async fn register<T: Send + Sync + 'static>(
        &mut self,
        name: &str,
        service: Arc<T>,
    ) -> Result<()> {
        self.register(name, service).await
    }

    /// 获取服务
    async fn get<T: Send + Sync + 'static>(&self, name: &str) -> Result<Arc<T>> {
        self.get(name).await
    }

    /// 检查服务是否存在
    fn contains(&self, name: &str) -> bool {
        self.contains(name)
    }

    /// 获取所有服务名称
    fn list_services(&self) -> Vec<String> {
        self.list_services()
    }

    /// 移除服务
    async fn remove(&mut self, name: &str) -> Result<bool> {
        self.remove(name).await
    }

    /// 清空所有服务
    async fn clear(&mut self) -> Result<()> {
        self.clear().await
    }
}

/// 应用程序上下文 - 管理应用程序级别的依赖
pub struct ApplicationContext {
    /// 服务容器
    container: ServiceContainer,
    /// 执行器注册表
    executor_registry: Option<Box<dyn ExecutorRegistry>>,
    /// 执行器工厂
    executor_factory: Option<Box<dyn ExecutorFactory>>,
}

impl ApplicationContext {
    /// 创建新的应用程序上下文
    pub fn new() -> Self {
        Self {
            container: ServiceContainer::new(),
            executor_registry: None,
            executor_factory: None,
        }
    }

    /// 创建带有自定义容器的应用程序上下文
    pub fn with_container(container: ServiceContainer) -> Self {
        Self {
            container,
            executor_registry: None,
            executor_factory: None,
        }
    }

    /// 设置执行器注册表
    pub fn with_executor_registry(mut self, registry: Box<dyn ExecutorRegistry>) -> Self {
        self.executor_registry = Some(registry);
        self
    }

    /// 设置执行器工厂
    pub fn with_executor_factory(mut self, factory: Box<dyn ExecutorFactory>) -> Self {
        self.executor_factory = Some(factory);
        self
    }

    /// 获取服务容器
    pub fn container(&self) -> &ServiceContainer {
        &self.container
    }

    /// 获取可变服务容器
    pub fn container_mut(&mut self) -> &mut ServiceContainer {
        &mut self.container
    }

    /// 获取执行器注册表
    pub fn executor_registry(&self) -> Option<&dyn ExecutorRegistry> {
        self.executor_registry.as_ref().map(|r| r.as_ref())
    }

    /// 获取执行器工厂
    pub fn executor_factory(&self) -> Option<&dyn ExecutorFactory> {
        self.executor_factory.as_ref().map(|f| f.as_ref())
    }

    /// 注册核心服务
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

    /// 获取任务仓库
    pub async fn get_task_repository(&self) -> Result<Arc<dyn TaskRepository>> {
        self.container.get_task_repository().await
    }

    /// 获取任务运行仓库
    pub async fn get_task_run_repository(&self) -> Result<Arc<dyn TaskRunRepository>> {
        self.container.get_task_run_repository().await
    }

    /// 获取Worker仓库
    pub async fn get_worker_repository(&self) -> Result<Arc<dyn WorkerRepository>> {
        self.container.get_worker_repository().await
    }

    /// 获取消息队列
    pub async fn get_message_queue(&self) -> Result<Arc<dyn MessageQueue>> {
        self.container.get_message_queue().await
    }

    /// 初始化应用程序
    pub async fn initialize(&mut self) -> Result<()> {
        // 预热执行器
        if let Some(registry) = &self.executor_registry {
            let executors = registry.list_executors().await;
            for executor_name in executors {
                if let Some(executor) = registry.get(&executor_name).await {
                    if let Err(e) = executor.warm_up().await {
                        eprintln!("Failed to warm up executor '{executor_name}': {e}");
                    }
                }
            }
        }

        Ok(())
    }

    /// 清理资源
    pub async fn cleanup(&mut self) -> Result<()> {
        // 清理执行器
        if let Some(registry) = &self.executor_registry {
            let executors = registry.list_executors().await;
            for executor_name in executors {
                if let Some(executor) = registry.get(&executor_name).await {
                    if let Err(e) = executor.cleanup().await {
                        eprintln!("Failed to cleanup executor '{executor_name}': {e}");
                    }
                }
            }
        }

        // 清理容器
        self.container.clear().await?;

        Ok(())
    }
}

impl Default for ApplicationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 服务定位器 - 简化的服务访问
pub struct ServiceLocator {
    context: Arc<ApplicationContext>,
}

impl ServiceLocator {
    /// 创建新的服务定位器
    pub fn new(context: Arc<ApplicationContext>) -> Self {
        Self { context }
    }

    /// 获取服务
    pub async fn get<T: Send + Sync + 'static>(&self, name: &str) -> Result<Arc<T>> {
        self.context.container().get(name).await
    }

    /// 获取任务仓库
    pub async fn task_repository(&self) -> Result<Arc<dyn TaskRepository>> {
        self.context.get_task_repository().await
    }

    /// 获取任务运行仓库
    pub async fn task_run_repository(&self) -> Result<Arc<dyn TaskRunRepository>> {
        self.context.get_task_run_repository().await
    }

    /// 获取Worker仓库
    pub async fn worker_repository(&self) -> Result<Arc<dyn WorkerRepository>> {
        self.context.get_worker_repository().await
    }

    /// 获取消息队列
    pub async fn message_queue(&self) -> Result<Arc<dyn MessageQueue>> {
        self.context.get_message_queue().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_container() {
        let mut container = ServiceContainer::new();

        // 注册服务
        let service = Arc::new("test_service".to_string());
        container.register("test", service.clone()).await.unwrap();

        // 检查服务存在
        assert!(container.contains("test"));

        // 获取服务
        let retrieved: Arc<String> = container.get("test").await.unwrap();
        assert_eq!(*retrieved, "test_service");

        // 移除服务
        assert!(container.remove("test").await.unwrap());
        assert!(!container.contains("test"));
    }

    #[tokio::test]
    async fn test_service_container_type_safety() {
        let mut container = ServiceContainer::new();

        // 注册字符串服务
        let service = Arc::new("test_service".to_string());
        container.register("test", service).await.unwrap();

        // 尝试以错误类型获取服务应该失败
        let result: Result<Arc<i32>> = container.get("test").await;
        assert!(result.is_err());
    }
}
