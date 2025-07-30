use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_core::{
    models::{TaskRun, TaskRunStatus},
    traits::{TaskExecutionStats, TaskRunRepository},
    Result, SchedulerError,
};
use sqlx::{PgPool, Row};
use tracing::debug;

/// PostgreSQL任务执行实例仓储实现
pub struct PostgresTaskRunRepository {
    pool: PgPool,
}

impl PostgresTaskRunRepository {
    /// 创建新的PostgreSQL任务执行实例仓储
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 将数据库行转换为TaskRun模型
    fn row_to_task_run(row: &sqlx::postgres::PgRow) -> Result<TaskRun> {
        Ok(TaskRun {
            id: row.try_get("id")?,
            task_id: row.try_get("task_id")?,
            status: row.try_get("status")?,
            worker_id: row.try_get("worker_id")?,
            retry_count: row.try_get("retry_count")?,
            shard_index: row.try_get("shard_index")?,
            shard_total: row.try_get("shard_total")?,
            scheduled_at: row.try_get("scheduled_at")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
            result: row.try_get("result")?,
            error_message: row.try_get("error_message")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[async_trait]
impl TaskRunRepository for PostgresTaskRunRepository {
    /// 创建新的任务执行实例
    async fn create(&self, task_run: &TaskRun) -> Result<TaskRun> {
        let row = sqlx::query(
            r#"
            INSERT INTO task_runs (task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at
            "#,
        )
        .bind(task_run.task_id)
        .bind(task_run.status)
        .bind(&task_run.worker_id)
        .bind(task_run.retry_count)
        .bind(task_run.shard_index)
        .bind(task_run.shard_total)
        .bind(task_run.scheduled_at)
        .bind(task_run.started_at)
        .bind(task_run.completed_at)
        .bind(&task_run.result)
        .bind(&task_run.error_message)
        .fetch_one(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let created_run = Self::row_to_task_run(&row)?;
        debug!("创建任务执行实例成功: ID {}", created_run.id);
        Ok(created_run)
    }

    /// 根据ID获取任务执行实例
    async fn get_by_id(&self, id: i64) -> Result<Option<TaskRun>> {
        let row = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_task_run(&row)?)),
            None => Ok(None),
        }
    }

    /// 更新任务执行实例
    async fn update(&self, task_run: &TaskRun) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE task_runs 
            SET status = $2, worker_id = $3, retry_count = $4, shard_index = $5, shard_total = $6,
                scheduled_at = $7, started_at = $8, completed_at = $9, result = $10, error_message = $11
            WHERE id = $1
            "#,
        )
        .bind(task_run.id)
        .bind(task_run.status)
        .bind(&task_run.worker_id)
        .bind(task_run.retry_count)
        .bind(task_run.shard_index)
        .bind(task_run.shard_total)
        .bind(task_run.scheduled_at)
        .bind(task_run.started_at)
        .bind(task_run.completed_at)
        .bind(&task_run.result)
        .bind(&task_run.error_message)
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::TaskRunNotFound { id: task_run.id });
        }

        debug!("更新任务执行实例成功: ID {}", task_run.id);
        Ok(())
    }

    /// 删除任务执行实例
    async fn delete(&self, id: i64) -> Result<()> {
        let result = sqlx::query("DELETE FROM task_runs WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::TaskRunNotFound { id });
        }

        debug!("删除任务执行实例成功: ID {}", id);
        Ok(())
    }

    /// 根据任务ID获取执行实例列表
    async fn get_by_task_id(&self, task_id: i64) -> Result<Vec<TaskRun>> {
        let rows = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC"
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let task_runs: Result<Vec<TaskRun>> = rows.iter().map(Self::row_to_task_run).collect();
        task_runs
    }

    /// 根据Worker ID获取执行实例列表
    async fn get_by_worker_id(&self, worker_id: &str) -> Result<Vec<TaskRun>> {
        let rows = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE worker_id = $1 ORDER BY created_at DESC"
        )
        .bind(worker_id)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let task_runs: Result<Vec<TaskRun>> = rows.iter().map(Self::row_to_task_run).collect();
        task_runs
    }

    /// 获取指定状态的任务执行实例
    async fn get_by_status(&self, status: TaskRunStatus) -> Result<Vec<TaskRun>> {
        let rows = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE status = $1 ORDER BY created_at DESC"
        )
        .bind(status)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let task_runs: Result<Vec<TaskRun>> = rows.iter().map(Self::row_to_task_run).collect();
        task_runs
    }

    /// 获取待调度的任务执行实例
    async fn get_pending_runs(&self, limit: Option<i64>) -> Result<Vec<TaskRun>> {
        let mut query = "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE status = $1 ORDER BY scheduled_at ASC".to_string();

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }

        let rows = sqlx::query(&query)
            .bind(TaskRunStatus::Pending)
            .fetch_all(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        let task_runs: Result<Vec<TaskRun>> = rows.iter().map(Self::row_to_task_run).collect();
        task_runs
    }

    /// 获取正在运行的任务执行实例
    async fn get_running_runs(&self) -> Result<Vec<TaskRun>> {
        self.get_by_status(TaskRunStatus::Running).await
    }

    /// 获取超时的任务执行实例
    async fn get_timeout_runs(&self, timeout_seconds: i64) -> Result<Vec<TaskRun>> {
        let rows = sqlx::query(
            r#"
            SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at 
            FROM task_runs 
            WHERE status = $1 
            AND started_at IS NOT NULL 
            AND EXTRACT(EPOCH FROM (NOW() - started_at)) > $2
            ORDER BY started_at ASC
            "#
        )
        .bind(TaskRunStatus::Running)
        .bind(timeout_seconds)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let task_runs: Result<Vec<TaskRun>> = rows.iter().map(Self::row_to_task_run).collect();
        task_runs
    }

    /// 更新任务执行状态
    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        worker_id: Option<&str>,
    ) -> Result<()> {
        let mut query = "UPDATE task_runs SET status = $1".to_string();
        let mut param_count = 1;

        if worker_id.is_some() {
            param_count += 1;
            query.push_str(&format!(", worker_id = ${param_count}"));
        }

        // 根据状态更新时间戳
        match status {
            TaskRunStatus::Running => {
                param_count += 1;
                query.push_str(&format!(", started_at = ${param_count}"));
            }
            TaskRunStatus::Completed | TaskRunStatus::Failed | TaskRunStatus::Timeout => {
                param_count += 1;
                query.push_str(&format!(", completed_at = ${param_count}"));
            }
            _ => {}
        }

        param_count += 1;
        query.push_str(&format!(" WHERE id = ${param_count}"));

        let mut sqlx_query = sqlx::query(&query).bind(status);

        if let Some(worker_id) = worker_id {
            sqlx_query = sqlx_query.bind(worker_id);
        }

        match status {
            TaskRunStatus::Running => {
                sqlx_query = sqlx_query.bind(Utc::now());
            }
            TaskRunStatus::Completed | TaskRunStatus::Failed | TaskRunStatus::Timeout => {
                sqlx_query = sqlx_query.bind(Utc::now());
            }
            _ => {}
        }

        sqlx_query = sqlx_query.bind(id);

        let result = sqlx_query
            .execute(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::TaskRunNotFound { id });
        }

        debug!("更新任务执行状态成功: ID {} -> {:?}", id, status);
        Ok(())
    }

    /// 更新任务执行结果
    async fn update_result(
        &self,
        id: i64,
        result: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let query_result =
            sqlx::query("UPDATE task_runs SET result = $1, error_message = $2 WHERE id = $3")
                .bind(result)
                .bind(error_message)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(SchedulerError::Database)?;

        if query_result.rows_affected() == 0 {
            return Err(SchedulerError::TaskRunNotFound { id });
        }

        debug!("更新任务执行结果成功: ID {}", id);
        Ok(())
    }

    /// 获取任务的最近执行记录
    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> Result<Vec<TaskRun>> {
        let rows = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let task_runs: Result<Vec<TaskRun>> = rows.iter().map(Self::row_to_task_run).collect();
        task_runs
    }

    /// 获取任务执行统计信息
    async fn get_execution_stats(&self, task_id: i64, days: i32) -> Result<TaskExecutionStats> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_runs,
                COUNT(CASE WHEN status = 'COMPLETED' THEN 1 END) as successful_runs,
                COUNT(CASE WHEN status = 'FAILED' THEN 1 END) as failed_runs,
                COUNT(CASE WHEN status = 'TIMEOUT' THEN 1 END) as timeout_runs,
                AVG(CASE WHEN completed_at IS NOT NULL AND started_at IS NOT NULL 
                    THEN EXTRACT(EPOCH FROM (completed_at - started_at)) * 1000 END) as avg_execution_time_ms,
                MAX(created_at) as last_execution
            FROM task_runs 
            WHERE task_id = $1 AND created_at >= NOW() - INTERVAL '%d days'
            "#
        )
        .bind(task_id)
        .bind(days)
        .fetch_one(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let total_runs: i64 = row.try_get("total_runs")?;
        let successful_runs: i64 = row.try_get("successful_runs")?;
        let failed_runs: i64 = row.try_get("failed_runs")?;
        let timeout_runs: i64 = row.try_get("timeout_runs")?;
        let average_execution_time_ms: Option<f64> = row.try_get("avg_execution_time_ms")?;
        let last_execution: Option<DateTime<Utc>> = row.try_get("last_execution")?;

        let success_rate = if total_runs > 0 {
            (successful_runs as f64 / total_runs as f64) * 100.0
        } else {
            0.0
        };

        Ok(TaskExecutionStats {
            task_id,
            total_runs,
            successful_runs,
            failed_runs,
            timeout_runs,
            average_execution_time_ms,
            success_rate,
            last_execution,
        })
    }

    /// 清理过期的任务执行记录
    async fn cleanup_old_runs(&self, days: i32) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM task_runs WHERE created_at < NOW() - INTERVAL '%d days'")
                .bind(days)
                .execute(&self.pool)
                .await
                .map_err(SchedulerError::Database)?;

        let deleted_count = result.rows_affected();
        debug!("清理了 {} 条过期执行记录", deleted_count);
        Ok(deleted_count)
    }

    /// 批量更新任务执行状态
    async fn batch_update_status(&self, run_ids: &[i64], status: TaskRunStatus) -> Result<()> {
        if run_ids.is_empty() {
            return Ok(());
        }

        let result = sqlx::query("UPDATE task_runs SET status = $1 WHERE id = ANY($2)")
            .bind(status)
            .bind(run_ids)
            .execute(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        debug!(
            "批量更新 {} 个任务执行状态为 {:?}",
            result.rows_affected(),
            status
        );
        Ok(())
    }
}
