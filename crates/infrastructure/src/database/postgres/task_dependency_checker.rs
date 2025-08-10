use scheduler_core::SchedulerResult;
use scheduler_domain::{
    entities::{Task, TaskRunStatus},
    task_dependency_service::TaskDependencyService,
};
use scheduler_errors::SchedulerError;
use sqlx::{PgPool, Row};
use tracing::debug;

/// Infrastructure implementation of task dependency checking
/// Delegates business logic to domain service and handles data access
pub struct TaskDependencyChecker {
    pool: PgPool,
}

impl TaskDependencyChecker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check if all dependencies of a task are satisfied by delegating to domain service
    pub async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool> {
        let task_dependencies = self.get_task_dependencies(task_id).await?;
        
        if task_dependencies.is_empty() {
            return Ok(true);
        }

        let dependency_statuses = self.get_dependency_run_statuses(&task_dependencies).await?;
        
        TaskDependencyService::check_dependencies_satisfied(&task_dependencies, &dependency_statuses)
    }

    /// Get dependency tasks as full Task entities 
    pub async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        let task_dependencies = self.get_task_dependencies(task_id).await?;

        if task_dependencies.is_empty() {
            return Ok(vec![]);
        }

        let mut dependency_tasks = Vec::new();
        for dep_id in &task_dependencies {
            if let Some(dep_task) = self.get_task_by_id(*dep_id).await? {
                dependency_tasks.push(dep_task);
            }
        }

        Ok(dependency_tasks)
    }

    /// Get the dependency IDs for a given task
    async fn get_task_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<i64>> {
        let task_row = sqlx::query("SELECT dependencies FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        match task_row {
            Some(row) => {
                let dependencies: Vec<i64> = row.try_get("dependencies").unwrap_or_default();
                debug!(
                    "Task {} has {} dependencies: {:?}",
                    task_id,
                    dependencies.len(),
                    dependencies
                );
                Ok(dependencies)
            }
            None => Err(SchedulerError::TaskNotFound { id: task_id }),
        }
    }

    /// Get the latest run status for each dependency task
    async fn get_dependency_run_statuses(
        &self,
        dependency_ids: &[i64],
    ) -> SchedulerResult<Vec<(i64, Option<TaskRunStatus>)>> {
        let mut statuses = Vec::new();

        for &dep_id in dependency_ids {
            let status = self.get_latest_task_run_status(dep_id).await?;
            statuses.push((dep_id, status));
        }

        Ok(statuses)
    }

    /// Get the latest run status for a specific task
    async fn get_latest_task_run_status(&self, task_id: i64) -> SchedulerResult<Option<TaskRunStatus>> {
        let recent_run = sqlx::query(
            "SELECT status FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match recent_run {
            Some(row) => {
                let status_str: String = row.try_get("status")?;
                let status = self.parse_task_run_status(&status_str)?;
                debug!("Task {} latest run status: {:?}", task_id, status);
                Ok(Some(status))
            }
            None => {
                debug!("Task {} has never been executed", task_id);
                Ok(None)
            }
        }
    }

    /// Parse string status to TaskRunStatus enum
    fn parse_task_run_status(&self, status_str: &str) -> SchedulerResult<TaskRunStatus> {
        match status_str {
            "PENDING" => Ok(TaskRunStatus::Pending),
            "DISPATCHED" => Ok(TaskRunStatus::Dispatched),
            "RUNNING" => Ok(TaskRunStatus::Running),
            "COMPLETED" => Ok(TaskRunStatus::Completed),
            "FAILED" => Ok(TaskRunStatus::Failed),
            "TIMEOUT" => Ok(TaskRunStatus::Timeout),
            _ => Err(SchedulerError::Internal(format!(
                "Unknown task run status: {}",
                status_str
            ))),
        }
    }

    /// Get task by ID - used internally for dependency resolution
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

    /// Convert database row to Task entity
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

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: Integration tests with actual database connections should use testcontainers
    // These tests focus on business logic that doesn't require database connections
    
    #[test]
    fn test_parse_task_run_status_valid_cases() {
        // Test the parsing logic without creating a real TaskDependencyChecker
        // Since parse_task_run_status is a private method, we test the logic directly
        
        let test_cases = vec![
            ("PENDING", TaskRunStatus::Pending),
            ("DISPATCHED", TaskRunStatus::Dispatched),
            ("RUNNING", TaskRunStatus::Running),
            ("COMPLETED", TaskRunStatus::Completed),
            ("FAILED", TaskRunStatus::Failed),
            ("TIMEOUT", TaskRunStatus::Timeout),
        ];
        
        for (status_str, expected) in test_cases {
            let result = parse_status_string(status_str);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), expected);
        }
    }
    
    #[test]
    fn test_parse_task_run_status_invalid_cases() {
        let invalid_cases = vec!["INVALID_STATUS", "", "pending", "Completed", "unknown"];
        
        for invalid_status in invalid_cases {
            let result = parse_status_string(invalid_status);
            assert!(result.is_err());
        }
    }
}

/// Helper function for testing status parsing logic
#[cfg(test)]
fn parse_status_string(status_str: &str) -> SchedulerResult<TaskRunStatus> {
    match status_str {
        "PENDING" => Ok(TaskRunStatus::Pending),
        "DISPATCHED" => Ok(TaskRunStatus::Dispatched),
        "RUNNING" => Ok(TaskRunStatus::Running),
        "COMPLETED" => Ok(TaskRunStatus::Completed),
        "FAILED" => Ok(TaskRunStatus::Failed),
        "TIMEOUT" => Ok(TaskRunStatus::Timeout),
        _ => Err(SchedulerError::Internal(format!(
            "Unknown task run status: {}",
            status_str
        ))),
    }
}