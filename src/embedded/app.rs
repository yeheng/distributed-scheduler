use anyhow::{Context, Result};
use scheduler_config::AppConfig;
use scheduler_core::{ApplicationContext, ServiceLocator};
use scheduler_dispatcher;
use scheduler_infrastructure::{
    database::sqlite::{SqliteTaskRepository, SqliteTaskRunRepository, SqliteWorkerRepository},
    in_memory_queue::InMemoryMessageQueue,
};
use scheduler_observability::MetricsCollector;
use scheduler_worker::WorkerServiceImpl;
use scheduler_application::{ExecutorRegistry, scheduler::WorkerService};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::{time::{sleep, Duration}, sync::RwLock};
use tracing::{error, info, warn};

// Import the shutdown module from the parent crate
use crate::shutdown::EmbeddedShutdownManager;

/// 嵌入式应用程序
pub struct EmbeddedApplication {
    config: AppConfig,
    service_locator: Option<Arc<ServiceLocator>>,
    metrics: Option<Arc<MetricsCollector>>,
}

/// 嵌入式应用程序句柄
pub struct EmbeddedApplicationHandle {
    api_address: String,
    service_locator: Arc<ServiceLocator>,
    shutdown_manager: EmbeddedShutdownManager,
    worker_service: Option<Arc<WorkerServiceImpl>>,
    running_tasks: Arc<RwLock<HashMap<i64, scheduler_domain::entities::TaskRun>>>,
    database_pool: sqlx::SqlitePool,
}

impl EmbeddedApplication {
    /// 创建新的嵌入式应用实例
    pub async fn new(config: AppConfig) -> Result<Self> {
        info!("初始化嵌入式应用程序");

        Ok(Self {
            config,
            service_locator: None,
            metrics: None,
        })
    }

    /// 创建数据库连接池并运行迁移
    async fn create_database_pool(&self, database_url: &str) -> Result<sqlx::SqlitePool> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;

        info!("创建SQLite数据库连接池");

        // 创建连接选项，启用外键约束和WAL模式
        let connect_options = SqliteConnectOptions::from_str(database_url)
            .context("解析数据库URL失败")?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        // 创建连接池
        let pool = SqlitePoolOptions::new()
            .max_connections(self.config.database.max_connections)
            .min_connections(self.config.database.min_connections)
            .connect_with(connect_options)
            .await
            .context("创建数据库连接池失败")?;

        // 运行数据库迁移
        self.run_database_migrations(&pool).await
            .context("运行数据库迁移失败")?;

        info!("✅ 数据库连接池创建完成");
        Ok(pool)
    }

    /// 运行数据库迁移
    async fn run_database_migrations(&self, pool: &sqlx::SqlitePool) -> Result<()> {
        info!("运行SQLite数据库迁移");

        // 创建任务表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                task_type TEXT NOT NULL,
                schedule TEXT NOT NULL,
                parameters TEXT NOT NULL DEFAULT '{}',
                timeout_seconds INTEGER NOT NULL DEFAULT 300,
                max_retries INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'ACTIVE',
                dependencies TEXT DEFAULT '[]',
                shard_config TEXT,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await
        .context("创建任务表失败")?;

        // 创建任务执行记录表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                status TEXT NOT NULL,
                worker_id TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                shard_index INTEGER,
                shard_total INTEGER,
                scheduled_at DATETIME NOT NULL,
                started_at DATETIME,
                completed_at DATETIME,
                result TEXT,
                error_message TEXT,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (task_id) REFERENCES tasks (id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(pool)
        .await
        .context("创建任务执行记录表失败")?;

        // 创建Worker表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workers (
                id TEXT PRIMARY KEY,
                hostname TEXT NOT NULL,
                ip_address TEXT NOT NULL,
                status TEXT NOT NULL,
                supported_task_types TEXT NOT NULL,
                max_concurrent_tasks INTEGER NOT NULL,
                current_task_count INTEGER NOT NULL DEFAULT 0,
                last_heartbeat DATETIME NOT NULL,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await
        .context("创建Worker表失败")?;

        // 创建系统状态表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS system_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await
        .context("创建系统状态表失败")?;

        // 创建索引
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)")
            .execute(pool)
            .await
            .context("创建任务状态索引失败")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_name ON tasks(name)")
            .execute(pool)
            .await
            .context("创建任务名称索引失败")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_task_runs_task_id ON task_runs(task_id)")
            .execute(pool)
            .await
            .context("创建任务执行记录任务ID索引失败")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_task_runs_status ON task_runs(status)")
            .execute(pool)
            .await
            .context("创建任务执行记录状态索引失败")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status)")
            .execute(pool)
            .await
            .context("创建Worker状态索引失败")?;

        info!("✅ 数据库迁移完成");
        Ok(())
    }

    /// 启动嵌入式应用
    pub async fn start(mut self) -> Result<EmbeddedApplicationHandle> {
        info!("启动嵌入式应用组件...");

        // 1. 初始化数据库
        let database_url = &self.config.database.url;
        info!("初始化SQLite数据库: {}", database_url);

        // 创建共享的数据库连接池
        let pool = self.create_database_pool(database_url).await
            .context("创建数据库连接池失败")?;

        let task_repository = Arc::new(SqliteTaskRepository::new(pool.clone()));
        let task_run_repository = Arc::new(SqliteTaskRunRepository::new(pool.clone()));
        let worker_repository = Arc::new(SqliteWorkerRepository::new(pool.clone()));

        info!("✅ 数据库初始化完成");

        // 2. 初始化内存消息队列
        info!("初始化内存消息队列");
        let message_queue = Arc::new(InMemoryMessageQueue::new());
        info!("✅ 内存消息队列初始化完成");

        // 3. 创建应用上下文和服务定位器
        let mut app_context = ApplicationContext::new();
        app_context
            .register_core_services(task_repository, task_run_repository, worker_repository, message_queue)
            .await
            .context("注册核心服务失败")?;

        let service_locator = Arc::new(ServiceLocator::new(Arc::new(app_context)));

        // 4. 创建指标收集器
        let metrics = Arc::new(MetricsCollector::new().context("创建指标收集器失败")?);

        self.metrics = Some(metrics.clone());

        // 5. 启动各个组件
        self.start_components(&service_locator, &metrics).await
            .context("启动应用组件失败")?;

        // 6. 创建嵌入式关闭管理器
        let shutdown_manager = EmbeddedShutdownManager::with_timeouts(
            30, // 30秒等待任务完成
            60, // 60秒强制关闭超时
        );

        // 7. 创建运行任务跟踪器
        let running_tasks = Arc::new(RwLock::new(HashMap::new()));

        // 8. 启动Worker服务（如果启用）
        let worker_service = if self.config.worker.enabled {
            info!("启动Worker服务");
            let worker_service = self.start_worker_service(&service_locator, &running_tasks).await
                .context("启动Worker服务失败")?;
            Some(worker_service)
        } else {
            None
        };

        self.service_locator = Some(service_locator.clone());

        Ok(EmbeddedApplicationHandle {
            api_address: self.config.api.bind_address.clone(),
            service_locator,
            shutdown_manager,
            worker_service,
            running_tasks,
            database_pool: pool,
        })
    }

    /// 启动各个应用组件
    async fn start_components(&self, service_locator: &Arc<ServiceLocator>, metrics: &Arc<MetricsCollector>) -> Result<()> {
        // 启动API服务器
        if self.config.api.enabled {
            info!("启动API服务器: {}", self.config.api.bind_address);
            self.start_api_server(service_locator).await
                .context("启动API服务器失败")?;
            info!("✅ API服务器启动完成");
        }

        // 启动调度器
        if self.config.dispatcher.enabled {
            info!("启动任务调度器");
            self.start_dispatcher(service_locator, metrics).await
                .context("启动调度器失败")?;
            info!("✅ 任务调度器启动完成");
        }

        // 启动Worker
        if self.config.worker.enabled {
            info!("启动任务执行器");
            self.start_worker(service_locator).await
                .context("启动Worker失败")?;
            info!("✅ 任务执行器启动完成");
        }

        Ok(())
    }

    /// 启动API服务器
    async fn start_api_server(&self, service_locator: &Arc<ServiceLocator>) -> Result<()> {
        use scheduler_api::create_app;
        use tokio::net::TcpListener;

        // 获取各个仓库
        let task_repo = service_locator.task_repository().await
            .context("获取任务仓库失败")?;
        let task_run_repo = service_locator.task_run_repository().await
            .context("获取任务执行仓库失败")?;
        let worker_repo = service_locator.worker_repository().await
            .context("获取Worker仓库失败")?;

        // 创建任务控制服务
        let message_queue = service_locator.message_queue().await.context("获取消息队列失败")?;
        let task_control_service = Arc::new(scheduler_dispatcher::controller::TaskController::new(
            task_repo.clone(),
            task_run_repo.clone(),
            message_queue,
            "control".to_string(), // 控制队列名称
        ));

        let app = create_app(
            task_repo,
            task_run_repo,
            worker_repo,
            task_control_service,
            self.config.api.clone(),
        );

        let listener = TcpListener::bind(&self.config.api.bind_address)
            .await
            .context("绑定API服务器地址失败")?;

        // 在后台启动服务器
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("API服务器运行错误: {}", e);
            }
        });

        // 等待服务器启动
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// 启动调度器
    async fn start_dispatcher(&self, _service_locator: &Arc<ServiceLocator>, _metrics: &Arc<MetricsCollector>) -> Result<()> {
        info!("调度器启动功能暂未实现");
        // TODO: 实现调度器启动逻辑
        Ok(())
    }

    /// 启动Worker
    async fn start_worker(&self, _service_locator: &Arc<ServiceLocator>) -> Result<()> {
        info!("Worker启动功能暂未实现");
        // TODO: 实现Worker启动逻辑
        Ok(())
    }

    /// 启动Worker服务
    async fn start_worker_service(
        &self,
        service_locator: &Arc<ServiceLocator>,
        _running_tasks: &Arc<RwLock<HashMap<i64, scheduler_domain::entities::TaskRun>>>,
    ) -> Result<Arc<WorkerServiceImpl>> {
        // 创建执行器工厂
        let executor_factory = Arc::new(scheduler_worker::ExecutorFactory::new(
            self.config.executor.clone(),
        ));
        executor_factory.initialize().await
            .context("初始化执行器工厂失败")?;
        
        let executor_registry: Arc<dyn ExecutorRegistry> = executor_factory;

        // 创建Worker服务
        let worker_service = WorkerServiceImpl::builder(
            self.config.worker.worker_id.clone(),
            Arc::clone(service_locator),
            self.config.message_queue.task_queue.clone(),
            self.config.message_queue.status_queue.clone(),
        )
        .with_executor_registry(executor_registry)
        .max_concurrent_tasks(self.config.worker.max_concurrent_tasks)
        .heartbeat_interval_seconds(self.config.worker.heartbeat_interval_seconds)
        .hostname(self.config.worker.hostname.clone())
        .ip_address(self.config.worker.ip_address.clone())
        .build()
        .await
        .context("构建Worker服务失败")?;

        // 启动Worker服务
        worker_service.start().await
            .context("启动Worker服务失败")?;

        info!("✅ Worker服务启动完成");
        Ok(Arc::new(worker_service))
    }
}

impl EmbeddedApplicationHandle {
    /// 获取API服务器地址
    pub fn api_address(&self) -> &str {
        &self.api_address
    }

    /// 优雅关闭应用
    pub async fn shutdown(self) -> Result<()> {
        info!("开始优雅关闭嵌入式应用...");

        let wait_for_tasks = self.wait_for_running_tasks();
        let persist_state = self.persist_system_state();
        let force_cleanup = self.cleanup_resources();

        match self.shutdown_manager.graceful_shutdown(wait_for_tasks, persist_state, force_cleanup).await {
            Ok(()) => {
                info!("✅ 嵌入式应用优雅关闭完成");
                Ok(())
            }
            Err(e) => {
                error!("优雅关闭过程中发生错误: {}", e);
                
                // 尝试强制关闭
                warn!("尝试强制关闭...");
                let force_cleanup = self.cleanup_resources();
                
                match self.shutdown_manager.force_shutdown(force_cleanup).await {
                    Ok(()) => {
                        warn!("强制关闭完成");
                        Ok(())
                    }
                    Err(force_err) => {
                        error!("强制关闭也失败了: {}", force_err);
                        Err(anyhow::anyhow!("关闭失败: 优雅关闭错误: {}, 强制关闭错误: {}", e, force_err))
                    }
                }
            }
        }
    }

    /// 等待正在执行的任务完成
    async fn wait_for_running_tasks(&self) -> Result<(), String> {
        info!("等待正在执行的任务完成...");

        // 1. 停止Worker服务接收新任务
        if let Some(ref worker_service) = self.worker_service {
            info!("停止Worker服务");
            if let Err(e) = worker_service.stop().await {
                warn!("停止Worker服务时发生错误: {}", e);
            }
        }

        // 2. 等待所有正在执行的任务完成
        let mut check_count = 0;
        const MAX_CHECKS: u32 = 300; // 最多检查300次（每次100ms，总共30秒）
        
        loop {
            let running_count = {
                let tasks = self.running_tasks.read().await;
                tasks.len()
            };

            if running_count == 0 {
                info!("所有任务已完成");
                break;
            }

            check_count += 1;
            if check_count >= MAX_CHECKS {
                return Err(format!("等待任务完成超时，仍有 {} 个任务在运行", running_count));
            }

            if check_count % 50 == 0 { // 每5秒打印一次状态
                info!("仍有 {} 个任务在运行，继续等待...", running_count);
            }

            sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    /// 持久化系统状态到数据库
    async fn persist_system_state(&self) -> Result<(), String> {
        info!("持久化系统状态到数据库...");

        // 1. 保存关闭时间戳
        let shutdown_timestamp = chrono::Utc::now().to_rfc3339();
        
        if let Err(e) = sqlx::query(
            "INSERT OR REPLACE INTO system_state (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)"
        )
        .bind("last_shutdown")
        .bind(&shutdown_timestamp)
        .execute(&self.database_pool)
        .await
        {
            return Err(format!("保存关闭时间戳失败: {}", e));
        }

        // 2. 保存应用版本信息
        let app_version = env!("CARGO_PKG_VERSION");
        if let Err(e) = sqlx::query(
            "INSERT OR REPLACE INTO system_state (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)"
        )
        .bind("app_version")
        .bind(app_version)
        .execute(&self.database_pool)
        .await
        {
            return Err(format!("保存应用版本失败: {}", e));
        }

        // 3. 更新所有运行中的任务状态为失败（如果有的话）
        let running_tasks = self.running_tasks.read().await;
        if !running_tasks.is_empty() {
            warn!("发现 {} 个未完成的任务，将其标记为失败", running_tasks.len());
            
            for (task_run_id, _) in running_tasks.iter() {
                if let Err(e) = sqlx::query(
                    "UPDATE task_runs SET status = 'FAILED', error_message = 'Application shutdown', completed_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'RUNNING'"
                )
                .bind(task_run_id)
                .execute(&self.database_pool)
                .await
                {
                    warn!("更新任务 {} 状态失败: {}", task_run_id, e);
                }
            }
        }

        // 4. 清理过期数据（可选）
        let retention_days = 30; // 保留30天的数据
        if let Err(e) = sqlx::query(
            "DELETE FROM task_runs WHERE created_at < datetime('now', '-' || ? || ' days')"
        )
        .bind(retention_days)
        .execute(&self.database_pool)
        .await
        {
            warn!("清理过期任务执行记录失败: {}", e);
        }

        info!("系统状态持久化完成");
        Ok(())
    }

    /// 清理资源
    async fn cleanup_resources(&self) -> Result<(), String> {
        info!("清理应用资源...");

        // 1. 关闭数据库连接池
        info!("关闭数据库连接池");
        self.database_pool.close().await;

        // 2. 清理运行任务跟踪器
        {
            let mut tasks = self.running_tasks.write().await;
            tasks.clear();
        }

        info!("资源清理完成");
        Ok(())
    }

    /// 获取服务定位器（用于测试）
    #[cfg(test)]
    pub fn service_locator(&self) -> &Arc<ServiceLocator> {
        &self.service_locator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_embedded_application_creation() {
        let config = AppConfig::embedded_default();
        let app = EmbeddedApplication::new(config).await;
        assert!(app.is_ok());
    }

    #[tokio::test]
    async fn test_embedded_application_with_temp_db() {
        // 创建临时数据库文件
        let temp_db = NamedTempFile::new().expect("Failed to create temp file");
        let db_path = format!("sqlite:{}", temp_db.path().display());

        let mut config = AppConfig::embedded_default();
        config.database.url = db_path;

        let app = EmbeddedApplication::new(config).await;
        assert!(app.is_ok());
    }
}