pub mod postgres_task_repository;
pub mod postgres_task_run_repository;
pub mod postgres_user_repository;
pub mod postgres_worker_repository;
pub mod task_dependency_checker;

pub use postgres_task_repository::*;
pub use postgres_task_run_repository::*;
pub use postgres_user_repository::*;
pub use postgres_worker_repository::*;

use anyhow::Result;
use scheduler_config::models::DatabaseConfig;
use sqlx::{PgPool, Pool, Postgres};
use std::time::Duration;

pub struct DatabaseManager {
    pool: PgPool,
}

impl DatabaseManager {
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connection_timeout_seconds))
            .idle_timeout(Duration::from_secs(config.idle_timeout_seconds))
            .max_lifetime(Duration::from_secs(1800)) // 30分钟默认生命周期
            .connect(&config.url)
            .await?;

        Ok(Self { pool })
    }
    pub fn pool(&self) -> &PgPool {
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

pub type DbPool = Pool<Postgres>;
