use anyhow::Result;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, Image};
use testcontainers_modules::postgres::Postgres;
use tokio::time::{sleep, Duration};

/// Database test container setup utility
pub struct DatabaseTestContainer {
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
    pub pool: PgPool,
    pub database_url: String,
}

impl DatabaseTestContainer {
    /// Create a new test database container
    pub async fn new() -> Result<Self> {
        // Start PostgreSQL container
        let postgres_image = Postgres::default()
            .with_db_name("scheduler_test")
            .with_user("test_user")
            .with_password("test_password");

        let container = postgres_image.start().await?;
        let port = container.get_host_port_ipv4(5432).await?;

        let database_url = format!(
            "postgresql://test_user:test_password@localhost:{}/scheduler_test",
            port
        );

        // Wait for database to be ready
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

        Ok(Self {
            container,
            pool,
            database_url,
        })
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        // Run the migration files manually since we can't access the migrations directory directly
        self.create_tasks_table().await?;
        self.create_task_runs_table().await?;
        self.create_workers_table().await?;
        self.create_indexes().await?;
        Ok(())
    }

    /// Create tasks table
    async fn create_tasks_table(&self) -> Result<()> {
        sqlx::query(r#"CREATE TYPE task_status AS ENUM ('ACTIVE', 'INACTIVE')"#)
            .execute(&self.pool)
            .await?;

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
                status task_status NOT NULL DEFAULT 'ACTIVE',
                dependencies BIGINT[] DEFAULT '{}',
                shard_config JSONB,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Create task_runs table
    async fn create_task_runs_table(&self) -> Result<()> {
        sqlx::query(r#"CREATE TYPE task_run_status AS ENUM ('PENDING', 'DISPATCHED', 'RUNNING', 'COMPLETED', 'FAILED', 'TIMEOUT')"#)
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE task_runs (
                id BIGSERIAL PRIMARY KEY,
                task_id BIGINT NOT NULL REFERENCES tasks(id),
                status task_run_status NOT NULL DEFAULT 'PENDING',
                worker_id VARCHAR(255),
                retry_count INTEGER NOT NULL DEFAULT 0,
                shard_index INTEGER,
                shard_total INTEGER,
                scheduled_at TIMESTAMPTZ NOT NULL,
                started_at TIMESTAMPTZ,
                completed_at TIMESTAMPTZ,
                result TEXT,
                error_message TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Create workers table
    async fn create_workers_table(&self) -> Result<()> {
        sqlx::query(r#"CREATE TYPE worker_status AS ENUM ('ALIVE', 'DOWN')"#)
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE workers (
                id VARCHAR(255) PRIMARY KEY,
                hostname VARCHAR(255) NOT NULL,
                ip_address INET NOT NULL,
                supported_task_types TEXT[] NOT NULL,
                max_concurrent_tasks INTEGER NOT NULL,
                current_task_count INTEGER NOT NULL DEFAULT 0,
                status worker_status NOT NULL DEFAULT 'ALIVE',
                last_heartbeat TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Create database indexes
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

    /// Clean all tables for test isolation
    pub async fn clean_tables(&self) -> Result<()> {
        let tables = vec!["task_runs", "tasks", "workers"];

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

    /// Insert test data for tasks
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

    /// Insert test data for task runs
    pub async fn insert_test_task_run(
        &self,
        task_id: i64,
        status: &str,
        worker_id: Option<&str>,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO task_runs (task_id, status, worker_id, scheduled_at)
            VALUES ($1, $2::task_run_status, $3, NOW())
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

    /// Insert test data for workers
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

    /// Get table row count for testing
    pub async fn get_table_count(&self, table_name: &str) -> Result<i64> {
        let row = sqlx::query(&format!("SELECT COUNT(*) as count FROM {}", table_name))
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
    }

    /// Verify database connection and schema
    pub async fn verify_setup(&self) -> Result<bool> {
        // Check if all tables exist
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

        let expected_tables = vec!["tasks", "task_runs", "workers"];
        for expected in &expected_tables {
            if !table_names.contains(&expected.to_string()) {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

/// Test helper to create multiple test containers with different configurations
pub struct MultiDatabaseTestSetup {
    pub containers: HashMap<String, DatabaseTestContainer>,
}

impl MultiDatabaseTestSetup {
    /// Create multiple test database containers
    pub async fn new(container_names: Vec<&str>) -> Result<Self> {
        let mut containers = HashMap::new();

        for name in container_names {
            let container = DatabaseTestContainer::new().await?;
            container.run_migrations().await?;
            containers.insert(name.to_string(), container);
        }

        Ok(Self { containers })
    }

    /// Get a specific test container
    pub fn get_container(&self, name: &str) -> Option<&DatabaseTestContainer> {
        self.containers.get(name)
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

        // Test basic operations
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
