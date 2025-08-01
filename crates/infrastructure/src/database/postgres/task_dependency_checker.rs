//! Task dependency checker - Handles task dependency validation
//!
//! This component is responsible only for checking task dependencies
//! and validating if tasks can be executed.

use scheduler_core::{errors::SchedulerError, models::Task, SchedulerResult};
use sqlx::{PgPool, Row};
use tracing::debug;

/// Task dependency checker - Handles task dependency validation
/// Follows SRP: Only responsible for dependency checking
pub struct TaskDependencyChecker {
    pool: PgPool,
}

impl TaskDependencyChecker {
    /// Create a new task dependency checker
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check if all dependencies for a task are satisfied
    pub async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool> {
        // Get the task first
        let task_row = sqlx::query("SELECT dependencies FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        let dependencies: Vec<i64> = match task_row {
            Some(row) => row.try_get("dependencies").unwrap_or_default(),
            None => return Err(SchedulerError::TaskNotFound { id: task_id }),
        };

        if dependencies.is_empty() {
            debug!("Task {} has no dependencies", task_id);
            return Ok(true);
        }

        debug!(
            "Checking {} dependencies for task {}",
            dependencies.len(),
            task_id
        );

        // Check all dependency tasks' recent execution status
        for dep_id in &dependencies {
            if !self.check_dependency_success(*dep_id).await? {
                debug!(
                    "Dependency {} for task {} is not satisfied",
                    dep_id, task_id
                );
                return Ok(false);
            }
        }

        debug!("All dependencies for task {} are satisfied", task_id);
        Ok(true)
    }

    /// Check if a specific dependency task was successfully executed
    async fn check_dependency_success(&self, dep_id: i64) -> SchedulerResult<bool> {
        let recent_run = sqlx::query(
            "SELECT status FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(dep_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match recent_run {
            Some(row) => {
                let status: String = row.try_get("status")?;
                let is_success = status == "COMPLETED";
                debug!(
                    "Dependency {} status: {}, success: {}",
                    dep_id, status, is_success
                );
                Ok(is_success)
            }
            None => {
                debug!("Dependency {} has never been executed", dep_id);
                Ok(false) // Dependency task never executed
            }
        }
    }

    /// Get all dependency tasks for a given task
    pub async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        let task_row = sqlx::query("SELECT dependencies FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        let dependencies: Vec<i64> = match task_row {
            Some(row) => row.try_get("dependencies").unwrap_or_default(),
            None => return Err(SchedulerError::TaskNotFound { id: task_id }),
        };

        if dependencies.is_empty() {
            return Ok(vec![]);
        }

        let mut dependency_tasks = Vec::new();
        for dep_id in &dependencies {
            if let Some(dep_task) = self.get_task_by_id(*dep_id).await? {
                dependency_tasks.push(dep_task);
            }
        }

        Ok(dependency_tasks)
    }

    /// Get task by ID (internal helper)
    async fn get_task_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
        let row = sqlx::query(
            "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_task(&row)?)),
            None => Ok(None),
        }
    }

    /// Convert database row to Task model (internal helper)
    fn row_to_task(row: &sqlx::postgres::PgRow) -> SchedulerResult<Task> {
        let dependencies: Vec<i64> = row
            .try_get::<Vec<i64>, _>("dependencies")
            .unwrap_or_default();

        let shard_config = row
            .try_get::<Option<serde_json::Value>, _>("shard_config")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok());

        Ok(Task {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            task_type: row.try_get("task_type")?,
            schedule: row.try_get("schedule")?,
            parameters: row.try_get("parameters")?,
            timeout_seconds: row.try_get("timeout_seconds")?,
            max_retries: row.try_get("max_retries")?,
            status: row.try_get("status")?,
            dependencies,
            shard_config,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}
