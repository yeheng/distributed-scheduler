use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_core::SchedulerResult;
use scheduler_domain::{
    entities::{TaskRun, TaskRunStatus},
    repositories::{TaskExecutionStats, TaskRunRepository},
};
use sqlx::{PgPool, Row};
use tracing::{debug, instrument};

use crate::{task_run_context, error_handling::{
    RepositoryErrorHelpers, RepositoryOperation,
}};

pub struct PostgresTaskRunRepository {
    pool: PgPool,
}

impl PostgresTaskRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    fn row_to_task_run(row: &sqlx::postgres::PgRow) -> SchedulerResult<TaskRun> {
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
    #[instrument(skip(self, task_run), fields(
        task_id = %task_run.task_id,
        run_status = ?task_run.status,
        worker_id = ?task_run.worker_id,
        retry_count = %task_run.retry_count,
    ))]
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun> {
        let context = task_run_context!(RepositoryOperation::Create, 
            task_id = task_run.task_id)
            .with_worker_id(task_run.worker_id.clone().unwrap_or_default())
            .with_status(task_run.status)
            .with_additional_info(format!("重试次数: {}, 分片: {:?}", task_run.retry_count, task_run.shard_index));

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
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        let created_run = Self::row_to_task_run(&row)?;
        RepositoryErrorHelpers::log_operation_success_task_run(
            context, 
            &created_run.entity_description(), 
            Some(&format!("状态: {:?}, Worker: {:?}", created_run.status, created_run.worker_id))
        );
        Ok(created_run)
    }
    #[instrument(skip(self), fields(run_id = %id))]
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>> {
        let context = task_run_context!(RepositoryOperation::Read, run_id = id);

        let row = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        match row {
            Some(row) => {
                let task_run = Self::row_to_task_run(&row)?;
                debug!("查询任务执行实例成功: ID {}, 任务ID: {}, 状态: {:?}", 
                    task_run.id, task_run.task_id, task_run.status);
                Ok(Some(task_run))
            }
            None => {
                debug!("查询任务执行实例不存在: ID {}", id);
                Ok(None)
            }
        }
    }
    #[instrument(skip(self, task_run), fields(
        run_id = %task_run.id,
        task_id = %task_run.task_id,
        status = ?task_run.status,
        worker_id = ?task_run.worker_id,
    ))]
    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        let context = task_run_context!(RepositoryOperation::Update, 
            run_id = task_run.id, 
            task_id = task_run.task_id)
            .with_worker_id(task_run.worker_id.clone().unwrap_or_default())
            .with_status(task_run.status)
            .with_additional_info(format!("重试次数: {}, 状态变更", task_run.retry_count));

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
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::task_run_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success_task_run(
            context, 
            &task_run.entity_description(), 
            Some(&format!("状态: {:?}, Worker: {:?}", task_run.status, task_run.worker_id))
        );
        Ok(())
    }
    #[instrument(skip(self), fields(run_id = %id))]
    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        let context = task_run_context!(RepositoryOperation::Delete, run_id = id);

        let result = sqlx::query("DELETE FROM task_runs WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::task_run_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success_task_run(
            context, 
            &format!("任务执行实例 (ID: {})", id), 
            None
        );
        Ok(())
    }
    #[instrument(skip(self), fields(task_id = %task_id))]
    async fn get_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>> {
        let context = task_run_context!(RepositoryOperation::Query, task_id = task_id);

        let rows = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC"
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        let task_runs: SchedulerResult<Vec<TaskRun>> =
            rows.iter().map(Self::row_to_task_run).collect();
        
        let result = task_runs?;
        debug!("查询任务执行实例成功: 任务ID {}, 返回 {} 个执行记录", task_id, result.len());
        Ok(result)
    }
    #[instrument(skip(self), fields(worker_id = %worker_id))]
    async fn get_by_worker_id(&self, worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
        let context = task_run_context!(RepositoryOperation::Query)
            .with_worker_id(worker_id.to_string());

        let rows = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE worker_id = $1 ORDER BY created_at DESC"
        )
        .bind(worker_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        let task_runs: SchedulerResult<Vec<TaskRun>> =
            rows.iter().map(Self::row_to_task_run).collect();
        
        let result = task_runs?;
        debug!("查询Worker任务执行记录成功: Worker {}, 返回 {} 个执行记录", worker_id, result.len());
        Ok(result)
    }
    #[instrument(skip(self), fields(status = ?status))]
    async fn get_by_status(&self, status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>> {
        let context = task_run_context!(RepositoryOperation::Query)
            .with_status(status);

        let rows = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE status = $1 ORDER BY created_at DESC"
        )
        .bind(status)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        let task_runs: SchedulerResult<Vec<TaskRun>> =
            rows.iter().map(Self::row_to_task_run).collect();
        
        let result = task_runs?;
        debug!("查询任务执行记录成功: 状态 {:?}, 返回 {} 个执行记录", status, result.len());
        Ok(result)
    }
    #[instrument(skip(self), fields(limit = ?limit))]
    async fn get_pending_runs(&self, limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>> {
        let context = task_run_context!(RepositoryOperation::Query)
            .with_status(TaskRunStatus::Pending)
            .with_additional_info(format!("限制数量: {:?}", limit));

        let mut query = "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE status = $1 ORDER BY scheduled_at ASC".to_string();

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }

        let rows = sqlx::query(&query)
            .bind(TaskRunStatus::Pending)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        let task_runs: SchedulerResult<Vec<TaskRun>> =
            rows.iter().map(Self::row_to_task_run).collect();
        
        let result = task_runs?;
        debug!("查询待执行任务成功: 返回 {} 个记录", result.len());
        Ok(result)
    }
    #[instrument(skip(self))]
    async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>> {
        self.get_by_status(TaskRunStatus::Running).await
    }
    #[instrument(skip(self), fields(timeout_seconds = %timeout_seconds))]
    async fn get_timeout_runs(&self, timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>> {
        let context = task_run_context!(RepositoryOperation::Query)
            .with_status(TaskRunStatus::Running)
            .with_additional_info(format!("超时阈值: {}秒", timeout_seconds));

        let rows = sqlx::query(
            r#"
            SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at 
            FROM task_runs 
            WHERE status = $1 
            AND started_at IS NOT NULL 
            AND EXTRACT(EPOCH FROM (NOW() - started_at)) > $2
            ORDER BY started_at ASC
            "#,
        )
        .bind(TaskRunStatus::Running)
        .bind(timeout_seconds)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        let task_runs: SchedulerResult<Vec<TaskRun>> =
            rows.iter().map(Self::row_to_task_run).collect();
        
        let result = task_runs?;
        debug!("查询超时任务成功: 超时{}秒, 返回 {} 个记录", timeout_seconds, result.len());
        Ok(result)
    }
    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        worker_id: Option<&str>,
    ) -> SchedulerResult<()> {
        let context = task_run_context!(RepositoryOperation::Update, run_id = id)
            .with_status(status)
            .with_worker_id(worker_id.unwrap_or_default().to_string())
            .with_additional_info(format!("状态变更为 {:?}", status));

        let mut query = "UPDATE task_runs SET status = $1".to_string();
        let mut param_count = 1;

        if worker_id.is_some() {
            param_count += 1;
            query.push_str(&format!(", worker_id = ${param_count}"));
        }
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
            .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::task_run_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success_task_run(
            context, 
            &format!("任务执行状态更新 (ID: {})", id), 
            Some(&format!("状态: {:?}, Worker: {:?}", status, worker_id))
        );
        Ok(())
    }
    async fn update_result(
        &self,
        id: i64,
        result: Option<&str>,
        error_message: Option<&str>,
    ) -> SchedulerResult<()> {
        let context = task_run_context!(RepositoryOperation::Update, run_id = id)
            .with_additional_info("更新任务执行结果".to_string());

        let query_result =
            sqlx::query("UPDATE task_runs SET result = $1, error_message = $2 WHERE id = $3")
                .bind(result)
                .bind(error_message)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        if query_result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::task_run_not_found(context));
        }

        debug!("更新任务执行结果成功: ID {}", id);
        Ok(())
    }
    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> SchedulerResult<Vec<TaskRun>> {
        let context = task_run_context!(RepositoryOperation::Query, task_id = task_id)
            .with_additional_info(format!("查询最近执行记录，限制: {}", limit));

        let rows = sqlx::query(
            "SELECT id, task_id, status, worker_id, retry_count, shard_index, shard_total, scheduled_at, started_at, completed_at, result, error_message, created_at FROM task_runs WHERE task_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        let task_runs: SchedulerResult<Vec<TaskRun>> =
            rows.iter().map(Self::row_to_task_run).collect();
        
        let result = task_runs?;
        debug!("查询任务最近执行记录成功: 任务ID {}, 返回 {} 条记录", task_id, result.len());
        Ok(result)
    }
    async fn get_execution_stats(
        &self,
        task_id: i64,
        days: i32,
    ) -> SchedulerResult<TaskExecutionStats> {
        let context = task_run_context!(RepositoryOperation::Query, task_id = task_id)
            .with_additional_info(format!("查询任务执行统计，时间范围: {}天", days));

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
        .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

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
    async fn cleanup_old_runs(&self, days: i32) -> SchedulerResult<u64> {
        let context = task_run_context!(RepositoryOperation::Delete)
            .with_additional_info(format!("清理过期执行记录，天数: {}", days));

        let result =
            sqlx::query("DELETE FROM task_runs WHERE created_at < NOW() - INTERVAL '%d days'")
                .bind(days)
                .execute(&self.pool)
                .await
                .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        let deleted_count = result.rows_affected();
        
        RepositoryErrorHelpers::log_operation_success_task_run(
            context, 
            &"过期执行记录清理操作", 
            Some(&format!("删除了 {} 条过期记录", deleted_count))
        );
        
        Ok(deleted_count)
    }
    async fn batch_update_status(
        &self,
        run_ids: &[i64],
        status: TaskRunStatus,
    ) -> SchedulerResult<()> {
        if run_ids.is_empty() {
            debug!("批量更新任务执行状态: 执行ID列表为空，跳过操作");
            return Ok(());
        }

        let context = task_run_context!(RepositoryOperation::BatchUpdate)
            .with_status(status)
            .with_additional_info(format!("批量更新 {} 个任务执行状态为 {:?}", run_ids.len(), status));

        let result = sqlx::query("UPDATE task_runs SET status = $1 WHERE id = ANY($2)")
            .bind(status)
            .bind(run_ids)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::task_run_database_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success_task_run(
            context, 
            &"批量任务执行状态更新", 
            Some(&format!("更新了 {} 个执行记录的状态为 {:?}", result.rows_affected(), status))
        );
        Ok(())
    }
}
