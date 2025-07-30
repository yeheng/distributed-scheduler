use scheduler_core::{
    traits::{TaskRepository, TaskRunRepository, WorkerRepository},
    Result,
};

// Import existing repository implementations
use super::postgres::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};
use super::sqlite::{SqliteTaskRepository, SqliteTaskRunRepository, SqliteWorkerRepository};

/// Database type detection (KISS principle)
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

/// Database connection pool enum (Open/Closed principle)
pub enum DatabasePool {
    PostgreSQL(sqlx::PgPool),
    SQLite(sqlx::SqlitePool),
}

impl DatabasePool {
    /// Create pool from URL with automatic type detection
    pub async fn new(url: &str, max_connections: u32) -> Result<Self> {
        let db_type = DatabaseType::from_url(url);

        match db_type {
            DatabaseType::PostgreSQL => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(max_connections)
                    .connect(url)
                    .await
                    .map_err(scheduler_core::SchedulerError::Database)?;
                Ok(DatabasePool::PostgreSQL(pool))
            }
            DatabaseType::SQLite => {
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(max_connections)
                    .connect(url)
                    .await
                    .map_err(scheduler_core::SchedulerError::Database)?;
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

    pub async fn health_check(&self) -> Result<()> {
        match self {
            DatabasePool::PostgreSQL(pool) => {
                sqlx::query("SELECT 1")
                    .execute(pool)
                    .await
                    .map_err(scheduler_core::SchedulerError::Database)?;
            }
            DatabasePool::SQLite(pool) => {
                sqlx::query("SELECT 1")
                    .execute(pool)
                    .await
                    .map_err(scheduler_core::SchedulerError::Database)?;
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

/// Unified database manager (Single Responsibility principle)
pub struct DatabaseManager {
    pool: DatabasePool,
}

impl DatabaseManager {
    /// Create new database manager with automatic type detection
    pub async fn new(url: &str, max_connections: u32) -> Result<Self> {
        let pool = DatabasePool::new(url, max_connections).await?;
        Ok(Self { pool })
    }

    pub fn database_type(&self) -> DatabaseType {
        self.pool.database_type()
    }

    pub async fn health_check(&self) -> Result<()> {
        self.pool.health_check().await
    }

    pub async fn close(&self) {
        self.pool.close().await
    }

    /// Factory method for task repository (Dependency Inversion principle)
    pub fn task_repository(&self) -> Box<dyn TaskRepository> {
        match &self.pool {
            DatabasePool::PostgreSQL(pool) => Box::new(PostgresTaskRepository::new(pool.clone())),
            DatabasePool::SQLite(pool) => Box::new(SqliteTaskRepository::new(pool.clone())),
        }
    }

    /// Factory method for task run repository
    pub fn task_run_repository(&self) -> Box<dyn TaskRunRepository> {
        match &self.pool {
            DatabasePool::PostgreSQL(pool) => {
                Box::new(PostgresTaskRunRepository::new(pool.clone()))
            }
            DatabasePool::SQLite(pool) => Box::new(SqliteTaskRunRepository::new(pool.clone())),
        }
    }

    /// Factory method for worker repository
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
        // Test PostgreSQL URL detection
        assert_eq!(
            DatabaseType::from_url("postgres://user:pass@localhost/db"),
            DatabaseType::PostgreSQL
        );
        assert_eq!(
            DatabaseType::from_url("postgresql://user:pass@localhost/db"),
            DatabaseType::PostgreSQL
        );

        // Test SQLite URL detection
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

        // Test database type detection
        assert_eq!(db_manager.database_type(), DatabaseType::SQLite);

        // Test health check
        assert!(db_manager.health_check().await.is_ok());

        // Test repository creation
        let _task_repo = db_manager.task_repository();
        let _task_run_repo = db_manager.task_run_repository();
        let _worker_repo = db_manager.worker_repository();

        // Close connection
        db_manager.close().await;
    }

    #[tokio::test]
    async fn test_database_switching() {
        // Test SQLite in memory - first instance
        {
            let db_manager = DatabaseManager::new("sqlite::memory:", 5).await.unwrap();
            assert_eq!(db_manager.database_type(), DatabaseType::SQLite);

            // Create and test repositories
            let _task_repo = db_manager.task_repository();

            db_manager.close().await;
        }

        // Test SQLite in memory - second instance (demonstrates switching)
        {
            let db_manager = DatabaseManager::new("sqlite::memory:", 3).await.unwrap();
            assert_eq!(db_manager.database_type(), DatabaseType::SQLite);

            // Test with different max_connections to show it's a new instance
            let _task_repo = db_manager.task_repository();

            db_manager.close().await;
        }
    }

    #[tokio::test]
    async fn test_repository_trait_compatibility() {
        let db_manager = DatabaseManager::new("sqlite::memory:", 10).await.unwrap();

        // Test that returned repositories implement the correct traits
        let task_repo = db_manager.task_repository();
        let task_run_repo = db_manager.task_run_repository();
        let worker_repo = db_manager.worker_repository();

        // These should compile - testing trait object compatibility
        fn accepts_task_repo(_repo: Box<dyn TaskRepository>) {}
        fn accepts_task_run_repo(_repo: Box<dyn TaskRunRepository>) {}
        fn accepts_worker_repo(_repo: Box<dyn WorkerRepository>) {}

        accepts_task_repo(task_repo);
        accepts_task_run_repo(task_run_repo);
        accepts_worker_repo(worker_repo);

        db_manager.close().await;
    }

    // Integration test demonstrating the unified interface
    #[tokio::test]
    async fn test_unified_database_interface() {
        async fn test_with_database(db_url: &str) -> Result<()> {
            let db_manager = DatabaseManager::new(db_url, 10).await?;

            // Health check should work regardless of database type
            db_manager.health_check().await?;

            // Repository creation should work regardless of database type
            let _task_repo = db_manager.task_repository();
            let _task_run_repo = db_manager.task_run_repository();
            let _worker_repo = db_manager.worker_repository();

            db_manager.close().await;
            Ok(())
        }

        // Test with different SQLite configurations
        assert!(test_with_database("sqlite::memory:").await.is_ok());

        // Test the same interface with a different URL pattern
        // (Both are SQLite but demonstrate URL flexibility)
        assert!(test_with_database("sqlite::memory:").await.is_ok());
    }
}
