use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_core::SchedulerResult;
use scheduler_domain::{
    entities::{WorkerInfo, WorkerStatus},
    repositories::{WorkerLoadStats, WorkerRepository},
};
use scheduler_errors::SchedulerError;
use sqlx::{Row, SqlitePool};
use tracing::debug;

use crate::database::mapping::MappingHelpers;

pub struct SqliteWorkerRepository {
    pool: SqlitePool,
}

impl SqliteWorkerRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    fn row_to_worker_info(row: &sqlx::sqlite::SqliteRow) -> SchedulerResult<WorkerInfo> {
        let supported_task_types = MappingHelpers::parse_supported_task_types_sqlite(row, "supported_task_types")?;

        Ok(WorkerInfo {
            id: row.try_get("id")?,
            hostname: row.try_get("hostname")?,
            ip_address: row.try_get("ip_address")?,
            supported_task_types,
            max_concurrent_tasks: row.try_get("max_concurrent_tasks")?,
            current_task_count: 0, // 需要从task_runs表计算
            status: row.try_get("status")?,
            last_heartbeat: row.try_get("last_heartbeat")?,
            registered_at: row.try_get("registered_at")?,
        })
    }
}

#[async_trait]
impl WorkerRepository for SqliteWorkerRepository {
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let supported_task_types_json = serde_json::to_string(&worker.supported_task_types)
            .map_err(|e| SchedulerError::Serialization(format!("序列化任务类型列表失败: {e}")))?;

        sqlx::query(
            r#"
            INSERT INTO workers (id, hostname, ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT(id) DO UPDATE SET
                hostname = excluded.hostname,
                ip_address = excluded.ip_address,
                supported_task_types = excluded.supported_task_types,
                max_concurrent_tasks = excluded.max_concurrent_tasks,
                status = excluded.status,
                last_heartbeat = excluded.last_heartbeat
            "#,
        )
        .bind(&worker.id)
        .bind(&worker.hostname)
        .bind(&worker.ip_address)
        .bind(supported_task_types_json)
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
    async fn unregister(&self, worker_id: &str) -> SchedulerResult<()> {
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
    async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
        let row = sqlx::query(
            "SELECT id, hostname, ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers WHERE id = $1"
        )
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match row {
            Some(row) => {
                let mut worker = Self::row_to_worker_info(&row)?;
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
    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let supported_task_types_json = serde_json::to_string(&worker.supported_task_types)
            .map_err(|e| SchedulerError::Serialization(format!("序列化任务类型列表失败: {e}")))?;

        let result = sqlx::query(
            r#"
            UPDATE workers 
            SET hostname = $2, ip_address = $3, supported_task_types = $4, 
                max_concurrent_tasks = $5, status = $6, last_heartbeat = $7
            WHERE id = $1
            "#,
        )
        .bind(&worker.id)
        .bind(&worker.hostname)
        .bind(&worker.ip_address)
        .bind(supported_task_types_json)
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
    async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        let rows = sqlx::query(
            "SELECT id, hostname, ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers ORDER BY registered_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut workers = Vec::new();
        for row in rows {
            let mut worker = Self::row_to_worker_info(&row)?;
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
    async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        let rows = sqlx::query(
            "SELECT id, hostname, ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers WHERE status = $1 ORDER BY last_heartbeat DESC"
        )
        .bind(WorkerStatus::Alive)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut workers = Vec::new();
        for row in rows {
            let mut worker = Self::row_to_worker_info(&row)?;
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
    async fn get_workers_by_task_type(&self, task_type: &str) -> SchedulerResult<Vec<WorkerInfo>> {
        let rows = sqlx::query(
            "SELECT id, hostname, ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers WHERE status = $1 AND supported_task_types LIKE $2 ORDER BY last_heartbeat DESC"
        )
        .bind(WorkerStatus::Alive)
        .bind(format!("%\"{task_type}\"% "))
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut workers = Vec::new();
        for row in rows {
            let mut worker = Self::row_to_worker_info(&row)?;
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
    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: DateTime<Utc>,
        current_task_count: i32,
    ) -> SchedulerResult<()> {
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
    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()> {
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
    async fn get_timeout_workers(&self, timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>> {
        let threshold = chrono::Utc::now() - chrono::Duration::seconds(timeout_seconds);

        let rows = sqlx::query(
            "SELECT id, hostname, ip_address, supported_task_types, max_concurrent_tasks, status, last_heartbeat, registered_at FROM workers WHERE status = $1 AND last_heartbeat < $2 ORDER BY last_heartbeat ASC"
        )
        .bind(WorkerStatus::Alive)
        .bind(threshold)
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut workers = Vec::new();
        for row in rows {
            let mut worker = Self::row_to_worker_info(&row)?;
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
    async fn cleanup_offline_workers(&self, timeout_seconds: i64) -> SchedulerResult<u64> {
        let threshold = chrono::Utc::now() - chrono::Duration::seconds(timeout_seconds);

        let result =
            sqlx::query("UPDATE workers SET status = $1 WHERE status = $2 AND last_heartbeat < $3")
                .bind(WorkerStatus::Down)
                .bind(WorkerStatus::Alive)
                .bind(threshold)
                .execute(&self.pool)
                .await
                .map_err(SchedulerError::Database)?;

        let updated_count = result.rows_affected();
        debug!("标记 {} 个Worker为离线状态", updated_count);
        Ok(updated_count)
    }
    async fn get_worker_load_stats(&self) -> SchedulerResult<Vec<WorkerLoadStats>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                w.id as worker_id,
                w.max_concurrent_tasks,
                w.last_heartbeat,
                COALESCE(running_tasks.count, 0) as current_task_count,
                COALESCE(completed_stats.total_completed, 0) as total_completed_tasks,
                COALESCE(failed_stats.total_failed, 0) as total_failed_tasks,
                COALESCE(duration_stats.avg_duration_ms, 0.0) as average_task_duration_ms
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
                    AVG((julianday(completed_at) - julianday(started_at)) * 86400000.0) as avg_duration_ms
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
            let average_task_duration_ms: f64 = row.try_get("average_task_duration_ms")?;
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
                average_task_duration_ms: if average_task_duration_ms > 0.0 {
                    Some(average_task_duration_ms)
                } else {
                    None
                },
                last_heartbeat,
            });
        }

        Ok(stats)
    }
    async fn batch_update_status(
        &self,
        worker_ids: &[String],
        status: WorkerStatus,
    ) -> SchedulerResult<()> {
        if worker_ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<String> = (0..worker_ids.len())
            .map(|i| format!("${}", i + 2))
            .collect();
        let query = format!(
            "UPDATE workers SET status = $1 WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut sqlx_query = sqlx::query(&query).bind(status);
        for worker_id in worker_ids {
            sqlx_query = sqlx_query.bind(worker_id);
        }

        let result = sqlx_query
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use scheduler_domain::entities::{WorkerInfo, WorkerStatus};
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::query(
            r#"
            CREATE TABLE workers (
                id TEXT PRIMARY KEY,
                hostname TEXT NOT NULL,
                ip_address TEXT NOT NULL,
                supported_task_types TEXT NOT NULL,
                max_concurrent_tasks INTEGER NOT NULL,
                status TEXT NOT NULL,
                last_heartbeat DATETIME NOT NULL,
                registered_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE task_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                status TEXT NOT NULL,
                worker_id TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                shard_index INTEGER,
                shard_total INTEGER,
                scheduled_at DATETIME NOT NULL,
                started_at DATETIME,
                completed_at DATETIME,
                result TEXT,
                error_message TEXT,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    fn create_test_worker() -> WorkerInfo {
        WorkerInfo {
            id: "test-worker-1".to_string(),
            hostname: "test-host".to_string(),
            ip_address: "127.0.0.1".to_string(),
            supported_task_types: vec!["shell".to_string(), "http".to_string()],
            max_concurrent_tasks: 10,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_register_worker() {
        let pool = setup_test_db().await;
        let repo = SqliteWorkerRepository::new(pool);
        let worker = create_test_worker();

        let result = repo.register(&worker).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_worker_by_id() {
        let pool = setup_test_db().await;
        let repo = SqliteWorkerRepository::new(pool);
        let worker = create_test_worker();
        repo.register(&worker).await.unwrap();
        let result = repo.get_by_id(&worker.id).await.unwrap();
        assert!(result.is_some());
        let retrieved_worker = result.unwrap();
        assert_eq!(retrieved_worker.id, worker.id);
        assert_eq!(retrieved_worker.hostname, worker.hostname);
    }

    #[tokio::test]
    async fn test_update_worker_status() {
        let pool = setup_test_db().await;
        let repo = SqliteWorkerRepository::new(pool);
        let worker = create_test_worker();
        repo.register(&worker).await.unwrap();
        let result = repo.update_status(&worker.id, WorkerStatus::Down).await;
        assert!(result.is_ok());
        let updated_worker = repo.get_by_id(&worker.id).await.unwrap().unwrap();
        assert_eq!(updated_worker.status, WorkerStatus::Down);
    }

    #[tokio::test]
    async fn test_list_workers() {
        let pool = setup_test_db().await;
        let repo = SqliteWorkerRepository::new(pool);
        let worker1 = create_test_worker();
        let mut worker2 = create_test_worker();
        worker2.id = "test-worker-2".to_string();
        repo.register(&worker1).await.unwrap();
        repo.register(&worker2).await.unwrap();
        let workers = repo.list().await.unwrap();
        assert_eq!(workers.len(), 2);
    }

    #[tokio::test]
    async fn test_unregister_worker() {
        let pool = setup_test_db().await;
        let repo = SqliteWorkerRepository::new(pool);
        let worker = create_test_worker();
        repo.register(&worker).await.unwrap();
        let result = repo.unregister(&worker.id).await;
        assert!(result.is_ok());
        let retrieved = repo.get_by_id(&worker.id).await.unwrap();
        assert!(retrieved.is_none());
    }
}
