//! Test container utilities for integration testing
//!
//! This module provides containerized test environments for databases
//! and other external services needed for integration testing.

use anyhow::Result;
use sqlx::{PgPool, Row};
use testcontainers::ContainerAsync;
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::time::{sleep, Duration};

/// PostgreSQL test container with pre-configured schema and data
///
/// This container automatically sets up the required database schema
/// for the scheduler system and provides utilities for test data management.
pub struct DatabaseTestContainer {
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
    pub pool: PgPool,
}

impl DatabaseTestContainer {
    /// Create a new PostgreSQL test container
    ///
    /// This will:
    /// 1. Start a PostgreSQL container
    /// 2. Wait for it to be ready
    /// 3. Create a connection pool
    ///
    /// Note: You must call `run_migrations()` after creation to set up the schema.
    pub async fn new() -> Result<Self> {
        let postgres_image = Postgres::default()
            .with_db_name("scheduler_test")
            .with_user("test_user")
            .with_password("test_password")
            .with_tag("16-alpine");

        let container = postgres_image.start().await?;
        let port = container.get_host_port_ipv4(5432).await?;

        let database_url = format!(
            "postgresql://test_user:test_password@localhost:{}/scheduler_test",
            port
        );

        // Retry connection with backoff
        let mut retry_count = 0;
        let pool = loop {
            match PgPool::connect(&database_url).await {
                Ok(pool) => break pool,
                Err(_) if retry_count < 30 => {
                    retry_count += 1;
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        };

        Ok(Self { container, pool })
    }

    /// Run database migrations to set up the complete schema
    pub async fn run_migrations(&self) -> Result<()> {
        self.create_tasks_table().await?;
        self.create_task_runs_table().await?;
        self.create_workers_table().await?;
        self.create_task_dependencies_table().await?;
        self.create_task_locks_table().await?;
        self.create_indexes().await?;
        Ok(())
    }

    /// Clean all tables (useful for test isolation)
    pub async fn clean_tables(&self) -> Result<()> {
        let tables = vec![
            "task_runs",
            "tasks",
            "workers",
            "task_dependencies",
            "task_locks",
        ];

        for table in tables {
            sqlx::query(&format!(
                "TRUNCATE TABLE {} RESTART IDENTITY CASCADE",
                table
            ))
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Insert a test task and return its ID
    pub async fn insert_test_task(
        &self,
        name: &str,
        task_type: &str,
        schedule: &str,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO tasks (name, task_type, schedule, parameters, timeout_seconds, max_retries)
            VALUES ($1, $2, $3, '{}', 300, 3)
            RETURNING id
        "#,
        )
        .bind(name)
        .bind(task_type)
        .bind(schedule)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    /// Insert a test task run and return its ID
    pub async fn insert_test_task_run(
        &self,
        task_id: i64,
        status: &str,
        worker_id: Option<&str>,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO task_runs (task_id, status, worker_id, scheduled_at)
            VALUES ($1, $2, $3, NOW())
            RETURNING id
        "#,
        )
        .bind(task_id)
        .bind(status)
        .bind(worker_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    /// Insert a test worker
    pub async fn insert_test_worker(
        &self,
        worker_id: &str,
        hostname: &str,
        ip: &str,
        task_types: Vec<String>,
    ) -> Result<()> {
        sqlx::query(r#"
            INSERT INTO workers (id, hostname, ip_address, supported_task_types, max_concurrent_tasks)
            VALUES ($1, $2, $3::inet, $4, 5)
        "#)
        .bind(worker_id)
        .bind(hostname)
        .bind(ip)
        .bind(&task_types)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get count of records in a table
    pub async fn get_table_count(&self, table_name: &str) -> Result<i64> {
        let row = sqlx::query(&format!("SELECT COUNT(*) as count FROM {}", table_name))
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
    }

    /// Verify that the database setup is complete
    pub async fn verify_setup(&self) -> Result<bool> {
        let tables_query = r#"
            SELECT table_name 
            FROM information_schema.tables 
            WHERE table_schema = 'public' 
            AND table_type = 'BASE TABLE'
        "#;

        let rows = sqlx::query(tables_query).fetch_all(&self.pool).await?;
        let table_names: Vec<String> = rows
            .iter()
            .map(|row| row.get::<String, _>("table_name"))
            .collect();

        let expected_tables = vec![
            "tasks",
            "task_runs",
            "workers",
            "task_dependencies",
            "task_locks",
        ];
        for expected in &expected_tables {
            if !table_names.contains(&expected.to_string()) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    // Private helper methods for schema creation
    async fn create_tasks_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE tasks (
                id BIGSERIAL PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                task_type VARCHAR(50) NOT NULL,
                schedule VARCHAR(100) NOT NULL,
                parameters JSONB NOT NULL DEFAULT '{}',
                timeout_seconds INTEGER NOT NULL DEFAULT 300,
                max_retries INTEGER NOT NULL DEFAULT 0,
                status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
                dependencies BIGINT[] DEFAULT '{}',
                shard_config JSONB,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                
                CONSTRAINT tasks_status_check CHECK (status IN ('ACTIVE', 'INACTIVE')),
                CONSTRAINT tasks_timeout_positive CHECK (timeout_seconds > 0),
                CONSTRAINT tasks_max_retries_non_negative CHECK (max_retries >= 0)
            )
        "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_task_runs_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE task_runs (
                id BIGSERIAL PRIMARY KEY,
                task_id BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
                worker_id VARCHAR(255),
                retry_count INTEGER NOT NULL DEFAULT 0,
                shard_index INTEGER,
                shard_total INTEGER,
                scheduled_at TIMESTAMPTZ NOT NULL,
                started_at TIMESTAMPTZ,
                completed_at TIMESTAMPTZ,
                result TEXT,
                error_message TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                
                CONSTRAINT task_runs_status_check CHECK (status IN ('PENDING', 'DISPATCHED', 'RUNNING', 'COMPLETED', 'FAILED', 'TIMEOUT')),
                CONSTRAINT task_runs_retry_count_non_negative CHECK (retry_count >= 0),
                CONSTRAINT task_runs_shard_valid CHECK (
                    (shard_index IS NULL AND shard_total IS NULL) OR 
                    (shard_index IS NOT NULL AND shard_total IS NOT NULL AND shard_index >= 0 AND shard_total > 0 AND shard_index < shard_total)
                ),
                CONSTRAINT task_runs_timing_check CHECK (
                    (started_at IS NULL OR started_at >= scheduled_at) AND
                    (completed_at IS NULL OR (started_at IS NOT NULL AND completed_at >= started_at))
                )
            )
        "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_workers_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE workers (
                id VARCHAR(255) PRIMARY KEY,
                hostname VARCHAR(255) NOT NULL,
                ip_address INET NOT NULL,
                supported_task_types TEXT[] NOT NULL,
                max_concurrent_tasks INTEGER NOT NULL,
                current_task_count INTEGER NOT NULL DEFAULT 0,
                status VARCHAR(20) NOT NULL DEFAULT 'ALIVE',
                last_heartbeat TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                
                CONSTRAINT workers_status_check CHECK (status IN ('ALIVE', 'DOWN')),
                CONSTRAINT workers_max_concurrent_positive CHECK (max_concurrent_tasks > 0)
            )
        "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_task_dependencies_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE task_dependencies (
                id BIGSERIAL PRIMARY KEY,
                task_id BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                depends_on_task_id BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                
                CONSTRAINT task_dependencies_no_self_reference CHECK (task_id != depends_on_task_id),
                UNIQUE(task_id, depends_on_task_id)
            )
        "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_task_locks_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE task_locks (
                task_id BIGINT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
                locked_by VARCHAR(255) NOT NULL,
                locked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ NOT NULL
            )
        "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_indexes(&self) -> Result<()> {
        let indexes = vec![
            "CREATE INDEX idx_tasks_schedule_status ON tasks(schedule, status) WHERE status = 'ACTIVE'",
            "CREATE INDEX idx_task_runs_status_worker ON task_runs(status, worker_id)",
            "CREATE INDEX idx_task_runs_task_id_status ON task_runs(task_id, status)",
            "CREATE INDEX idx_workers_status_heartbeat ON workers(status, last_heartbeat)",
        ];

        for index in indexes {
            sqlx::query(index).execute(&self.pool).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_container_setup() {
        let container = DatabaseTestContainer::new()
            .await
            .expect("Failed to create container");

        container
            .run_migrations()
            .await
            .expect("Failed to run migrations");

        assert!(container
            .verify_setup()
            .await
            .expect("Failed to verify setup"));

        let task_id = container
            .insert_test_task("test_task", "shell", "0 0 * * *")
            .await
            .expect("Failed to insert test task");

        assert!(task_id > 0);

        let count = container
            .get_table_count("tasks")
            .await
            .expect("Failed to get table count");

        assert_eq!(count, 1);

        container
            .clean_tables()
            .await
            .expect("Failed to clean tables");

        let count_after_clean = container
            .get_table_count("tasks")
            .await
            .expect("Failed to get table count after clean");

        assert_eq!(count_after_clean, 0);
    }
}
