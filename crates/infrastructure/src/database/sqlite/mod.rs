pub mod sqlite_task_repository;
pub mod sqlite_task_run_repository;
pub mod sqlite_worker_repository;

pub use sqlite_task_repository::SqliteTaskRepository;
pub use sqlite_task_run_repository::SqliteTaskRunRepository;
pub use sqlite_worker_repository::SqliteWorkerRepository;

use anyhow::Result;
use scheduler_core::config::models::DatabaseConfig;
use sqlx::{Pool, Sqlite, SqlitePool};
use std::time::Duration;

/// 数据库连接池管理器
pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    /// 创建新的数据库管理器
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connection_timeout_seconds))
            .idle_timeout(Duration::from_secs(config.idle_timeout_seconds))
            .max_lifetime(Duration::from_secs(1800)) // 30分钟默认生命周期
            .connect(&config.url)
            .await?;

        Ok(Self { pool })
    }

    /// 获取数据库连接池
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// 运行数据库迁移
    pub async fn migrate(&self) -> Result<()> {
        // sqlx::migrate!("../migrations").run(&self.pool).await?;
        Ok(())
    }

    /// 检查数据库连接健康状态
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// 关闭数据库连接池
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// 数据库连接池类型别名
pub type DbPool = Pool<Sqlite>;
