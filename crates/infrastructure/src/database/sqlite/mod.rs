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

pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
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
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
    pub async fn migrate(&self) -> Result<()> {
        Ok(())
    }
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

pub type DbPool = Pool<Sqlite>;
