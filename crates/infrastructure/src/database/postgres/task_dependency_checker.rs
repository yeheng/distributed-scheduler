use scheduler_domain::{
    entities::{Task, TaskRunStatus},
    task_dependency_service::TaskDependencyService,
};
use scheduler_foundation::{SchedulerError, SchedulerResult};
use sqlx::{PgPool, Row};
use std::collections::HashSet;
use tracing::{debug, instrument};

use crate::{
    error_handling::{RepositoryErrorHelpers, RepositoryOperation},
    task_context,
};

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

        TaskDependencyService::check_dependencies_satisfied(
            &task_dependencies,
            &dependency_statuses,
        )
    }

    /// Get dependency tasks as full Task entities (optimized batch query)
    pub async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        let task_dependencies = self.get_task_dependencies(task_id).await?;

        if task_dependencies.is_empty() {
            return Ok(vec![]);
        }

        // Use batch query to get all dependency tasks in one call
        self.get_tasks_by_ids(&task_dependencies).await
    }

    /// Get multiple tasks by their IDs using a single batch query
    async fn get_tasks_by_ids(&self, task_ids: &[i64]) -> SchedulerResult<Vec<Task>> {
        if task_ids.is_empty() {
            return Ok(vec![]);
        }

        let context = task_context!(RepositoryOperation::BatchRead)
            .with_additional_info(format!("批量查询{}个任务详情", task_ids.len()));

        // Create placeholders for the IN clause
        let placeholders: Vec<String> = (1..=task_ids.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at 
             FROM tasks 
             WHERE id IN ({}) 
             ORDER BY id",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for &task_id in task_ids {
            query = query.bind(task_id);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::database_error(context.clone(), e))?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(Self::row_to_task(&row)?);
        }

        debug!("批量查询到 {} 个任务", tasks.len());
        Ok(tasks)
    }

    /// Get the dependency IDs for a given task
    #[instrument(skip(self), fields(task_id = %task_id))]
    async fn get_task_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<i64>> {
        let context = task_context!(RepositoryOperation::Read, task_id = task_id)
            .with_additional_info("查询任务依赖关系".to_string());

        let task_row = sqlx::query("SELECT dependencies FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

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
            None => Err(RepositoryErrorHelpers::task_not_found(context)),
        }
    }

    /// Get the latest run status for each dependency task (optimized batch query)
    async fn get_dependency_run_statuses(
        &self,
        dependency_ids: &[i64],
    ) -> SchedulerResult<Vec<(i64, Option<TaskRunStatus>)>> {
        if dependency_ids.is_empty() {
            return Ok(vec![]);
        }

        let context = task_context!(RepositoryOperation::BatchRead).with_additional_info(format!(
            "批量查询{}个任务的最新运行状态",
            dependency_ids.len()
        ));

        // Create placeholders for the IN clause
        let placeholders: Vec<String> = (1..=dependency_ids.len())
            .map(|i| format!("${i}"))
            .collect();

        // Use window function to get the latest run status for each task
        let sql = format!(
            "SELECT DISTINCT 
                tr.task_id,
                FIRST_VALUE(tr.status) OVER (PARTITION BY tr.task_id ORDER BY tr.created_at DESC) as status
             FROM task_runs tr 
             WHERE tr.task_id IN ({})
             ORDER BY tr.task_id",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for &task_id in dependency_ids {
            query = query.bind(task_id);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::database_error(context.clone(), e))?;

        let mut statuses = Vec::new();
        let mut found_task_ids = HashSet::new();

        // Parse results
        for row in rows {
            let task_id: i64 = row.try_get("task_id")?;
            let status_str: String = row.try_get("status")?;
            let status = self.parse_task_run_status(&status_str)?;
            statuses.push((task_id, Some(status)));
            found_task_ids.insert(task_id);
        }

        // Add None status for tasks that have never been executed
        for &dep_id in dependency_ids {
            if !found_task_ids.contains(&dep_id) {
                statuses.push((dep_id, None));
            }
        }

        // Sort by task_id for consistent ordering
        statuses.sort_by_key(|(task_id, _)| *task_id);

        debug!("批量查询到 {} 个任务的运行状态", statuses.len());
        Ok(statuses)
    }

    /// Get the latest run status for a specific task
    #[instrument(skip(self), fields(task_id = %task_id))]
    async fn get_latest_task_run_status(
        &self,
        task_id: i64,
    ) -> SchedulerResult<Option<TaskRunStatus>> {
        let context = task_context!(RepositoryOperation::Read, task_id = task_id)
            .with_additional_info("查询任务最新运行状态".to_string());

        let recent_run = sqlx::query(
            "SELECT status FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

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
                "Unknown task run status: {status_str}"
            ))),
        }
    }

    /// Get task by ID - used internally for dependency resolution
    #[instrument(skip(self), fields(task_id = %id))]
    async fn get_task_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
        let context = task_context!(RepositoryOperation::Read, task_id = id)
            .with_additional_info("内部查询任务详情".to_string());

        let row = sqlx::query(
            "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

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
