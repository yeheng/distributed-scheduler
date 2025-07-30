use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_core::{
    errors::Result,
    errors::SchedulerError,
    models::{WorkerInfo, WorkerStatus},
    traits::{WorkerLoadStats, WorkerRepository},
};
use sqlx::{PgPool, Row};
use tracing::debug;

/// PostgreSQL Worker仓储实现
pub struct PostgresWorkerRepository {
    pool: PgPool,
}

impl PostgresWorkerRepository {
    /// 创建新的PostgreSQL Worker仓储
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 将数据库行转换为WorkerInfo模型
    fn row_to_worker_info(row: &sqlx::postgres::PgRow) -> Result<WorkerInfo> {
        let supported_task_types: Vec<String> = row
            .try_get::<Vec<String>, _>("supported_task_types")
            .unwrap_or_default();

        Ok(WorkerInfo {
            id: row.try_get("id")?,
            hostname: row.try_get("hostname")?,
            ip_address: row.try_get("ip_address")?,
            supported_task_types,
            max_concurrent_tasks: row.try_get("max_concurrent_tasks")?,
            current_task_count: 0, // 这个字段不在数据库中，需要从task_runs表计算
            status: row.try_get("status")?,
            last_heartbeat: row.try_get("last_heartbeat")?,
            registered_at: row.try_get("registered_at")?,
        })
    }
}

#[async_trait]
impl WorkerRepository for PostgresWorkerRepository {
    /// 注册新的Worker
    async fn register(&self, worker: &WorkerInfo) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO workers (id, hostname, ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at)
            VALUES ($1, $2, $3::inet, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                hostname = EXCLUDED.hostname,
                ip_address = EXCLUDED.ip_address,
                supported_task_types = EXCLUDED.supported_task_types,
                max_concurrent_tasks = EXCLUDED.max_concurrent_tasks,
                status = EXCLUDED.status,
                last_heartbeat = EXCLUDED.last_heartbeat
            "#,
        )
        .bind(&worker.id)
        .bind(&worker.hostname)
        .bind(&worker.ip_address)
        .bind(&worker.supported_task_types)
        .bind(worker.max_concurrent_tasks)
        .bind(worker.status)
        .bind(worker.last_heartbeat)
        .bind(worker.registered_at)
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        debug!("注册Worker成功: {}", worker.id);
        Ok(())
    }

    /// 注销Worker
    async fn unregister(&self, worker_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM workers WHERE id = $1")
            .bind(worker_id)
            .execute(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::WorkerNotFound {
                id: worker_id.to_string(),
            });
        }

        debug!("注销Worker成功: {}", worker_id);
        Ok(())
    }

    /// 根据ID获取Worker信息
    async fn get_by_id(&self, worker_id: &str) -> Result<Option<WorkerInfo>> {
        let row = sqlx::query(
            "SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers WHERE id = $1"
        )
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match row {
            Some(row) => {
                let mut worker = Self::row_to_worker_info(&row)?;

                // 计算当前任务数量
                let task_count_row = sqlx::query(
                    "SELECT COUNT(*) as count FROM task_runs WHERE worker_id = $1 AND status IN ('DISPATCHED', 'RUNNING')"
                )
                .bind(worker_id)
                .fetch_one(&self.pool)
                .await
                .map_err(SchedulerError::Database)?;

                worker.current_task_count = task_count_row.try_get::<i64, _>("count")? as i32;
                Ok(Some(worker))
            }
            None => Ok(None),
        }
    }

    /// 更新Worker信息
    async fn update(&self, worker: &WorkerInfo) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE workers 
            SET hostname = $2, ip_address = $3::inet, supported_task_types = $4, 
                max_concurrent_tasks = $5, status = $6, last_heartbeat = $7
            WHERE id = $1
            "#,
        )
        .bind(&worker.id)
        .bind(&worker.hostname)
        .bind(&worker.ip_address)
        .bind(&worker.supported_task_types)
        .bind(worker.max_concurrent_tasks)
        .bind(worker.status)
        .bind(worker.last_heartbeat)
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::WorkerNotFound {
                id: worker.id.to_string(),
            });
        }

        debug!("更新Worker成功: {}", worker.id);
        Ok(())
    }

    /// 获取所有Worker列表
    async fn list(&self) -> Result<Vec<WorkerInfo>> {
        let rows = sqlx::query(
            "SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers ORDER BY registered_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut workers = Vec::new();
        for row in rows {
            let mut worker = Self::row_to_worker_info(&row)?;

            // 计算当前任务数量
            let task_count_row = sqlx::query(
                "SELECT COUNT(*) as count FROM task_runs WHERE worker_id = $1 AND status IN ('DISPATCHED', 'RUNNING')"
            )
            .bind(&worker.id)
            .fetch_one(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

            worker.current_task_count = task_count_row.try_get::<i64, _>("count")? as i32;
            workers.push(worker);
        }

        Ok(workers)
    }

    /// 获取活跃的Worker列表
    async fn get_alive_workers(&self) -> Result<Vec<WorkerInfo>> {
        let rows = sqlx::query(
            "SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers WHERE status = $1 ORDER BY last_heartbeat DESC"
        )
        .bind(WorkerStatus::Alive)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut workers = Vec::new();
        for row in rows {
            let mut worker = Self::row_to_worker_info(&row)?;

            // 计算当前任务数量
            let task_count_row = sqlx::query(
                "SELECT COUNT(*) as count FROM task_runs WHERE worker_id = $1 AND status IN ('DISPATCHED', 'RUNNING')"
            )
            .bind(&worker.id)
            .fetch_one(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

            worker.current_task_count = task_count_row.try_get::<i64, _>("count")? as i32;
            workers.push(worker);
        }

        Ok(workers)
    }

    /// 获取支持指定任务类型的Worker列表
    async fn get_workers_by_task_type(&self, task_type: &str) -> Result<Vec<WorkerInfo>> {
        let rows = sqlx::query(
            "SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers WHERE status = $1 AND $2 = ANY(supported_task_types) ORDER BY last_heartbeat DESC"
        )
        .bind(WorkerStatus::Alive)
        .bind(task_type)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut workers = Vec::new();
        for row in rows {
            let mut worker = Self::row_to_worker_info(&row)?;

            // 计算当前任务数量
            let task_count_row = sqlx::query(
                "SELECT COUNT(*) as count FROM task_runs WHERE worker_id = $1 AND status IN ('DISPATCHED', 'RUNNING')"
            )
            .bind(&worker.id)
            .fetch_one(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

            worker.current_task_count = task_count_row.try_get::<i64, _>("count")? as i32;
            workers.push(worker);
        }

        Ok(workers)
    }

    /// 更新Worker心跳
    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: DateTime<Utc>,
        current_task_count: i32,
    ) -> Result<()> {
        let result =
            sqlx::query("UPDATE workers SET last_heartbeat = $1, status = $2 WHERE id = $3")
                .bind(heartbeat_time)
                .bind(WorkerStatus::Alive)
                .bind(worker_id)
                .execute(&self.pool)
                .await
                .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::WorkerNotFound {
                id: worker_id.to_string(),
            });
        }

        debug!(
            "更新Worker心跳成功: {} (任务数: {})",
            worker_id, current_task_count
        );
        Ok(())
    }

    /// 更新Worker状态
    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> Result<()> {
        let result = sqlx::query("UPDATE workers SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(worker_id)
            .execute(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::WorkerNotFound {
                id: worker_id.to_string(),
            });
        }

        debug!("更新Worker状态成功: {} -> {:?}", worker_id, status);
        Ok(())
    }

    /// 获取超时的Worker列表
    async fn get_timeout_workers(&self, timeout_seconds: i64) -> Result<Vec<WorkerInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at 
            FROM workers 
            WHERE status = $1 
            AND EXTRACT(EPOCH FROM (NOW() - last_heartbeat)) > $2
            ORDER BY last_heartbeat ASC
            "#
        )
        .bind(WorkerStatus::Alive)
        .bind(timeout_seconds)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut workers = Vec::new();
        for row in rows {
            let mut worker = Self::row_to_worker_info(&row)?;

            // 计算当前任务数量
            let task_count_row = sqlx::query(
                "SELECT COUNT(*) as count FROM task_runs WHERE worker_id = $1 AND status IN ('DISPATCHED', 'RUNNING')"
            )
            .bind(&worker.id)
            .fetch_one(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

            worker.current_task_count = task_count_row.try_get::<i64, _>("count")? as i32;
            workers.push(worker);
        }

        Ok(workers)
    }

    /// 清理离线Worker
    async fn cleanup_offline_workers(&self, timeout_seconds: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE workers 
            SET status = $1 
            WHERE status = $2 
            AND EXTRACT(EPOCH FROM (NOW() - last_heartbeat)) > $3
            "#,
        )
        .bind(WorkerStatus::Down)
        .bind(WorkerStatus::Alive)
        .bind(timeout_seconds)
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let updated_count = result.rows_affected();
        debug!("标记 {} 个Worker为离线状态", updated_count);
        Ok(updated_count)
    }

    /// 获取Worker负载统计
    async fn get_worker_load_stats(&self) -> Result<Vec<WorkerLoadStats>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                w.id as worker_id,
                w.max_concurrent_tasks,
                w.last_heartbeat,
                COALESCE(running_tasks.count, 0) as current_task_count,
                COALESCE(completed_stats.total_completed, 0) as total_completed_tasks,
                COALESCE(failed_stats.total_failed, 0) as total_failed_tasks,
                COALESCE(duration_stats.avg_duration_ms, 0)::DOUBLE PRECISION as average_task_duration_ms
            FROM workers w
            LEFT JOIN (
                SELECT worker_id, COUNT(*) as count
                FROM task_runs 
                WHERE status IN ('DISPATCHED', 'RUNNING')
                GROUP BY worker_id
            ) running_tasks ON w.id = running_tasks.worker_id
            LEFT JOIN (
                SELECT worker_id, COUNT(*) as total_completed
                FROM task_runs 
                WHERE status = 'COMPLETED'
                GROUP BY worker_id
            ) completed_stats ON w.id = completed_stats.worker_id
            LEFT JOIN (
                SELECT worker_id, COUNT(*) as total_failed
                FROM task_runs 
                WHERE status IN ('FAILED', 'TIMEOUT')
                GROUP BY worker_id
            ) failed_stats ON w.id = failed_stats.worker_id
            LEFT JOIN (
                SELECT 
                    worker_id, 
                    AVG(EXTRACT(EPOCH FROM (completed_at - started_at)) * 1000)::DOUBLE PRECISION as avg_duration_ms
                FROM task_runs 
                WHERE status = 'COMPLETED' 
                AND started_at IS NOT NULL 
                AND completed_at IS NOT NULL
                GROUP BY worker_id
            ) duration_stats ON w.id = duration_stats.worker_id
            WHERE w.status = $1
            ORDER BY w.last_heartbeat DESC
            "#,
        )
        .bind(WorkerStatus::Alive)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut stats = Vec::new();
        for row in rows {
            let worker_id: String = row.try_get("worker_id")?;
            let max_concurrent_tasks: i32 = row.try_get("max_concurrent_tasks")?;
            let current_task_count: i64 = row.try_get("current_task_count")?;
            let total_completed_tasks: i64 = row.try_get("total_completed_tasks")?;
            let total_failed_tasks: i64 = row.try_get("total_failed_tasks")?;
            let average_task_duration_ms: Option<f64> = row.try_get("average_task_duration_ms")?;
            let last_heartbeat: DateTime<Utc> = row.try_get("last_heartbeat")?;

            let load_percentage = if max_concurrent_tasks > 0 {
                (current_task_count as f64 / max_concurrent_tasks as f64) * 100.0
            } else {
                0.0
            };

            stats.push(WorkerLoadStats {
                worker_id,
                current_task_count: current_task_count as i32,
                max_concurrent_tasks,
                load_percentage,
                total_completed_tasks,
                total_failed_tasks,
                average_task_duration_ms,
                last_heartbeat,
            });
        }

        Ok(stats)
    }

    /// 批量更新Worker状态
    async fn batch_update_status(&self, worker_ids: &[String], status: WorkerStatus) -> Result<()> {
        if worker_ids.is_empty() {
            return Ok(());
        }

        let result = sqlx::query("UPDATE workers SET status = $1 WHERE id = ANY($2)")
            .bind(status)
            .bind(worker_ids)
            .execute(&self.pool)
            .await
            .map_err(SchedulerError::Database)?;

        debug!(
            "批量更新 {} 个Worker状态为 {:?}",
            result.rows_affected(),
            status
        );
        Ok(())
    }
}
