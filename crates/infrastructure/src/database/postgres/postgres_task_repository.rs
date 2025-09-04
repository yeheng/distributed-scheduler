use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_domain::{
    entities::{Task, TaskFilter, TaskStatus},
    repositories::TaskRepository,
    task_query_builder::{TaskQueryBuilder, TaskQueryParam},
};
use scheduler_errors::SchedulerResult;
use sqlx::{PgPool, Row};
use tracing::{debug, instrument};

use super::task_dependency_checker::TaskDependencyChecker;
use crate::{
    error_handling::{RepositoryErrorHelpers, RepositoryOperation},
    task_context,
    timeout_handler::TimeoutUtils,
};

pub struct PostgresTaskRepository {
    pool: PgPool,
    dependency_checker: TaskDependencyChecker,
}

impl PostgresTaskRepository {
    pub fn new(pool: PgPool) -> Self {
        let dependency_checker = TaskDependencyChecker::new(pool.clone());
        Self {
            pool,
            dependency_checker,
        }
    }
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
    fn bind_query_params<'q>(
        &'q self,
        mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
        params: &'q [TaskQueryParam],
    ) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
        for param in params.iter() {
            query = match param {
                TaskQueryParam::String(value) => query.bind(value.as_str()),
                TaskQueryParam::Status(status) => {
                    let status_str = match status {
                        TaskStatus::Active => "ACTIVE",
                        TaskStatus::Inactive => "INACTIVE",
                    };
                    query.bind(status_str)
                }
                TaskQueryParam::Int64(value) => query.bind(*value),
                TaskQueryParam::Int32(value) => query.bind(*value),
                TaskQueryParam::TaskType(value) => query.bind(value.as_str()),
            };
        }
        query
    }
}

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    #[instrument(skip(self, task), fields(
        task_id = %task.id,
        task_name = %task.name,
        task_type = %task.task_type,
    ))]
    async fn create(&self, task: &Task) -> SchedulerResult<Task> {
        let context = task_context!(
            RepositoryOperation::Create,
            task_id = task.id,
            task_name = &task.name
        )
        .with_task_type(task.task_type.clone());

        let shard_config_json = task
            .shard_config
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| RepositoryErrorHelpers::task_serialization_error(context.clone(), e))?;

        let row = TimeoutUtils::database(
            async {
                sqlx::query(
                    r#"
                    INSERT INTO tasks (name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    RETURNING id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at
                    "#,
                )
                .bind(&task.name)
                .bind(&task.task_type)
                .bind(&task.schedule)
                .bind(&task.parameters)
                .bind(task.timeout_seconds)
                .bind(task.max_retries)
                .bind(task.status)
                .bind(&task.dependencies)
                .bind(shard_config_json)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))
            },
            &format!("创建任务 '{}'", task.name),
        )
        .await?;

        let created_task = Self::row_to_task(&row)?;
        RepositoryErrorHelpers::log_operation_success(
            context,
            &created_task.entity_description(),
            Some(&format!(
                "ID: {}, 类型: {}",
                created_task.id, created_task.task_type
            )),
        );
        Ok(created_task)
    }
    #[instrument(skip(self), fields(task_id = %id))]
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
        let context = task_context!(RepositoryOperation::Read, task_id = id);

        let row = TimeoutUtils::database(
            async {
                sqlx::query(
                    "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE id = $1"
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))
            },
            &format!("查询任务 ID {id}"),
        )
        .await?;

        match row {
            Some(row) => {
                let task = Self::row_to_task(&row)?;
                debug!("查询任务成功: ID {}, 名称: {}", task.id, task.name);
                Ok(Some(task))
            }
            None => {
                debug!("查询任务不存在: ID {}", id);
                Ok(None)
            }
        }
    }
    #[instrument(skip(self), fields(task_name = %name))]
    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>> {
        let context = task_context!(RepositoryOperation::Read, task_name = name);

        let row = sqlx::query(
            "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE name = $1"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        match row {
            Some(row) => {
                let task = Self::row_to_task(&row)?;
                debug!("按名称查询任务成功: {} (ID: {})", task.name, task.id);
                Ok(Some(task))
            }
            None => {
                debug!("按名称查询任务不存在: {}", name);
                Ok(None)
            }
        }
    }
    #[instrument(skip(self, task), fields(
        task_id = %task.id,
        task_name = %task.name,
        task_type = %task.task_type,
    ))]
    async fn update(&self, task: &Task) -> SchedulerResult<()> {
        let context = task_context!(
            RepositoryOperation::Update,
            task_id = task.id,
            task_name = &task.name
        )
        .with_task_type(task.task_type.clone());

        let shard_config_json = task
            .shard_config
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| RepositoryErrorHelpers::task_serialization_error(context.clone(), e))?;

        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET name = $2, task_type = $3, schedule = $4, parameters = $5,
                timeout_seconds = $6, max_retries = $7, status = $8,
                dependencies = $9, shard_config = $10, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(task.id)
        .bind(&task.name)
        .bind(&task.task_type)
        .bind(&task.schedule)
        .bind(&task.parameters)
        .bind(task.timeout_seconds)
        .bind(task.max_retries)
        .bind(task.status)
        .bind(&task.dependencies)
        .bind(shard_config_json)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::task_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success(
            context,
            &task.entity_description(),
            Some(&format!(
                "状态: {:?}, 类型: {}",
                task.status, task.task_type
            )),
        );
        Ok(())
    }
    #[instrument(skip(self), fields(task_id = %id))]
    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        let context = task_context!(RepositoryOperation::Delete, task_id = id);

        let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::task_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success(context, &format!("任务 (ID: {id})"), None);
        Ok(())
    }
    #[instrument(skip(self, filter), fields(
        task_status = ?filter.status,
        task_type = ?filter.task_type,
        name_pattern = ?filter.name_pattern,
        limit = ?filter.limit,
    ))]
    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
        let context = task_context!(RepositoryOperation::Query)
            .with_additional_info(format!("过滤器: {filter:?}"));

        let (query, params) = TaskQueryBuilder::build_select_query(filter);

        let mut sqlx_query = sqlx::query(&query);
        sqlx_query = self.bind_query_params(sqlx_query, &params);

        let rows = sqlx_query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        let tasks: SchedulerResult<Vec<Task>> = rows.iter().map(Self::row_to_task).collect();
        let result = tasks?;
        debug!("查询任务列表成功，返回 {} 个任务", result.len());
        Ok(result)
    }
    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
        let filter = TaskFilter {
            status: Some(TaskStatus::Active),
            ..Default::default()
        };
        self.list(&filter).await
    }
    async fn get_schedulable_tasks(
        &self,
        _current_time: DateTime<Utc>,
    ) -> SchedulerResult<Vec<Task>> {
        self.get_active_tasks().await
    }
    async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool> {
        self.dependency_checker.check_dependencies(task_id).await
    }
    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        self.dependency_checker.get_dependencies(task_id).await
    }
    #[instrument(skip(self, task_ids), fields(
        task_count = %task_ids.len(),
        target_status = ?status,
    ))]
    async fn batch_update_status(
        &self,
        task_ids: &[i64],
        status: TaskStatus,
    ) -> SchedulerResult<()> {
        if task_ids.is_empty() {
            debug!("批量更新任务状态: 任务ID列表为空，跳过操作");
            return Ok(());
        }

        let context = task_context!(RepositoryOperation::BatchUpdate).with_additional_info(
            format!("批量更新 {} 个任务状态为 {:?}", task_ids.len(), status),
        );

        let query = TaskQueryBuilder::build_batch_update_query();
        let status_str = match status {
            TaskStatus::Active => "ACTIVE",
            TaskStatus::Inactive => "INACTIVE",
        };

        let result = sqlx::query(&query)
            .bind(status_str)
            .bind(task_ids)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_database_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success(
            context,
            "批量任务状态更新",
            Some(&format!(
                "更新了 {} 个任务的状态为 {}",
                result.rows_affected(),
                status_str
            )),
        );
        Ok(())
    }
}
