use crate::database::{DatabaseManager, DatabaseType};
use scheduler_foundation::SchedulerResult;
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository, WorkerRepository};

pub struct ApplicationService {
    db_manager: DatabaseManager,
}

impl ApplicationService {
    pub async fn new(database_url: &str, max_connections: u32) -> SchedulerResult<Self> {
        let db_manager = DatabaseManager::new(database_url, max_connections).await?;
        Ok(Self { db_manager })
    }
    pub fn database_info(&self) -> DatabaseType {
        self.db_manager.database_type()
    }
    pub async fn health_check(&self) -> SchedulerResult<()> {
        self.db_manager.health_check().await
    }
    pub fn tasks(&self) -> Box<dyn TaskRepository> {
        self.db_manager.task_repository()
    }
    pub fn task_runs(&self) -> Box<dyn TaskRunRepository> {
        self.db_manager.task_run_repository()
    }
    pub fn workers(&self) -> Box<dyn WorkerRepository> {
        self.db_manager.worker_repository()
    }
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
        assert_eq!(app.database_info(), DatabaseType::SQLite);
        assert!(app.health_check().await.is_ok());
        let _tasks = app.tasks();
        let _task_runs = app.task_runs();
        let _workers = app.workers();

        app.close().await;
    }

    #[tokio::test]
    async fn test_application_database_switching() {
        {
            let app = ApplicationService::new("sqlite::memory:", 5).await.unwrap();
            assert_eq!(app.database_info(), DatabaseType::SQLite);
            assert!(app.health_check().await.is_ok());
            app.close().await;
        }
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
        fn use_repositories(
            _tasks: Box<dyn TaskRepository>,
            _task_runs: Box<dyn TaskRunRepository>,
            _workers: Box<dyn WorkerRepository>,
        ) {
        }

        use_repositories(app.tasks(), app.task_runs(), app.workers());

        app.close().await;
    }
}
