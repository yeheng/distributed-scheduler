use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_core::{
    SchedulerResult,
    errors::SchedulerError,
    models::{Task, TaskFilter, TaskStatus},
    traits::TaskRepository,
};
use sqlx::{PgPool, Row};
use tracing::debug;

/// PostgreSQL任务仓储实现
pub struct PostgresTaskRepository {
    pool: PgPool,
}

impl PostgresTaskRepository {
    /// 创建新的PostgreSQL任务仓储
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
        let mut query = "SELECT id, name, task_type, schedule, parameters, timeout_seconds, max_retries, status, dependencies, shard_config, created_at, updated_at FROM tasks WHERE 1=1".to_string();
        let mut bind_count = 0;

        // 构建动态查询
        if filter.status.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND status = ${bind_count}"));
        }

        if filter.task_type.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND task_type = ${bind_count}"));
        }

        if filter.name_pattern.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND name ILIKE ${bind_count}"));
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(_limit) = filter.limit {
            bind_count += 1;
            query.push_str(&format!(" LIMIT ${bind_count}"));
        }

        if let Some(_offset) = filter.offset {
            bind_count += 1;
            query.push_str(&format!(" OFFSET ${bind_count}"));
        }

        let mut sqlx_query = sqlx::query(&query);

        // 绑定参数
        if let Some(status) = filter.status {
            sqlx_query = sqlx_query.bind(status);
        }

        if let Some(task_type) = &filter.task_type {
            sqlx_query = sqlx_query.bind(task_type);
        }

        if let Some(name_pattern) = &filter.name_pattern {
            sqlx_query = sqlx_query.bind(format!("%{name_pattern}%"));
        }

        if let Some(limit) = filter.limit {
            sqlx_query = sqlx_query.bind(limit);
        }

        if let Some(offset) = filter.offset {
            sqlx_query = sqlx_query.bind(offset);
        }

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
    async fn get_schedulable_tasks(&self, _current_time: DateTime<Utc>) -> SchedulerResult<Vec<Task>> {
        // 这里简化实现，实际应该解析cron表达式判断是否到达调度时间
        self.get_active_tasks().await
    }

    /// 检查任务依赖是否满足
    async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool> {
        let task = self.get_by_id(task_id).await?;

        if let Some(task) = task {
            if task.dependencies.is_empty() {
                return Ok(true);
            }

            // 检查所有依赖任务的最近执行是否成功
            for dep_id in &task.dependencies {
                let recent_run = sqlx::query(
                    "SELECT status FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC LIMIT 1"
                )
                .bind(dep_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(SchedulerError::Database)?;

                match recent_run {
                    Some(row) => {
                        let status: String = row.try_get("status")?;
                        if status != "COMPLETED" {
                            return Ok(false);
                        }
                    }
                    None => return Ok(false), // 依赖任务从未执行
                }
            }
            Ok(true)
        } else {
            Err(SchedulerError::TaskNotFound { id: task_id })
        }
    }

    /// 获取任务的依赖列表
    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        let task = self.get_by_id(task_id).await?;

        if let Some(task) = task {
            if task.dependencies.is_empty() {
                return Ok(vec![]);
            }

            let mut dependencies = Vec::new();
            for dep_id in &task.dependencies {
                if let Some(dep_task) = self.get_by_id(*dep_id).await? {
                    dependencies.push(dep_task);
                }
            }
            Ok(dependencies)
        } else {
            Err(SchedulerError::TaskNotFound { id: task_id })
        }
    }

    /// 批量更新任务状态
    async fn batch_update_status(&self, task_ids: &[i64], status: TaskStatus) -> SchedulerResult<()> {
        if task_ids.is_empty() {
            return Ok(());
        }

        let status_str = match status {
            TaskStatus::Active => "ACTIVE",
            TaskStatus::Inactive => "INACTIVE",
        };

        let result =
            sqlx::query("UPDATE tasks SET status = $1, updated_at = NOW() WHERE id = ANY($2)")
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
