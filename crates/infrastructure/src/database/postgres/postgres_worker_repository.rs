use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_core::SchedulerResult;
use scheduler_domain::{
    entities::{WorkerInfo, WorkerStatus},
    repositories::{WorkerLoadStats, WorkerRepository},
};
use sqlx::{PgPool, Row};
use tracing::{debug, instrument};

use crate::{worker_context, error_handling::{
    RepositoryErrorHelpers, RepositoryOperation,
}};

pub struct PostgresWorkerRepository {
    pool: PgPool,
}

impl PostgresWorkerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    fn row_to_worker_info(row: &sqlx::postgres::PgRow) -> SchedulerResult<WorkerInfo> {
        let supported_task_types: Vec<String> = row
            .try_get::<Vec<String>, _>("supported_task_types")
            .unwrap_or_default();

        Ok(WorkerInfo {
            id: row.try_get("id")?,
            hostname: row.try_get("hostname")?,
            ip_address: row.try_get("ip_address")?,
            supported_task_types,
            max_concurrent_tasks: row.try_get("max_concurrent_tasks")?,
            current_task_count: row.try_get::<i32, _>("current_task_count").unwrap_or(0),
            status: row.try_get("status")?,
            last_heartbeat: row.try_get("last_heartbeat")?,
            registered_at: row.try_get("registered_at")?,
        })
    }
}

#[async_trait]
impl WorkerRepository for PostgresWorkerRepository {
    #[instrument(skip(self, worker), fields(
        worker_id = %worker.id,
        hostname = %worker.hostname,
        status = ?worker.status,
        task_types = ?worker.supported_task_types,
    ))]
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let context = worker_context!(RepositoryOperation::Create, worker_id = &worker.id)
            .with_hostname(worker.hostname.clone())
            .with_status(worker.status)
            .with_additional_info(format!(
                "任务类型: {:?}, 最大并发: {}",
                worker.supported_task_types, worker.max_concurrent_tasks
            ));

        let _result = sqlx::query(
            r#"
            INSERT INTO workers (id, hostname, ip_address, supported_task_types, max_concurrent_tasks, current_task_count, status, last_heartbeat, registered_at)
            VALUES ($1, $2, $3::inet, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO UPDATE SET
                hostname = EXCLUDED.hostname,
                ip_address = EXCLUDED.ip_address,
                supported_task_types = EXCLUDED.supported_task_types,
                max_concurrent_tasks = EXCLUDED.max_concurrent_tasks,
                current_task_count = EXCLUDED.current_task_count,
                status = EXCLUDED.status,
                last_heartbeat = EXCLUDED.last_heartbeat
            "#,
        )
        .bind(&worker.id)
        .bind(&worker.hostname)
        .bind(&worker.ip_address)
        .bind(&worker.supported_task_types)
        .bind(worker.max_concurrent_tasks)
        .bind(worker.current_task_count)
        .bind(worker.status)
        .bind(worker.last_heartbeat)
        .bind(worker.registered_at)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success_worker(
            context,
            &format!("Worker '{}'", worker.id),
            Some(&format!("主机名: {}, 状态: {:?}", worker.hostname, worker.status))
        );
        Ok(())
    }
    #[instrument(skip(self), fields(worker_id = %worker_id))]
    async fn unregister(&self, worker_id: &str) -> SchedulerResult<()> {
        let context = worker_context!(RepositoryOperation::Delete, worker_id = worker_id);

        let result = sqlx::query("DELETE FROM workers WHERE id = $1")
            .bind(worker_id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::worker_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success_worker(
            context,
            &format!("Worker '{}'", worker_id),
            None
        );
        Ok(())
    }
    #[instrument(skip(self), fields(worker_id = %worker_id))]
    async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
        let context = worker_context!(RepositoryOperation::Read, worker_id = worker_id);

        let row = sqlx::query(
            "SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, current_task_count, status, last_heartbeat, registered_at FROM workers WHERE id = $1"
        )
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        match row {
            Some(row) => {
                let worker = Self::row_to_worker_info(&row)?;
                debug!("查询Worker成功: {} ({})", worker.id, worker.hostname);
                Ok(Some(worker))
            }
            None => {
                debug!("查询Worker不存在: {}", worker_id);
                Ok(None)
            }
        }
    }
    #[instrument(skip(self, worker), fields(
        worker_id = %worker.id,
        hostname = %worker.hostname,
        status = ?worker.status,
        current_tasks = %worker.current_task_count,
    ))]
    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let context = worker_context!(RepositoryOperation::Update, worker_id = &worker.id)
            .with_hostname(worker.hostname.clone())
            .with_status(worker.status)
            .with_additional_info(format!(
                "当前任务: {}, 最大并发: {}",
                worker.current_task_count, worker.max_concurrent_tasks
            ));

        let result = sqlx::query(
            r#"
            UPDATE workers 
            SET hostname = $2, ip_address = $3::inet, supported_task_types = $4, 
                max_concurrent_tasks = $5, current_task_count = $6, status = $7, last_heartbeat = $8
            WHERE id = $1
            "#,
        )
        .bind(&worker.id)
        .bind(&worker.hostname)
        .bind(&worker.ip_address)
        .bind(&worker.supported_task_types)
        .bind(worker.max_concurrent_tasks)
        .bind(worker.current_task_count)
        .bind(worker.status)
        .bind(worker.last_heartbeat)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::worker_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success_worker(
            context,
            &format!("Worker '{}'", worker.id),
            Some(&format!("状态: {:?}, 任务数: {}", worker.status, worker.current_task_count))
        );
        Ok(())
    }
    #[instrument(skip(self))]
    async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        let context = worker_context!(RepositoryOperation::Query);

        let rows = sqlx::query(
            "SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, current_task_count, status, last_heartbeat, registered_at FROM workers ORDER BY registered_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        let workers: SchedulerResult<Vec<WorkerInfo>> = rows.iter().map(Self::row_to_worker_info).collect();
        let result = workers?;
        
        debug!("查询Worker列表成功，返回 {} 个Worker", result.len());
        Ok(result)
    }
    #[instrument(skip(self))]
    async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        let context = worker_context!(RepositoryOperation::Query)
            .with_status(WorkerStatus::Alive)
            .with_additional_info("查询活跃Worker".to_string());

        let rows = sqlx::query(
            "SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, current_task_count, status, last_heartbeat, registered_at FROM workers WHERE status = $1 ORDER BY last_heartbeat DESC"
        )
        .bind(WorkerStatus::Alive)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        let workers: SchedulerResult<Vec<WorkerInfo>> = rows.iter().map(Self::row_to_worker_info).collect();
        let result = workers?;
        
        debug!("查询活跃Worker成功，返回 {} 个Worker", result.len());
        Ok(result)
    }
    #[instrument(skip(self), fields(task_type = %task_type))]
    async fn get_workers_by_task_type(&self, task_type: &str) -> SchedulerResult<Vec<WorkerInfo>> {
        let context = worker_context!(RepositoryOperation::Query)
            .with_status(WorkerStatus::Alive)
            .with_additional_info(format!("按任务类型查询: {}", task_type));

        let rows = sqlx::query(
            "SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, current_task_count, status, last_heartbeat, registered_at FROM workers WHERE status = $1 AND $2 = ANY(supported_task_types) ORDER BY last_heartbeat DESC"
        )
        .bind(WorkerStatus::Alive)
        .bind(task_type)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        let workers: SchedulerResult<Vec<WorkerInfo>> = rows.iter().map(Self::row_to_worker_info).collect();
        let result = workers?;
        
        debug!("按任务类型 {} 查询Worker成功，返回 {} 个Worker", task_type, result.len());
        Ok(result)
    }
    #[instrument(skip(self), fields(
        worker_id = %worker_id,
        current_task_count = %current_task_count,
    ))]
    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: DateTime<Utc>,
        current_task_count: i32,
    ) -> SchedulerResult<()> {
        let context = worker_context!(RepositoryOperation::Update, worker_id = worker_id)
            .with_status(WorkerStatus::Alive)
            .with_additional_info(format!("心跳更新，当前任务数: {}", current_task_count));

        let result =
            sqlx::query("UPDATE workers SET last_heartbeat = $1, status = $2, current_task_count = $3 WHERE id = $4")
                .bind(heartbeat_time)
                .bind(WorkerStatus::Alive)
                .bind(current_task_count)
                .bind(worker_id)
                .execute(&self.pool)
                .await
                .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::worker_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success_worker(
            context,
            &format!("Worker '{}'", worker_id),
            Some(&format!("心跳更新，任务数: {}", current_task_count))
        );
        Ok(())
    }
    #[instrument(skip(self), fields(
        worker_id = %worker_id,
        status = ?status,
    ))]
    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()> {
        let context = worker_context!(RepositoryOperation::Update, worker_id = worker_id)
            .with_status(status)
            .with_additional_info(format!("状态变更为 {:?}", status));

        let result = sqlx::query("UPDATE workers SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(worker_id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryErrorHelpers::worker_not_found(context));
        }

        RepositoryErrorHelpers::log_operation_success_worker(
            context,
            &format!("Worker '{}'", worker_id),
            Some(&format!("状态更新为 {:?}", status))
        );
        Ok(())
    }
    #[instrument(skip(self), fields(timeout_seconds = %timeout_seconds))]
    async fn get_timeout_workers(&self, timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>> {
        let context = worker_context!(RepositoryOperation::Query)
            .with_status(WorkerStatus::Alive)
            .with_additional_info(format!("查询超时Worker，阈值: {}秒", timeout_seconds));

        let rows = sqlx::query(
            r#"
            SELECT id, hostname, ip_address::text as ip_address, supported_task_types, max_concurrent_tasks, current_task_count, status, last_heartbeat, registered_at 
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
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        let workers: SchedulerResult<Vec<WorkerInfo>> = rows.iter().map(Self::row_to_worker_info).collect();
        let result = workers?;
        
        debug!("查询超时Worker成功，阈值{}秒，返回 {} 个Worker", timeout_seconds, result.len());
        Ok(result)
    }
    #[instrument(skip(self), fields(timeout_seconds = %timeout_seconds))]
    async fn cleanup_offline_workers(&self, timeout_seconds: i64) -> SchedulerResult<u64> {
        let context = worker_context!(RepositoryOperation::Update)
            .with_additional_info(format!("清理离线Worker，阈值: {}秒", timeout_seconds));

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
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        let updated_count = result.rows_affected();
        
        RepositoryErrorHelpers::log_operation_success_worker(
            context,
            &format!("离线Worker清理操作"),
            Some(&format!("标记了 {} 个Worker为离线状态", updated_count))
        );
        
        Ok(updated_count)
    }
    #[instrument(skip(self))]
    async fn get_worker_load_stats(&self) -> SchedulerResult<Vec<WorkerLoadStats>> {
        let context = worker_context!(RepositoryOperation::Query)
            .with_status(WorkerStatus::Alive)
            .with_additional_info("查询Worker负载统计".to_string());

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
        .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

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

        debug!("查询Worker负载统计成功，返回 {} 个Worker的统计数据", stats.len());
        Ok(stats)
    }
    #[instrument(skip(self, worker_ids), fields(
        worker_count = %worker_ids.len(),
        target_status = ?status,
    ))]
    async fn batch_update_status(
        &self,
        worker_ids: &[String],
        status: WorkerStatus,
    ) -> SchedulerResult<()> {
        if worker_ids.is_empty() {
            debug!("批量更新Worker状态: Worker ID列表为空，跳过操作");
            return Ok(());
        }

        let context = worker_context!(RepositoryOperation::BatchUpdate)
            .with_status(status)
            .with_additional_info(format!("批量更新 {} 个Worker状态为 {:?}", worker_ids.len(), status));

        let result = sqlx::query("UPDATE workers SET status = $1 WHERE id = ANY($2)")
            .bind(status)
            .bind(worker_ids)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryErrorHelpers::worker_database_error(context.clone(), e))?;

        RepositoryErrorHelpers::log_operation_success_worker(
            context,
            &format!("批量Worker状态更新"),
            Some(&format!("更新了 {} 个Worker的状态为 {:?}", result.rows_affected(), status))
        );
        Ok(())
    }
}
