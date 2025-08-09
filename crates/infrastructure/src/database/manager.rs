use scheduler_core::{
    errors::SchedulerError,
    traits::{TaskRepository, TaskRunRepository, WorkerRepository},
    SchedulerResult,
};
use super::postgres::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};
use super::sqlite::{SqliteTaskRepository, SqliteTaskRunRepository, SqliteWorkerRepository};

#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseType {
    PostgreSQL,
    SQLite,
}

impl DatabaseType {
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            DatabaseType::PostgreSQL
        } else {
            DatabaseType::SQLite
        }
    }
}

pub enum DatabasePool {
    PostgreSQL(sqlx::PgPool),
    SQLite(sqlx::SqlitePool),
}

impl DatabasePool {
    pub async fn new(url: &str, max_connections: u32) -> SchedulerResult<Self> {
        let db_type = DatabaseType::from_url(url);

        match db_type {
            DatabaseType::PostgreSQL => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(max_connections)
                    .connect(url)
                    .await
                    .map_err(SchedulerError::Database)?;
                Ok(DatabasePool::PostgreSQL(pool))
            }
            DatabaseType::SQLite => {
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(max_connections)
                    .connect(url)
                    .await
                    .map_err(SchedulerError::Database)?;
                Ok(DatabasePool::SQLite(pool))
            }
        }
    }

    pub fn database_type(&self) -> DatabaseType {
        match self {
            DatabasePool::PostgreSQL(_) => DatabaseType::PostgreSQL,
            DatabasePool::SQLite(_) => DatabaseType::SQLite,
        }
    }

    pub async fn health_check(&self) -> SchedulerResult<()> {
        match self {
            DatabasePool::PostgreSQL(pool) => {
                sqlx::query("SELECT 1")
                    .execute(pool)
                    .await
                    .map_err(SchedulerError::Database)?;
            }
            DatabasePool::SQLite(pool) => {
                sqlx::query("SELECT 1")
                    .execute(pool)
                    .await
                    .map_err(SchedulerError::Database)?;
            }
        }
        Ok(())
    }

    pub async fn close(&self) {
        match self {
            DatabasePool::PostgreSQL(pool) => pool.close().await,
            DatabasePool::SQLite(pool) => pool.close().await,
        }
    }
}

pub struct DatabaseManager {
    pool: DatabasePool,
}

impl DatabaseManager {
    pub async fn new(url: &str, max_connections: u32) -> SchedulerResult<Self> {
        let pool = DatabasePool::new(url, max_connections).await?;
        Ok(Self { pool })
    }

    pub fn database_type(&self) -> DatabaseType {
        self.pool.database_type()
    }

    pub async fn health_check(&self) -> SchedulerResult<()> {
        self.pool.health_check().await
    }

    pub async fn close(&self) {
        self.pool.close().await
    }
    pub fn task_repository(&self) -> Box<dyn TaskRepository> {
        match &self.pool {
            DatabasePool::PostgreSQL(pool) => Box::new(PostgresTaskRepository::new(pool.clone())),
            DatabasePool::SQLite(pool) => Box::new(SqliteTaskRepository::new(pool.clone())),
        }
    }
    pub fn task_run_repository(&self) -> Box<dyn TaskRunRepository> {
        match &self.pool {
            DatabasePool::PostgreSQL(pool) => {
                Box::new(PostgresTaskRunRepository::new(pool.clone()))
            }
            DatabasePool::SQLite(pool) => Box::new(SqliteTaskRunRepository::new(pool.clone())),
        }
    }
    pub fn worker_repository(&self) -> Box<dyn WorkerRepository> {
        match &self.pool {
            DatabasePool::PostgreSQL(pool) => Box::new(PostgresWorkerRepository::new(pool.clone())),
            DatabasePool::SQLite(pool) => Box::new(SqliteWorkerRepository::new(pool.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_type_detection() {
        assert_eq!(
            DatabaseType::from_url("postgres://user:pass@localhost/db"),
            DatabaseType::PostgreSQL
        );
        assert_eq!(
            DatabaseType::from_url("postgresql://user:pass@localhost/db"),
            DatabaseType::PostgreSQL
        );
        assert_eq!(
            DatabaseType::from_url("sqlite:test.db"),
            DatabaseType::SQLite
        );
        assert_eq!(
            DatabaseType::from_url("/path/to/database.db"),
            DatabaseType::SQLite
        );
    }

    #[tokio::test]
    async fn test_sqlite_database_manager() {
        let db_manager = DatabaseManager::new("sqlite::memory:", 10).await.unwrap();
        assert_eq!(db_manager.database_type(), DatabaseType::SQLite);
        assert!(db_manager.health_check().await.is_ok());
        let _task_repo = db_manager.task_repository();
        let _task_run_repo = db_manager.task_run_repository();
        let _worker_repo = db_manager.worker_repository();
        db_manager.close().await;
    }

    #[tokio::test]
    async fn test_database_switching() {
        {
            let db_manager = DatabaseManager::new("sqlite::memory:", 5).await.unwrap();
            assert_eq!(db_manager.database_type(), DatabaseType::SQLite);
            let _task_repo = db_manager.task_repository();

            db_manager.close().await;
        }
        {
            let db_manager = DatabaseManager::new("sqlite::memory:", 3).await.unwrap();
            assert_eq!(db_manager.database_type(), DatabaseType::SQLite);
            let _task_repo = db_manager.task_repository();

            db_manager.close().await;
        }
    }

    #[tokio::test]
    async fn test_repository_trait_compatibility() {
        let db_manager = DatabaseManager::new("sqlite::memory:", 10).await.unwrap();
        let task_repo = db_manager.task_repository();
        let task_run_repo = db_manager.task_run_repository();
        let worker_repo = db_manager.worker_repository();
        fn accepts_task_repo(_repo: Box<dyn TaskRepository>) {}
        fn accepts_task_run_repo(_repo: Box<dyn TaskRunRepository>) {}
        fn accepts_worker_repo(_repo: Box<dyn WorkerRepository>) {}

        accepts_task_repo(task_repo);
        accepts_task_run_repo(task_run_repo);
        accepts_worker_repo(worker_repo);

        db_manager.close().await;
    }
    #[tokio::test]
    async fn test_unified_database_interface() {
        async fn test_with_database(db_url: &str) -> SchedulerResult<()> {
            let db_manager = DatabaseManager::new(db_url, 10).await?;
            db_manager.health_check().await?;
            let _task_repo = db_manager.task_repository();
            let _task_run_repo = db_manager.task_run_repository();
            let _worker_repo = db_manager.worker_repository();

            db_manager.close().await;
            Ok(())
        }
        assert!(test_with_database("sqlite::memory:").await.is_ok());
        assert!(test_with_database("sqlite::memory:").await.is_ok());
    }
}
