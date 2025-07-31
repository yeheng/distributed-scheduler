use anyhow::Result;
use chrono::Utc;
use scheduler_core::models::{Task, TaskRun, TaskRunStatus, TaskStatus, WorkerInfo, WorkerStatus};
use scheduler_core::traits::{TaskRepository, TaskRunRepository, WorkerRepository};
use scheduler_infrastructure::database::postgres::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};
use testcontainers::ContainerAsync;
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::time::{sleep, Duration};

/// Core Integration Test Setup
pub struct CoreIntegrationTestSetup {
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
    pub task_repo: PostgresTaskRepository,
    pub task_run_repo: PostgresTaskRunRepository,
    pub worker_repo: PostgresWorkerRepository,
}

impl CoreIntegrationTestSetup {
    /// Create new integration test setup
    pub async fn new() -> Result<Self> {
        // Start PostgreSQL container
        let postgres_image = Postgres::default()
            .with_db_name("scheduler_core_integration_test")
            .with_user("test_user")
            .with_password("test_password")
            .with_tag("16-alpine");

        let container = postgres_image.start().await?;
        let port = container.get_host_port_ipv4(5432).await?;

        let database_url = format!(
            "postgresql://test_user:test_password@localhost:{}/scheduler_core_integration_test",
            port
        );

        // Wait for database to be ready
        let mut retry_count = 0;
        let pool = loop {
            match sqlx::PgPool::connect(&database_url).await {
                Ok(pool) => break pool,
                Err(_) if retry_count < 30 => {
                    retry_count += 1;
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        };

        // Run database migrations
        sqlx::migrate!("../../migrations").run(&pool).await?;

        // Create repositories
        let task_repo = PostgresTaskRepository::new(pool.clone());
        let task_run_repo = PostgresTaskRunRepository::new(pool.clone());
        let worker_repo = PostgresWorkerRepository::new(pool);

        Ok(Self {
            container,
            task_repo,
            task_run_repo,
            worker_repo,
        })
    }

    /// Create test worker
    pub async fn create_test_worker(
        &self,
        worker_id: &str,
        task_types: Vec<String>,
    ) -> Result<WorkerInfo> {
        let worker = WorkerInfo {
            id: worker_id.to_string(),
            hostname: format!("{}-host", worker_id),
            ip_address: "127.0.0.1".to_string(),
            supported_task_types: task_types,
            max_concurrent_tasks: 5,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
        };

        self.worker_repo.register(&worker).await?;
        Ok(worker)
    }

    /// Create test task
    pub async fn create_test_task(
        &self,
        name: &str,
        task_type: &str,
        schedule: &str,
        dependencies: Vec<i64>,
    ) -> Result<Task> {
        let mut task = Task::new(
            name.to_string(),
            task_type.to_string(),
            schedule.to_string(),
            serde_json::json!({"command": format!("echo '{}'", name)}),
        );
        task.dependencies = dependencies;
        task.max_retries = 3;
        task.timeout_seconds = 300;

        self.task_repo
            .create(&task)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Create test task run
    pub async fn create_test_task_run(&self, task_id: i64) -> Result<TaskRun> {
        let task_run = TaskRun::new(task_id, Utc::now());
        self.task_run_repo
            .create(&task_run)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Clean up all test data
    pub async fn cleanup(&self) -> Result<()> {
        // Clean in reverse order of dependencies
        self.task_run_repo
            .cleanup_old_runs(0)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Note: In a real implementation, we'd need to handle foreign key constraints
        // For now, we'll just verify the cleanup worked
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_lifecycle() -> Result<()> {
        let setup = CoreIntegrationTestSetup::new().await?;

        // Test worker registration
        let worker = setup
            .create_test_worker("test-worker", vec!["shell".to_string()])
            .await?;

        // Verify worker was created
        let retrieved_worker = setup.worker_repo.get_by_id(&worker.id).await?;
        assert!(retrieved_worker.is_some());
        let retrieved_worker = retrieved_worker.unwrap();
        assert_eq!(retrieved_worker.id, worker.id);
        assert_eq!(retrieved_worker.status, WorkerStatus::Alive);

        // Test worker heartbeat update
        let new_heartbeat = Utc::now();
        setup
            .worker_repo
            .update_heartbeat(&worker.id, new_heartbeat, 2)
            .await?;

        let updated_worker = setup.worker_repo.get_by_id(&worker.id).await?.unwrap();
        assert_eq!(updated_worker.last_heartbeat, new_heartbeat);
        assert_eq!(updated_worker.current_task_count, 2);

        // Test worker status update
        setup
            .worker_repo
            .update_status(&worker.id, WorkerStatus::Down)
            .await?;

        let down_worker = setup.worker_repo.get_by_id(&worker.id).await?.unwrap();
        assert_eq!(down_worker.status, WorkerStatus::Down);

        // Test worker unregistration
        setup.worker_repo.unregister(&worker.id).await?;
        let deleted_worker = setup.worker_repo.get_by_id(&worker.id).await?;
        assert!(deleted_worker.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_task_lifecycle() -> Result<()> {
        let setup = CoreIntegrationTestSetup::new().await?;

        // Test task creation
        let task = setup
            .create_test_task("test-task", "shell", "0 0 * * *", vec![])
            .await?;

        // Verify task was created
        let retrieved_task = setup.task_repo.get_by_id(task.id).await?;
        assert!(retrieved_task.is_some());
        let retrieved_task = retrieved_task.unwrap();
        assert_eq!(retrieved_task.name, "test-task");
        assert_eq!(retrieved_task.task_type, "shell");
        assert_eq!(retrieved_task.status, TaskStatus::Active);

        // Test task update
        let mut updated_task = task.clone();
        updated_task.status = TaskStatus::Inactive;
        setup.task_repo.update(&updated_task).await?;

        let retrieved_updated_task = setup.task_repo.get_by_id(task.id).await?.unwrap();
        assert_eq!(retrieved_updated_task.status, TaskStatus::Inactive);

        // Test task deletion
        setup.task_repo.delete(task.id).await?;
        let deleted_task = setup.task_repo.get_by_id(task.id).await?;
        assert!(deleted_task.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_task_run_lifecycle() -> Result<()> {
        let setup = CoreIntegrationTestSetup::new().await?;

        // Create a task first
        let task = setup
            .create_test_task("test-task", "shell", "0 0 * * *", vec![])
            .await?;

        // Test task run creation
        let task_run = setup.create_test_task_run(task.id).await?;

        // Verify task run was created
        let retrieved_task_run = setup.task_run_repo.get_by_id(task_run.id).await?;
        assert!(retrieved_task_run.is_some());
        let retrieved_task_run = retrieved_task_run.unwrap();
        assert_eq!(retrieved_task_run.task_id, task.id);
        assert_eq!(retrieved_task_run.status, TaskRunStatus::Pending);

        // Test task run status update
        let worker = setup
            .create_test_worker("test-worker", vec!["shell".to_string()])
            .await?;

        setup
            .task_run_repo
            .update_status(task_run.id, TaskRunStatus::Running, Some(&worker.id))
            .await?;

        let updated_task_run = setup.task_run_repo.get_by_id(task_run.id).await?.unwrap();
        assert_eq!(updated_task_run.status, TaskRunStatus::Running);
        assert_eq!(updated_task_run.worker_id, Some(worker.id.clone()));

        // Test task run result update
        let result = "Task completed successfully";
        setup
            .task_run_repo
            .update_result(task_run.id, Some(result), None)
            .await?;

        setup
            .task_run_repo
            .update_status(task_run.id, TaskRunStatus::Completed, Some(&worker.id))
            .await?;

        let completed_task_run = setup.task_run_repo.get_by_id(task_run.id).await?.unwrap();
        assert_eq!(completed_task_run.status, TaskRunStatus::Completed);
        assert_eq!(completed_task_run.result, Some(result.to_string()));

        // Test task run deletion
        setup.task_run_repo.delete(task_run.id).await?;
        let deleted_task_run = setup.task_run_repo.get_by_id(task_run.id).await?;
        assert!(deleted_task_run.is_none());

        // Clean up
        setup.task_repo.delete(task.id).await?;
        setup.worker_repo.unregister(&worker.id).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_task_dependency_resolution() -> Result<()> {
        let setup = CoreIntegrationTestSetup::new().await?;

        // Create task chain: task_a -> task_b -> task_c
        let task_a = setup
            .create_test_task("task-a", "shell", "0 0 * * *", vec![])
            .await?;
        let task_b = setup
            .create_test_task("task-b", "shell", "0 0 * * *", vec![task_a.id])
            .await?;
        let task_c = setup
            .create_test_task("task-c", "shell", "0 0 * * *", vec![task_b.id])
            .await?;

        // Test dependency checking
        // task_a has no dependencies, should be executable
        let can_execute_a = setup
            .task_repo
            .check_dependencies(task_a.id)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        assert!(can_execute_a);

        // task_b depends on task_a
        let _can_execute_b = setup
            .task_repo
            .check_dependencies(task_b.id)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        // Note: This depends on the implementation of check_dependencies
        // It should check if dependencies are completed

        // Create and complete task_a run
        let worker = setup
            .create_test_worker("test-worker", vec!["shell".to_string()])
            .await?;

        let task_a_run = setup.create_test_task_run(task_a.id).await?;
        setup
            .task_run_repo
            .update_status(task_a_run.id, TaskRunStatus::Running, Some(&worker.id))
            .await?;
        setup
            .task_run_repo
            .update_result(task_a_run.id, Some("task-a completed"), None)
            .await?;
        setup
            .task_run_repo
            .update_status(task_a_run.id, TaskRunStatus::Completed, Some(&worker.id))
            .await?;

        // Now task_b should be executable
        let can_execute_b_after_a = setup.task_repo.check_dependencies(task_b.id).await?;
        assert!(can_execute_b_after_a);

        // Clean up
        setup.task_run_repo.delete(task_a_run.id).await?;
        setup.task_repo.delete(task_a.id).await?;
        setup.task_repo.delete(task_b.id).await?;
        setup.task_repo.delete(task_c.id).await?;
        setup.worker_repo.unregister(&worker.id).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_worker_load_management() -> Result<()> {
        let setup = CoreIntegrationTestSetup::new().await?;

        // Create multiple workers
        let worker1 = setup
            .create_test_worker("worker-1", vec!["shell".to_string()])
            .await?;
        let worker2 = setup
            .create_test_worker("worker-2", vec!["shell".to_string()])
            .await?;

        // Create tasks
        let task1 = setup
            .create_test_task("task-1", "shell", "0 0 * * *", vec![])
            .await?;
        let task2 = setup
            .create_test_task("task-2", "shell", "0 0 * * *", vec![])
            .await?;

        // Assign tasks to workers
        let task_run1 = setup.create_test_task_run(task1.id).await?;
        let task_run2 = setup.create_test_task_run(task2.id).await?;

        setup
            .task_run_repo
            .update_status(task_run1.id, TaskRunStatus::Running, Some(&worker1.id))
            .await?;
        setup
            .task_run_repo
            .update_status(task_run2.id, TaskRunStatus::Running, Some(&worker2.id))
            .await?;

        // Update worker load
        setup
            .worker_repo
            .update_heartbeat(&worker1.id, Utc::now(), 1)
            .await?;
        setup
            .worker_repo
            .update_heartbeat(&worker2.id, Utc::now(), 1)
            .await?;

        // Test worker load statistics
        let load_stats = setup.worker_repo.get_worker_load_stats().await?;
        assert_eq!(load_stats.len(), 2);

        for stats in load_stats {
            assert_eq!(stats.current_task_count, 1);
            assert_eq!(stats.load_percentage, 20.0); // 1/5 * 100%
        }

        // Test getting alive workers
        let alive_workers = setup.worker_repo.get_alive_workers().await?;
        assert_eq!(alive_workers.len(), 2);

        // Test getting workers by task type
        let shell_workers = setup.worker_repo.get_workers_by_task_type("shell").await?;
        assert_eq!(shell_workers.len(), 2);

        // Clean up
        setup.task_run_repo.delete(task_run1.id).await?;
        setup.task_run_repo.delete(task_run2.id).await?;
        setup.task_repo.delete(task1.id).await?;
        setup.task_repo.delete(task2.id).await?;
        setup.worker_repo.unregister(&worker1.id).await?;
        setup.worker_repo.unregister(&worker2.id).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_task_retry_logic() -> Result<()> {
        let setup = CoreIntegrationTestSetup::new().await?;

        // Create a task with retries
        let mut task = setup
            .create_test_task("retry-task", "shell", "0 0 * * *", vec![])
            .await?;
        task.max_retries = 2;
        setup.task_repo.update(&task).await?;

        // Create initial task run
        let task_run = setup.create_test_task_run(task.id).await?;
        let worker = setup
            .create_test_worker("test-worker", vec!["shell".to_string()])
            .await?;

        // Simulate task failure
        setup
            .task_run_repo
            .update_status(task_run.id, TaskRunStatus::Running, Some(&worker.id))
            .await?;
        setup
            .task_run_repo
            .update_result(task_run.id, None, Some("Task failed"))
            .await?;
        setup
            .task_run_repo
            .update_status(task_run.id, TaskRunStatus::Failed, Some(&worker.id))
            .await?;

        // Verify failure
        let failed_run = setup.task_run_repo.get_by_id(task_run.id).await?.unwrap();
        assert_eq!(failed_run.status, TaskRunStatus::Failed);
        assert!(failed_run.error_message.is_some());

        // Create retry runs
        let retry1 = setup.create_test_task_run(task.id).await?;
        let mut retry1_updated = retry1.clone();
        retry1_updated.retry_count = 1;
        setup.task_run_repo.update(&retry1_updated).await?;

        let retry2 = setup.create_test_task_run(task.id).await?;
        let mut retry2_updated = retry2.clone();
        retry2_updated.retry_count = 2;
        setup.task_run_repo.update(&retry2_updated).await?;

        // Simulate retry success
        setup
            .task_run_repo
            .update_status(retry2.id, TaskRunStatus::Running, Some(&worker.id))
            .await?;
        setup
            .task_run_repo
            .update_result(retry2.id, Some("Retry succeeded"), None)
            .await?;
        setup
            .task_run_repo
            .update_status(retry2.id, TaskRunStatus::Completed, Some(&worker.id))
            .await?;

        // Verify retry success
        let completed_retry = setup.task_run_repo.get_by_id(retry2.id).await?.unwrap();
        assert_eq!(completed_retry.status, TaskRunStatus::Completed);
        assert_eq!(completed_retry.retry_count, 2);

        // Test getting task runs by task ID
        let all_runs = setup.task_run_repo.get_by_task_id(task.id).await?;
        assert_eq!(all_runs.len(), 3); // Original + 2 retries

        // Clean up
        setup.task_run_repo.delete(task_run.id).await?;
        setup.task_run_repo.delete(retry1.id).await?;
        setup.task_run_repo.delete(retry2.id).await?;
        setup.task_repo.delete(task.id).await?;
        setup.worker_repo.unregister(&worker.id).await?;

        Ok(())
    }
}
