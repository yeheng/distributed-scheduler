use crate::database::{DatabaseManager, DatabaseType};
use scheduler_core::SchedulerResult;
use scheduler_core::traits::{TaskRepository, TaskRunRepository, WorkerRepository};

/// Example application service that works with any database backend
/// This demonstrates how the unified database layer follows SOLID principles
pub struct ApplicationService {
    db_manager: DatabaseManager,
}

impl ApplicationService {
    /// Create new application service with database URL
    /// The database type is automatically detected from the URL
    pub async fn new(database_url: &str, max_connections: u32) -> SchedulerResult<Self> {
        let db_manager = DatabaseManager::new(database_url, max_connections).await?;
        Ok(Self { db_manager })
    }

    /// Get information about the database backend being used
    pub fn database_info(&self) -> DatabaseType {
        self.db_manager.database_type()
    }

    /// Check database health
    pub async fn health_check(&self) -> SchedulerResult<()> {
        self.db_manager.health_check().await
    }

    /// Get task repository - works with any database backend
    pub fn tasks(&self) -> Box<dyn TaskRepository> {
        self.db_manager.task_repository()
    }

    /// Get task run repository - works with any database backend
    pub fn task_runs(&self) -> Box<dyn TaskRunRepository> {
        self.db_manager.task_run_repository()
    }

    /// Get worker repository - works with any database backend
    pub fn workers(&self) -> Box<dyn WorkerRepository> {
        self.db_manager.worker_repository()
    }

    /// Close database connections
    pub async fn close(self) {
        self.db_manager.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_application_with_sqlite() {
        let app = ApplicationService::new("sqlite::memory:", 10)
            .await
            .unwrap();

        // Verify we're using SQLite
        assert_eq!(app.database_info(), DatabaseType::SQLite);

        // Test health check
        assert!(app.health_check().await.is_ok());

        // Test repository access
        let _tasks = app.tasks();
        let _task_runs = app.task_runs();
        let _workers = app.workers();

        app.close().await;
    }

    #[tokio::test]
    async fn test_application_database_switching() {
        // Same application code works with different database backends

        // Test with SQLite
        {
            let app = ApplicationService::new("sqlite::memory:", 5).await.unwrap();
            assert_eq!(app.database_info(), DatabaseType::SQLite);
            assert!(app.health_check().await.is_ok());
            app.close().await;
        }

        // Application code doesn't need to change for different databases
        // (In real usage, you might switch from PostgreSQL to SQLite or vice versa)
        {
            let app = ApplicationService::new("sqlite::memory:", 5).await.unwrap();
            assert_eq!(app.database_info(), DatabaseType::SQLite);
            assert!(app.health_check().await.is_ok());
            app.close().await;
        }
    }

    #[tokio::test]
    async fn test_repository_polymorphism() {
        let app = ApplicationService::new("sqlite::memory:", 10)
            .await
            .unwrap();

        // Demonstrate that repositories are polymorphic
        // The same code works regardless of the underlying database
        fn use_repositories(
            _tasks: Box<dyn TaskRepository>,
            _task_runs: Box<dyn TaskRunRepository>,
            _workers: Box<dyn WorkerRepository>,
        ) {
            // Application logic here doesn't need to know about specific database implementation
            // This follows the Dependency Inversion Principle
        }

        use_repositories(app.tasks(), app.task_runs(), app.workers());

        app.close().await;
    }
}
