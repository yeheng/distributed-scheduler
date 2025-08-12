//! Circuit breaker wrapper for database and message queue operations
//!
//! This module provides circuit breaker integration for external service calls
//! to improve system resilience and prevent cascading failures.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scheduler_core::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerMiddleware},
    SchedulerResult,
};
use scheduler_domain::{
    entities::{Task, TaskFilter, TaskRun, TaskRunStatus, TaskStatus, WorkerInfo, WorkerStatus},
    repositories::{TaskExecutionStats, TaskRepository, TaskRunRepository, WorkerLoadStats, WorkerRepository},
};

/// Circuit breaker wrapper for TaskRepository
pub struct CircuitBreakerTaskRepository {
    inner: Arc<dyn TaskRepository + Send + Sync>,
    circuit_breaker: CircuitBreakerMiddleware,
}

impl CircuitBreakerTaskRepository {
    pub fn new(
        inner: Arc<dyn TaskRepository + Send + Sync>,
        circuit_breaker_config: Option<CircuitBreakerConfig>,
    ) -> Self {
        let config = circuit_breaker_config.unwrap_or_default();
        let circuit_breaker = Arc::new(CircuitBreaker::with_config(config));
        let middleware = CircuitBreakerMiddleware::new(circuit_breaker, "TaskRepository".to_string());

        Self {
            inner,
            circuit_breaker: middleware,
        }
    }
}

#[async_trait]
impl TaskRepository for CircuitBreakerTaskRepository {
    async fn create(&self, task: &Task) -> SchedulerResult<Task> {
        self.circuit_breaker
            .execute("create", || {
                let inner = self.inner.clone();
                let task = task.clone();
                async move { inner.create(&task).await }
            })
            .await
    }

    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
        self.circuit_breaker
            .execute("get_by_id", || {
                let inner = self.inner.clone();
                async move { inner.get_by_id(id).await }
            })
            .await
    }

    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>> {
        self.circuit_breaker
            .execute("get_by_name", || {
                let inner = self.inner.clone();
                let name = name.to_string();
                async move { inner.get_by_name(&name).await }
            })
            .await
    }

    async fn update(&self, task: &Task) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("update", || {
                let inner = self.inner.clone();
                let task = task.clone();
                async move { inner.update(&task).await }
            })
            .await
    }

    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("delete", || {
                let inner = self.inner.clone();
                async move { inner.delete(id).await }
            })
            .await
    }

    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
        self.circuit_breaker
            .execute("list", || {
                let inner = self.inner.clone();
                let filter = filter.clone();
                async move { inner.list(&filter).await }
            })
            .await
    }

    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
        self.circuit_breaker
            .execute("get_active_tasks", || {
                let inner = self.inner.clone();
                async move { inner.get_active_tasks().await }
            })
            .await
    }

    async fn get_schedulable_tasks(&self, current_time: DateTime<Utc>) -> SchedulerResult<Vec<Task>> {
        self.circuit_breaker
            .execute("get_schedulable_tasks", || {
                let inner = self.inner.clone();
                async move { inner.get_schedulable_tasks(current_time).await }
            })
            .await
    }

    async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool> {
        self.circuit_breaker
            .execute("check_dependencies", || {
                let inner = self.inner.clone();
                async move { inner.check_dependencies(task_id).await }
            })
            .await
    }

    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        self.circuit_breaker
            .execute("get_dependencies", || {
                let inner = self.inner.clone();
                async move { inner.get_dependencies(task_id).await }
            })
            .await
    }

    async fn batch_update_status(&self, task_ids: &[i64], status: TaskStatus) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("batch_update_status", || {
                let inner = self.inner.clone();
                let task_ids = task_ids.to_vec();
                async move { inner.batch_update_status(&task_ids, status).await }
            })
            .await
    }
}

/// Circuit breaker wrapper for TaskRunRepository
pub struct CircuitBreakerTaskRunRepository {
    inner: Arc<dyn TaskRunRepository + Send + Sync>,
    circuit_breaker: CircuitBreakerMiddleware,
}

impl CircuitBreakerTaskRunRepository {
    pub fn new(
        inner: Arc<dyn TaskRunRepository + Send + Sync>,
        circuit_breaker_config: Option<CircuitBreakerConfig>,
    ) -> Self {
        let config = circuit_breaker_config.unwrap_or_default();
        let circuit_breaker = Arc::new(CircuitBreaker::with_config(config));
        let middleware = CircuitBreakerMiddleware::new(circuit_breaker, "TaskRunRepository".to_string());

        Self {
            inner,
            circuit_breaker: middleware,
        }
    }
}

#[async_trait]
impl TaskRunRepository for CircuitBreakerTaskRunRepository {
    async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun> {
        self.circuit_breaker
            .execute("create", || {
                let inner = self.inner.clone();
                let task_run = task_run.clone();
                async move { inner.create(&task_run).await }
            })
            .await
    }

    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>> {
        self.circuit_breaker
            .execute("get_by_id", || {
                let inner = self.inner.clone();
                async move { inner.get_by_id(id).await }
            })
            .await
    }

    async fn update(&self, task_run: &TaskRun) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("update", || {
                let inner = self.inner.clone();
                let task_run = task_run.clone();
                async move { inner.update(&task_run).await }
            })
            .await
    }

    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("delete", || {
                let inner = self.inner.clone();
                async move { inner.delete(id).await }
            })
            .await
    }

    async fn get_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>> {
        self.circuit_breaker
            .execute("get_by_task_id", || {
                let inner = self.inner.clone();
                async move { inner.get_by_task_id(task_id).await }
            })
            .await
    }

    async fn get_by_worker_id(&self, worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
        self.circuit_breaker
            .execute("get_by_worker_id", || {
                let inner = self.inner.clone();
                let worker_id = worker_id.to_string();
                async move { inner.get_by_worker_id(&worker_id).await }
            })
            .await
    }

    async fn get_by_status(&self, status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>> {
        self.circuit_breaker
            .execute("get_by_status", || {
                let inner = self.inner.clone();
                async move { inner.get_by_status(status).await }
            })
            .await
    }

    async fn get_pending_runs(&self, limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>> {
        self.circuit_breaker
            .execute("get_pending_runs", || {
                let inner = self.inner.clone();
                async move { inner.get_pending_runs(limit).await }
            })
            .await
    }

    async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>> {
        self.circuit_breaker
            .execute("get_running_runs", || {
                let inner = self.inner.clone();
                async move { inner.get_running_runs().await }
            })
            .await
    }

    async fn get_timeout_runs(&self, timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>> {
        self.circuit_breaker
            .execute("get_timeout_runs", || {
                let inner = self.inner.clone();
                async move { inner.get_timeout_runs(timeout_seconds).await }
            })
            .await
    }

    async fn update_status(
        &self,
        id: i64,
        status: TaskRunStatus,
        worker_id: Option<&str>,
    ) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("update_status", || {
                let inner = self.inner.clone();
                let worker_id = worker_id.map(|s| s.to_string());
                async move {
                    inner
                        .update_status(id, status, worker_id.as_deref())
                        .await
                }
            })
            .await
    }

    async fn update_result(
        &self,
        id: i64,
        result: Option<&str>,
        error_message: Option<&str>,
    ) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("update_result", || {
                let inner = self.inner.clone();
                let result = result.map(|s| s.to_string());
                let error_message = error_message.map(|s| s.to_string());
                async move {
                    inner
                        .update_result(id, result.as_deref(), error_message.as_deref())
                        .await
                }
            })
            .await
    }

    async fn get_recent_runs(&self, task_id: i64, limit: i64) -> SchedulerResult<Vec<TaskRun>> {
        self.circuit_breaker
            .execute("get_recent_runs", || {
                let inner = self.inner.clone();
                async move { inner.get_recent_runs(task_id, limit).await }
            })
            .await
    }

    async fn get_execution_stats(&self, task_id: i64, days: i32) -> SchedulerResult<TaskExecutionStats> {
        self.circuit_breaker
            .execute("get_execution_stats", || {
                let inner = self.inner.clone();
                async move { inner.get_execution_stats(task_id, days).await }
            })
            .await
    }

    async fn cleanup_old_runs(&self, days: i32) -> SchedulerResult<u64> {
        self.circuit_breaker
            .execute("cleanup_old_runs", || {
                let inner = self.inner.clone();
                async move { inner.cleanup_old_runs(days).await }
            })
            .await
    }

    async fn batch_update_status(&self, run_ids: &[i64], status: TaskRunStatus) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("batch_update_status", || {
                let inner = self.inner.clone();
                let run_ids = run_ids.to_vec();
                async move { inner.batch_update_status(&run_ids, status).await }
            })
            .await
    }
}

/// Circuit breaker wrapper for WorkerRepository
pub struct CircuitBreakerWorkerRepository {
    inner: Arc<dyn WorkerRepository + Send + Sync>,
    circuit_breaker: CircuitBreakerMiddleware,
}

impl CircuitBreakerWorkerRepository {
    pub fn new(
        inner: Arc<dyn WorkerRepository + Send + Sync>,
        circuit_breaker_config: Option<CircuitBreakerConfig>,
    ) -> Self {
        let config = circuit_breaker_config.unwrap_or_default();
        let circuit_breaker = Arc::new(CircuitBreaker::with_config(config));
        let middleware = CircuitBreakerMiddleware::new(circuit_breaker, "WorkerRepository".to_string());

        Self {
            inner,
            circuit_breaker: middleware,
        }
    }
}

#[async_trait]
impl WorkerRepository for CircuitBreakerWorkerRepository {
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("register", || {
                let inner = self.inner.clone();
                let worker = worker.clone();
                async move { inner.register(&worker).await }
            })
            .await
    }

    async fn unregister(&self, worker_id: &str) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("unregister", || {
                let inner = self.inner.clone();
                let worker_id = worker_id.to_string();
                async move { inner.unregister(&worker_id).await }
            })
            .await
    }

    async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
        self.circuit_breaker
            .execute("get_by_id", || {
                let inner = self.inner.clone();
                let worker_id = worker_id.to_string();
                async move { inner.get_by_id(&worker_id).await }
            })
            .await
    }

    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("update", || {
                let inner = self.inner.clone();
                let worker = worker.clone();
                async move { inner.update(&worker).await }
            })
            .await
    }

    async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        self.circuit_breaker
            .execute("list", || {
                let inner = self.inner.clone();
                async move { inner.list().await }
            })
            .await
    }

    async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        self.circuit_breaker
            .execute("get_alive_workers", || {
                let inner = self.inner.clone();
                async move { inner.get_alive_workers().await }
            })
            .await
    }

    async fn get_workers_by_task_type(&self, task_type: &str) -> SchedulerResult<Vec<WorkerInfo>> {
        self.circuit_breaker
            .execute("get_workers_by_task_type", || {
                let inner = self.inner.clone();
                let task_type = task_type.to_string();
                async move { inner.get_workers_by_task_type(&task_type).await }
            })
            .await
    }

    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: DateTime<Utc>,
        current_task_count: i32,
    ) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("update_heartbeat", || {
                let inner = self.inner.clone();
                let worker_id = worker_id.to_string();
                async move {
                    inner
                        .update_heartbeat(&worker_id, heartbeat_time, current_task_count)
                        .await
                }
            })
            .await
    }

    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("update_status", || {
                let inner = self.inner.clone();
                let worker_id = worker_id.to_string();
                async move { inner.update_status(&worker_id, status).await }
            })
            .await
    }

    async fn get_timeout_workers(&self, timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>> {
        self.circuit_breaker
            .execute("get_timeout_workers", || {
                let inner = self.inner.clone();
                async move { inner.get_timeout_workers(timeout_seconds).await }
            })
            .await
    }

    async fn cleanup_offline_workers(&self, timeout_seconds: i64) -> SchedulerResult<u64> {
        self.circuit_breaker
            .execute("cleanup_offline_workers", || {
                let inner = self.inner.clone();
                async move { inner.cleanup_offline_workers(timeout_seconds).await }
            })
            .await
    }

    async fn get_worker_load_stats(&self) -> SchedulerResult<Vec<WorkerLoadStats>> {
        self.circuit_breaker
            .execute("get_worker_load_stats", || {
                let inner = self.inner.clone();
                async move { inner.get_worker_load_stats().await }
            })
            .await
    }

    async fn batch_update_status(&self, worker_ids: &[String], status: WorkerStatus) -> SchedulerResult<()> {
        self.circuit_breaker
            .execute("batch_update_status", || {
                let inner = self.inner.clone();
                let worker_ids = worker_ids.to_vec();
                async move { inner.batch_update_status(&worker_ids, status).await }
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Mock repository for testing
    struct MockTaskRepository;

    #[async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn create(&self, _task: &Task) -> SchedulerResult<Task> {
            // Simulate network delay
            tokio::time::sleep(Duration::from_millis(10)).await;
            Err(scheduler_core::SchedulerError::Internal("Mock database error".to_string()))
        }

        async fn get_by_id(&self, _id: i64) -> SchedulerResult<Option<Task>> {
            Ok(None)
        }

        async fn get_by_name(&self, _name: &str) -> SchedulerResult<Option<Task>> {
            Ok(None)
        }

        async fn update(&self, _task: &Task) -> SchedulerResult<()> {
            Ok(())
        }

        async fn delete(&self, _id: i64) -> SchedulerResult<()> {
            Ok(())
        }

        async fn list(&self, _filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
            Ok(vec![])
        }

        async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
            Ok(vec![])
        }

        async fn get_schedulable_tasks(&self, _current_time: DateTime<Utc>) -> SchedulerResult<Vec<Task>> {
            Ok(vec![])
        }

        async fn check_dependencies(&self, _task_id: i64) -> SchedulerResult<bool> {
            Ok(true)
        }

        async fn get_dependencies(&self, _task_id: i64) -> SchedulerResult<Vec<Task>> {
            Ok(vec![])
        }

        async fn batch_update_status(&self, _task_ids: &[i64], _status: TaskStatus) -> SchedulerResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_task_repository() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        
        let mock_repo = Arc::new(MockTaskRepository);
        let cb_repo = CircuitBreakerTaskRepository::new(mock_repo, Some(config));

        // Create a dummy task
        let task = Task {
            id: 1,
            name: "test".to_string(),
            task_type: "test".to_string(),
            schedule: "0 0 * * *".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 30,
            max_retries: 3,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // First call should fail
        let result1 = cb_repo.create(&task).await;
        assert!(result1.is_err());

        // Second call should fail and trigger circuit breaker
        let result2 = cb_repo.create(&task).await;
        assert!(result2.is_err());

        // Third call should be blocked by circuit breaker
        let result3 = cb_repo.create(&task).await;
        assert!(result3.is_err());
        assert!(result3.unwrap_err().to_string().contains("Circuit breaker"));
    }
}