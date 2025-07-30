pub mod example;
pub mod manager;
pub mod postgres;
pub mod sqlite;

// Re-export the unified manager and types (Interface Segregation principle)
pub use manager::{DatabaseManager, DatabasePool, DatabaseType};

// Re-export repository implementations for direct access if needed (YAGNI principle)
pub use postgres::{PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository};
pub use sqlite::{SqliteTaskRepository, SqliteTaskRunRepository, SqliteWorkerRepository};

// Re-export the example application service
pub use example::ApplicationService;
