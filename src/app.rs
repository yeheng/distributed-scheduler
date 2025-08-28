use std::sync::Arc;

use anyhow::{Context, Result};
use scheduler_api::create_app;
use scheduler_application::{
    ExecutorRegistry, 
    scheduler::{StateListenerService, WorkerService},
    task_services::{TaskControlService, TaskSchedulerService}
};
use scheduler_config::AppConfig;
use scheduler_core::ServiceLocator;
use scheduler_dispatcher::{
    controller::TaskController, scheduler::TaskScheduler, state_listener::StateListener,
};
use scheduler_infrastructure::{
    database::postgres::{
        PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
    },
    message_queue::RabbitMQMessageQueue,
};
use scheduler_observability::MetricsCollector;
use scheduler_worker::WorkerServiceImpl;
use sqlx::PgPool;
use tokio::{net::TcpListener, sync::broadcast};
use tracing::{error, info};

/// 应用运行模式
#[derive(Debug, Clone)]
pub enum AppMode {
    /// 仅运行Dispatcher
    Dispatcher,
    /// 仅运行Worker
    Worker,
    /// 仅运行API服务器
    Api,
    /// 运行所有组件
    All,
}

/// 主应用程序
pub struct Application {
    config: AppConfig,
    mode: AppMode,
    service_locator: Arc<ServiceLocator>,
    metrics: Arc<MetricsCollector>,
}

impl Application {
    /// 创建新的应用实例
    pub async fn new(config: AppConfig, mode: AppMode) -> Result<Self> {
        info!("初始化应用程序，模式: {:?}", mode);

        // 创建数据库连接池
        let db_pool = create_database_pool(&config).await?;

        // 创建消息队列
        let message_queue = create_message_queue(&config).await?;

        // 创建Repository实例
        let task_repo = Arc::new(PostgresTaskRepository::new(db_pool.clone()));
        let task_run_repo = Arc::new(PostgresTaskRunRepository::new(db_pool.clone()));
        let worker_repo = Arc::new(PostgresWorkerRepository::new(db_pool.clone()));

        // 创建应用上下文
        let mut app_context = scheduler_core::ApplicationContext::new();
        app_context
            .register_core_services(task_repo, task_run_repo, worker_repo, message_queue)
            .await?;

        // 创建服务定位器
        let service_locator = Arc::new(scheduler_core::ServiceLocator::new(Arc::new(app_context)));

        // 创建指标收集器
        let metrics = Arc::new(MetricsCollector::new().context("创建指标收集器失败")?);

        Ok(Self {
            config,
            mode,
            service_locator,
            metrics,
        })
    }

    /// 运行应用程序
    pub async fn run(&self, shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        info!("启动应用程序，模式: {:?}", self.mode);

        match self.mode {
            AppMode::Dispatcher => {
                self.run_dispatcher(shutdown_rx).await?;
            }
            AppMode::Worker => {
                self.run_worker(shutdown_rx).await?;
            }
            AppMode::Api => {
                self.run_api(shutdown_rx).await?;
            }
            AppMode::All => {
                self.run_all_components(shutdown_rx).await?;
            }
        }

        Ok(())
    }

    /// 运行Dispatcher模式
    async fn run_dispatcher(&self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        info!("启动Dispatcher服务");

        // 创建任务调度器
        let task_scheduler = Arc::new(TaskScheduler::new(
            self.service_locator.task_repository().await?,
            self.service_locator.task_run_repository().await?,
            self.service_locator.message_queue().await?,
            self.config.message_queue.task_queue.clone(),
            Arc::clone(&self.metrics),
        ));

        // 创建状态监听器
        let state_listener = Arc::new(StateListener::new(
            self.service_locator.task_run_repository().await?,
            self.service_locator.worker_repository().await?,
            self.service_locator.message_queue().await?,
            self.config.message_queue.status_queue.clone(),
            self.config.message_queue.heartbeat_queue.clone(),
        ));

        // 启动调度器任务
        let scheduler_handle = {
            let scheduler = Arc::clone(&task_scheduler);
            let interval = self.config.dispatcher.schedule_interval_seconds;
            let shutdown_rx = shutdown_rx.resubscribe();

            tokio::spawn(async move {
                run_scheduler_loop(scheduler, interval, shutdown_rx).await;
            })
        };

        // 启动状态监听器任务
        let listener_handle = {
            let listener = Arc::clone(&state_listener);
            let shutdown_rx = shutdown_rx.resubscribe();

            tokio::spawn(async move {
                run_state_listener_loop(listener, shutdown_rx).await;
            })
        };

        // 等待关闭信号
        let _ = shutdown_rx.recv().await;
        info!("Dispatcher收到关闭信号");

        // 等待任务完成
        let _ = tokio::join!(scheduler_handle, listener_handle);

        info!("Dispatcher服务已停止");
        Ok(())
    }

    /// 运行Worker模式
    async fn run_worker(&self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        info!("启动Worker服务: {}", self.config.worker.worker_id);

        // 创建执行器工厂
        let executor_factory = Arc::new(scheduler_worker::ExecutorFactory::new(
            self.config.executor.clone(),
        ));
        executor_factory.initialize().await?;
        let executor_registry: Arc<dyn ExecutorRegistry> = executor_factory;

        // 创建Worker服务
        let worker_service = WorkerServiceImpl::builder(
            self.config.worker.worker_id.clone(),
            Arc::clone(&self.service_locator),
            self.config.message_queue.task_queue.clone(),
            self.config.message_queue.status_queue.clone(),
        )
        .with_executor_registry(executor_registry)
        .max_concurrent_tasks(self.config.worker.max_concurrent_tasks as usize)
        .heartbeat_interval_seconds(self.config.worker.heartbeat_interval_seconds)
        .hostname(self.config.worker.hostname.clone())
        .ip_address(self.config.worker.ip_address.clone())
        .build()
        .await?;

        // 启动Worker服务
        worker_service.start().await?;

        // 等待关闭信号
        let _ = shutdown_rx.recv().await;
        info!("Worker收到关闭信号");

        // 停止Worker服务
        worker_service.stop().await?;

        info!("Worker服务已停止");
        Ok(())
    }

    /// 运行API模式
    async fn run_api(&self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        info!("启动API服务器: {}", self.config.api.bind_address);

        // 创建任务控制服务
        let task_controller = Arc::new(TaskController::new(
            self.service_locator.task_repository().await?,
            self.service_locator.task_run_repository().await?,
            self.service_locator.message_queue().await?,
            self.config.message_queue.control_queue.clone(),
        ));

        // 创建API应用
        let app = create_app(
            self.service_locator.task_repository().await?,
            self.service_locator.task_run_repository().await?,
            self.service_locator.worker_repository().await?,
            task_controller as Arc<dyn TaskControlService>,
            self.config.api.clone(),
        );

        // 创建TCP监听器
        let listener = TcpListener::bind(&self.config.api.bind_address)
            .await
            .with_context(|| format!("绑定地址失败: {}", self.config.api.bind_address))?;

        info!("API服务器启动在 http://{}", self.config.api.bind_address);

        // 启动服务器
        let server_handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app.into_make_service()).await {
                error!("API服务器运行失败: {}", e);
            }
        });

        // 等待关闭信号
        let _ = shutdown_rx.recv().await;
        info!("API服务器收到关闭信号");

        // 停止服务器
        server_handle.abort();

        info!("API服务器已停止");
        Ok(())
    }

    /// 运行所有组件
    async fn run_all_components(&self, shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        info!("启动所有组件");

        let mut handles = Vec::new();

        // 启动Dispatcher（如果启用）
        if self.config.dispatcher.enabled {
            let app = self.clone_for_mode(AppMode::Dispatcher);
            let shutdown_rx = shutdown_rx.resubscribe();

            handles.push(tokio::spawn(async move {
                if let Err(e) = app.run_dispatcher(shutdown_rx).await {
                    error!("Dispatcher运行失败: {}", e);
                }
            }));
        }

        // 启动Worker（如果启用）
        if self.config.worker.enabled {
            let app = self.clone_for_mode(AppMode::Worker);
            let shutdown_rx = shutdown_rx.resubscribe();

            handles.push(tokio::spawn(async move {
                if let Err(e) = app.run_worker(shutdown_rx).await {
                    error!("Worker运行失败: {}", e);
                }
            }));
        }

        // 启动API服务器（如果启用）
        if self.config.api.enabled {
            let app = self.clone_for_mode(AppMode::Api);
            let shutdown_rx = shutdown_rx.resubscribe();

            handles.push(tokio::spawn(async move {
                if let Err(e) = app.run_api(shutdown_rx).await {
                    error!("API服务器运行失败: {}", e);
                }
            }));
        }

        // 等待所有组件完成
        for handle in handles {
            let _ = handle.await;
        }

        info!("所有组件已停止");
        Ok(())
    }

    /// 为特定模式克隆应用实例
    fn clone_for_mode(&self, mode: AppMode) -> Self {
        Self {
            config: self.config.clone(),
            mode,
            service_locator: Arc::clone(&self.service_locator),
            metrics: Arc::clone(&self.metrics),
        }
    }
}

/// 创建数据库连接池
async fn create_database_pool(config: &AppConfig) -> Result<PgPool> {
    info!("连接数据库: {}", mask_database_url(&config.database.url));

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .acquire_timeout(std::time::Duration::from_secs(
            config.database.connection_timeout_seconds,
        ))
        .idle_timeout(std::time::Duration::from_secs(
            config.database.idle_timeout_seconds,
        ))
        .connect(&config.database.url)
        .await
        .context("连接数据库失败")?;

    // 运行数据库迁移
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("运行数据库迁移失败")?;

    info!("数据库连接成功");
    Ok(pool)
}

/// 创建消息队列
async fn create_message_queue(config: &AppConfig) -> Result<Arc<RabbitMQMessageQueue>> {
    info!("连接消息队列: {}", mask_amqp_url(&config.message_queue.url));

    let message_queue = RabbitMQMessageQueue::new(config.message_queue.clone())
        .await
        .context("连接消息队列失败")?;

    info!("消息队列连接成功");
    Ok(Arc::new(message_queue))
}

/// 运行调度器循环
async fn run_scheduler_loop(
    scheduler: Arc<TaskScheduler>,
    interval_seconds: u64,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_seconds));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = scheduler.scan_and_schedule().await {
                    error!("任务调度失败: {}", e);
                }
            }
            _ = shutdown_rx.recv() => {
                info!("调度器循环收到关闭信号");
                break;
            }
        }
    }
}

/// 运行状态监听器循环
async fn run_state_listener_loop(
    listener: Arc<StateListener>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    tokio::select! {
        result = listener.listen_for_updates() => {
            if let Err(e) = result {
                error!("状态监听器失败: {}", e);
            }
        }
        _ = shutdown_rx.recv() => {
            info!("状态监听器收到关闭信号");
        }
    }
}

/// 屏蔽数据库URL中的敏感信息
fn mask_database_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let mut masked = url.to_string();
            masked.replace_range(colon_pos + 1..at_pos, "***");
            return masked;
        }
    }
    url.to_string()
}

/// 屏蔽AMQP URL中的敏感信息
fn mask_amqp_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let mut masked = url.to_string();
            masked.replace_range(colon_pos + 1..at_pos, "***");
            return masked;
        }
    }
    url.to_string()
}
