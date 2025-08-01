use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_core::{
    errors::SchedulerError,
    models::{Task, TaskFilter, TaskStatus},
    traits::TaskRepository,
    SchedulerResult,
};
use sqlx::{PgPool, Row};
use tracing::debug;

use super::{
    task_dependency_checker::TaskDependencyChecker,
    task_query_builder::{TaskQueryBuilder, TaskQueryParam},
};

/// Refactored PostgreSQL任务仓储实现
///
/// 这个重构版本使用了专门的组件来处理：
/// - 查询构建 (TaskQueryBuilder)
/// - 依赖检查 (TaskDependencyChecker)
///
/// 这遵循了单一职责原则，每个组件只负责一个特定的功能。
pub struct PostgresTaskRepository {
    pool: PgPool,
    dependency_checker: TaskDependencyChecker,
}

impl PostgresTaskRepository {
    /// 创建新的PostgreSQL任务仓储
    pub fn new(pool: PgPool) -> Self {
        let dependency_checker = TaskDependencyChecker::new(pool.clone());
        Self {
            pool,
            dependency_checker: dependency_checker,
        }
    }

    /// 将数据库行转换为Task模型
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

    /// 绑定查询参数到SQL查询
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
            };
        }
        query
    }
}

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    /// 创建新任务
    async fn create(&self, task: &Task) -> SchedulerResult<Task> {
        let shard_config_json = task
            .shard_config
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| SchedulerError::Serialization(format!("序列化分片配置失败: {e}")))?;

        let row = sqlx::query(
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
        .map_err(SchedulerError::Database)?;

        let created_task = Self::row_to_task(&row)?;
        debug!(
            "创建任务成功: {} (ID: {})",
            created_task.name, created_task.id
        );
        Ok(created_task)
    }

    /// 根据ID获取任务
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
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

    /// 根据名称获取任务
    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>> {
        let row = sqlx::query(
            "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE name = $1"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_task(&row)?)),
            None => Ok(None),
        }
    }

    /// 更新任务
    async fn update(&self, task: &Task) -> SchedulerResult<()> {
        let shard_config_json = task
            .shard_config
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| SchedulerError::Serialization(format!("序列化分片配置失败: {e}")))?;

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
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::TaskNotFound { id: task.id });
        }

        debug!("更新任务成功: {} (ID: {})", task.name, task.id);
        Ok(())
    }

    /// 删除任务
    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::TaskNotFound { id });
        }

        debug!("删除任务成功: ID {}", id);
        Ok(())
    }

    /// 根据过滤条件查询任务列表
    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
        let (query, params) = TaskQueryBuilder::build_select_query(filter);

        let mut sqlx_query = sqlx::query(&query);
        sqlx_query = self.bind_query_params(sqlx_query, &params);

        let rows = sqlx_query
            .fetch_all(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        let tasks: SchedulerResult<Vec<Task>> = rows.iter().map(Self::row_to_task).collect();
        tasks
    }

    /// 获取所有活跃任务
    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
        let filter = TaskFilter {
            status: Some(TaskStatus::Active),
            ..Default::default()
        };
        self.list(&filter).await
    }

    /// 获取需要调度的任务（活跃且到达调度时间）
    async fn get_schedulable_tasks(
        &self,
        _current_time: DateTime<Utc>,
    ) -> SchedulerResult<Vec<Task>> {
        // 这里简化实现，实际应该解析cron表达式判断是否到达调度时间
        self.get_active_tasks().await
    }

    /// 检查任务依赖是否满足
    async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool> {
        self.dependency_checker.check_dependencies(task_id).await
    }

    /// 获取任务的依赖列表
    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        self.dependency_checker.get_dependencies(task_id).await
    }

    /// 批量更新任务状态
    async fn batch_update_status(
        &self,
        task_ids: &[i64],
        status: TaskStatus,
    ) -> SchedulerResult<()> {
        if task_ids.is_empty() {
            return Ok(());
        }

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
            .map_err(SchedulerError::Database)?;

        debug!(
            "批量更新 {} 个任务状态为 {}",
            result.rows_affected(),
            status_str
        );
        Ok(())
    }
}
