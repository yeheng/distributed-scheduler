//! Cached repository implementations for performance optimization

use super::{CacheService, CacheServiceExt, task_cache_key, task_name_cache_key, worker_cache_key, system_stats_cache_key, task_run_cache_key, task_dependencies_cache_key};
use async_trait::async_trait;
use scheduler_core::SchedulerResult;
use scheduler_domain::entities::*;
use scheduler_domain::repositories::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

/// Cached task repository wrapper
pub struct CachedTaskRepository {
    /// Inner repository implementation
    inner: Arc<dyn TaskRepository>,
    /// Cache service
    cache: Arc<dyn CacheService>,
    /// Cache TTL for tasks
    task_ttl: Duration,
}

impl CachedTaskRepository {
    pub fn new(
        inner: Arc<dyn TaskRepository>,
        cache: Arc<dyn CacheService>,
        task_ttl: Duration,
    ) -> Self {
        Self {
            inner,
            cache,
            task_ttl,
        }
    }

    /// Invalidate cache for a specific task
    async fn invalidate_task_cache(&self, task_id: i64) -> SchedulerResult<()> {
        let cache_key = task_cache_key(task_id);
        self.cache.delete(&cache_key).await?;
        Ok(())
    }

    /// Invalidate cache for task by name
    async fn invalidate_task_name_cache(&self, name: &str) -> SchedulerResult<()> {
        let cache_key = task_name_cache_key(name);
        self.cache.delete(&cache_key).await?;
        Ok(())
    }

    /// Invalidate all task-related cache
    async fn invalidate_all_task_cache(&self) -> SchedulerResult<()> {
        self.cache.clear_prefix("task").await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl TaskRepository for CachedTaskRepository {
    #[instrument(skip(self, task))]
    async fn create(&self, task: &Task) -> SchedulerResult<Task> {
        let result = self.inner.create(task).await?;
        
        // Cache the newly created task
        let cache_key = task_cache_key(result.id);
        self.cache.set_typed(&cache_key, &result, self.task_ttl).await?;
        
        // Also cache by name if it has one
        if !result.name.is_empty() {
            let name_cache_key = task_name_cache_key(&result.name);
            self.cache.set_typed(&name_cache_key, &result, self.task_ttl).await?;
        }
        
        Ok(result)
    }

    #[instrument(skip(self))]
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
        let cache_key = task_cache_key(id);
        
        // Try cache first
        if let Some(cached_task) = self.cache.get_typed::<Task>(&cache_key).await? {
            debug!("Cache hit for task ID: {}", id);
            return Ok(Some(cached_task));
        }
        
        // Cache miss, get from database
        debug!("Cache miss for task ID: {}", id);
        let task = self.inner.get_by_id(id).await?;
        
        if let Some(ref task) = task {
            // Cache the result
            self.cache.set_typed(&cache_key, task, self.task_ttl).await?;
            
            // Also cache by name if it has one
            if !task.name.is_empty() {
                let name_cache_key = task_name_cache_key(&task.name);
                self.cache.set_typed(&name_cache_key, task, self.task_ttl).await?;
            }
        }
        
        Ok(task)
    }

    #[instrument(skip(self))]
    async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>> {
        let cache_key = task_name_cache_key(name);
        
        // Try cache first
        if let Some(cached_task) = self.cache.get_typed::<Task>(&cache_key).await? {
            debug!("Cache hit for task name: {}", name);
            return Ok(Some(cached_task));
        }
        
        // Cache miss, get from database
        debug!("Cache miss for task name: {}", name);
        let task = self.inner.get_by_name(name).await?;
        
        if let Some(ref task) = task {
            // Cache the result
            self.cache.set_typed(&cache_key, task, self.task_ttl).await?;
            
            // Also cache by ID
            let id_cache_key = task_cache_key(task.id);
            self.cache.set_typed(&id_cache_key, task, self.task_ttl).await?;
        }
        
        Ok(task)
    }

    #[instrument(skip(self, task))]
    async fn update(&self, task: &Task) -> SchedulerResult<()> {
        let result = self.inner.update(task).await?;
        
        // Invalidate cache for this task
        self.invalidate_task_cache(task.id).await?;
        self.invalidate_task_name_cache(&task.name).await?;
        
        Ok(result)
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: i64) -> SchedulerResult<()> {
        // Get task info before deletion for cache invalidation
        if let Some(task) = self.inner.get_by_id(id).await? {
            let result = self.inner.delete(id).await?;
            
            // Invalidate cache
            self.invalidate_task_cache(task.id).await?;
            self.invalidate_task_name_cache(&task.name).await?;
            
            Ok(result)
        } else {
            self.inner.delete(id).await
        }
    }

    #[instrument(skip(self, filter))]
    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
        // For list operations, we don't cache individual filter results
        // as they can be too varied. This could be enhanced with more
        // sophisticated caching strategies in the future.
        self.inner.list(filter).await
    }

    #[instrument(skip(self))]
    async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
        // Cache active tasks list for a short duration
        let cache_key = task_cache_key(-1); // Use -1 for active tasks list
        
        if let Some(cached_tasks) = self.cache.get_typed::<Vec<Task>>(&cache_key).await? {
            debug!("Cache hit for active tasks");
            return Ok(cached_tasks);
        }
        
        debug!("Cache miss for active tasks");
        let tasks = self.inner.get_active_tasks().await?;
        
        // Cache for shorter duration (30 seconds)
        self.cache.set_typed(&cache_key, &tasks, Duration::from_secs(30)).await?;
        
        Ok(tasks)
    }

    #[instrument(skip(self))]
    async fn get_schedulable_tasks(&self, current_time: chrono::DateTime<chrono::Utc>) -> SchedulerResult<Vec<Task>> {
        // Schedulable tasks are time-sensitive, so we cache them for a very short duration
        let cache_key = format!("schedulable:{}", current_time.timestamp());
        
        if let Some(cached_tasks) = self.cache.get_typed::<Vec<Task>>(&cache_key).await? {
            debug!("Cache hit for schedulable tasks at {}", current_time);
            return Ok(cached_tasks);
        }
        
        debug!("Cache miss for schedulable tasks at {}", current_time);
        let tasks = self.inner.get_schedulable_tasks(current_time).await?;
        
        // Cache for 5 seconds only
        self.cache.set_typed(&cache_key, &tasks, Duration::from_secs(5)).await?;
        
        Ok(tasks)
    }

    #[instrument(skip(self))]
    async fn check_dependencies(&self, task_id: i64) -> SchedulerResult<bool> {
        let cache_key = task_dependencies_cache_key(task_id);
        
        if let Some(cached_result) = self.cache.get_typed::<bool>(&cache_key).await? {
            debug!("Cache hit for task {} dependencies", task_id);
            return Ok(cached_result);
        }
        
        debug!("Cache miss for task {} dependencies", task_id);
        let result = self.inner.check_dependencies(task_id).await?;
        
        // Cache dependencies for 2 minutes
        self.cache.set_typed(&cache_key, &result, Duration::from_secs(120)).await?;
        
        Ok(result)
    }

    #[instrument(skip(self))]
    async fn get_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<Task>> {
        let cache_key = format!("dependencies:list:{}", task_id);
        
        if let Some(cached_deps) = self.cache.get_typed::<Vec<Task>>(&cache_key).await? {
            debug!("Cache hit for task {} dependencies list", task_id);
            return Ok(cached_deps);
        }
        
        debug!("Cache miss for task {} dependencies list", task_id);
        let deps = self.inner.get_dependencies(task_id).await?;
        
        // Cache dependencies list for 2 minutes
        self.cache.set_typed(&cache_key, &deps, Duration::from_secs(120)).await?;
        
        Ok(deps)
    }

    #[instrument(skip(self, task_ids, status))]
    async fn batch_update_status(&self, task_ids: &[i64], status: TaskStatus) -> SchedulerResult<()> {
        let result = self.inner.batch_update_status(task_ids, status).await?;
        
        // Invalidate cache for all affected tasks
        for &task_id in task_ids {
            self.invalidate_task_cache(task_id).await?;
        }
        
        Ok(result)
    }
}

/// Cached worker repository wrapper
pub struct CachedWorkerRepository {
    inner: Arc<dyn WorkerRepository>,
    cache: Arc<dyn CacheService>,
    worker_ttl: Duration,
}

impl CachedWorkerRepository {
    pub fn new(
        inner: Arc<dyn WorkerRepository>,
        cache: Arc<dyn CacheService>,
        worker_ttl: Duration,
    ) -> Self {
        Self {
            inner,
            cache,
            worker_ttl,
        }
    }

    async fn invalidate_worker_cache(&self, worker_id: &str) -> SchedulerResult<()> {
        let cache_key = worker_cache_key(worker_id);
        self.cache.delete(&cache_key).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl WorkerRepository for CachedWorkerRepository {
    #[instrument(skip(self, worker))]
    async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let result = self.inner.register(worker).await?;
        
        // Cache the newly registered worker
        let cache_key = worker_cache_key(&worker.id);
        self.cache.set_typed(&cache_key, worker, self.worker_ttl).await?;
        
        Ok(result)
    }

    #[instrument(skip(self))]
    async fn unregister(&self, worker_id: &str) -> SchedulerResult<()> {
        let result = self.inner.unregister(worker_id).await?;
        
        // Remove from cache
        self.invalidate_worker_cache(worker_id).await?;
        
        Ok(result)
    }

    #[instrument(skip(self))]
    async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
        let cache_key = worker_cache_key(worker_id);
        
        if let Some(cached_worker) = self.cache.get_typed::<WorkerInfo>(&cache_key).await? {
            debug!("Cache hit for worker ID: {}", worker_id);
            return Ok(Some(cached_worker));
        }
        
        debug!("Cache miss for worker ID: {}", worker_id);
        let worker = self.inner.get_by_id(worker_id).await?;
        
        if let Some(ref worker) = worker {
            self.cache.set_typed(&cache_key, worker, self.worker_ttl).await?;
        }
        
        Ok(worker)
    }

    #[instrument(skip(self, worker))]
    async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        let result = self.inner.update(worker).await?;
        
        // Update cache
        let cache_key = worker_cache_key(&worker.id);
        self.cache.set_typed(&cache_key, worker, self.worker_ttl).await?;
        
        Ok(result)
    }

    #[instrument(skip(self))]
    async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        // Cache worker list for short duration (15 seconds)
        let cache_key = "workers:list";
        
        if let Some(cached_workers) = self.cache.get_typed::<Vec<WorkerInfo>>(cache_key).await? {
            debug!("Cache hit for workers list");
            return Ok(cached_workers);
        }
        
        debug!("Cache miss for workers list");
        let workers = self.inner.list().await?;
        
        self.cache.set_typed(cache_key, &workers, Duration::from_secs(15)).await?;
        
        Ok(workers)
    }

    #[instrument(skip(self))]
    async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        // Cache alive workers for very short duration (10 seconds)
        let cache_key = "workers:alive";
        
        if let Some(cached_workers) = self.cache.get_typed::<Vec<WorkerInfo>>(cache_key).await? {
            debug!("Cache hit for alive workers");
            return Ok(cached_workers);
        }
        
        debug!("Cache miss for alive workers");
        let workers = self.inner.get_alive_workers().await?;
        
        self.cache.set_typed(cache_key, &workers, Duration::from_secs(10)).await?;
        
        Ok(workers)
    }

    #[instrument(skip(self))]
    async fn update_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_time: chrono::DateTime<chrono::Utc>,
        current_task_count: i32,
    ) -> SchedulerResult<()> {
        let result = self.inner.update_heartbeat(worker_id, heartbeat_time, current_task_count).await?;
        
        // Update worker cache
        if let Some(mut worker) = self.get_by_id(worker_id).await? {
            worker.last_heartbeat = heartbeat_time;
            worker.current_task_count = current_task_count;
            let cache_key = worker_cache_key(worker_id);
            self.cache.set_typed(&cache_key, &worker, self.worker_ttl).await?;
        }
        
        Ok(result)
    }

    #[instrument(skip(self, worker_ids, status))]
    async fn batch_update_status(&self, worker_ids: &[String], status: WorkerStatus) -> SchedulerResult<()> {
        let result = self.inner.batch_update_status(worker_ids, status).await?;
        
        // Invalidate cache for all affected workers
        for worker_id in worker_ids {
            self.invalidate_worker_cache(worker_id).await?;
        }
        
        Ok(result)
    }

    // Other methods delegate to inner repository without caching
    async fn get_workers_by_task_type(&self, task_type: &str) -> SchedulerResult<Vec<WorkerInfo>> {
        self.inner.get_workers_by_task_type(task_type).await
    }

    async fn update_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()> {
        let result = self.inner.update_status(worker_id, status).await?;
        self.invalidate_worker_cache(worker_id).await?;
        Ok(result)
    }

    async fn get_timeout_workers(&self, timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>> {
        self.inner.get_timeout_workers(timeout_seconds).await
    }

    async fn cleanup_offline_workers(&self, timeout_seconds: i64) -> SchedulerResult<u64> {
        self.inner.cleanup_offline_workers(timeout_seconds).await
    }

    async fn get_worker_load_stats(&self) -> SchedulerResult<Vec<WorkerLoadStats>> {
        self.inner.get_worker_load_stats().await
    }
}