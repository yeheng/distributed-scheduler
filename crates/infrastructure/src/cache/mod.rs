//! Redis-based caching infrastructure for the scheduler system
//!
//! This module provides a high-performance caching layer for frequently accessed data
//! such as task definitions, worker status, and system metrics.

pub mod config;
pub mod factory;
pub mod invalidation;
pub mod manager;
pub mod metrics;
pub mod repository;
pub mod warming;

use async_trait::async_trait;
pub use config::*;
pub use factory::*;
pub use invalidation::*;
pub use manager::*;
pub use metrics::*;
pub use repository::*;
pub use warming::*;

use scheduler_errors::SchedulerError;
use scheduler_foundation::SchedulerResult;

/// Cache key prefix patterns for different data types
#[derive(Debug, Clone, PartialEq)]
pub enum CachePrefix {
    Task,
    Worker,
    SystemStats,
    TaskRun,
    Dependencies,
}

impl CachePrefix {
    pub fn as_str(&self) -> &'static str {
        match self {
            CachePrefix::Task => "task",
            CachePrefix::Worker => "worker",
            CachePrefix::SystemStats => "system_stats",
            CachePrefix::TaskRun => "task_run",
            CachePrefix::Dependencies => "dependencies",
        }
    }
}

/// Cache TTL (Time To Live) configurations
#[derive(Debug, Clone)]
pub struct CacheTtl {
    /// Task definitions cache TTL (5 minutes)
    pub task: std::time::Duration,
    /// Worker status cache TTL (30 seconds)
    pub worker: std::time::Duration,
    /// System stats cache TTL (1 minute)
    pub system_stats: std::time::Duration,
    /// Task run cache TTL (10 minutes)
    pub task_run: std::time::Duration,
    /// Dependencies cache TTL (2 minutes)
    pub dependencies: std::time::Duration,
}

impl Default for CacheTtl {
    fn default() -> Self {
        Self {
            task: std::time::Duration::from_secs(300),
            worker: std::time::Duration::from_secs(30),
            system_stats: std::time::Duration::from_secs(60),
            task_run: std::time::Duration::from_secs(600),
            dependencies: std::time::Duration::from_secs(120),
        }
    }
}

/// Cache statistics and metrics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub deletes: u64,
    pub errors: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }

    pub fn error_rate(&self) -> f64 {
        let total_ops = self.hits + self.misses + self.sets + self.deletes;
        if total_ops == 0 {
            0.0
        } else {
            self.errors as f64 / total_ops as f64
        }
    }
}

/// Cache service trait for dependency injection
#[async_trait]
pub trait CacheService: Send + Sync {
    /// Get a value from cache as raw bytes
    async fn get(&self, key: &str) -> SchedulerResult<Option<Vec<u8>>>;

    /// Set a value in cache with TTL
    async fn set(&self, key: &str, value: &[u8], ttl: std::time::Duration) -> SchedulerResult<()>;

    /// Delete a value from cache
    async fn delete(&self, key: &str) -> SchedulerResult<bool>;

    /// Check if a key exists in cache
    async fn exists(&self, key: &str) -> SchedulerResult<bool>;

    /// Get cache statistics
    async fn get_stats(&self) -> CacheStats;

    /// Clear all cache entries with a specific prefix
    async fn clear_prefix(&self, prefix: &str) -> SchedulerResult<usize>;

    /// Health check for cache service
    async fn health_check(&self) -> SchedulerResult<bool>;
}

/// Extension trait for convenient type-safe caching
#[async_trait]
pub trait CacheServiceExt: Send + Sync {
    /// Get a typed value from cache
    async fn get_typed<T>(&self, key: &str) -> SchedulerResult<Option<T>>
    where
        T: serde::de::DeserializeOwned + Send + Sync;

    /// Set a typed value in cache with TTL
    async fn set_typed<T>(
        &self,
        key: &str,
        value: &T,
        ttl: std::time::Duration,
    ) -> SchedulerResult<()>
    where
        T: serde::Serialize + Send + Sync;
}

#[async_trait]
impl<T: CacheService + ?Sized> CacheServiceExt for T {
    async fn get_typed<U>(&self, key: &str) -> SchedulerResult<Option<U>>
    where
        U: serde::de::DeserializeOwned + Send + Sync,
    {
        match self.get(key).await? {
            Some(bytes) => {
                let value = serde_json::from_slice(&bytes)
                    .map_err(|e| SchedulerError::CacheError(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set_typed<U>(
        &self,
        key: &str,
        value: &U,
        ttl: std::time::Duration,
    ) -> SchedulerResult<()>
    where
        U: serde::Serialize + Send + Sync,
    {
        let bytes =
            serde_json::to_vec(value).map_err(|e| SchedulerError::CacheError(e.to_string()))?;
        self.set(key, &bytes, ttl).await
    }
}

/// Build cache key with prefix
pub fn build_cache_key(prefix: CachePrefix, id: &str) -> String {
    format!("{}:{}", prefix.as_str(), id)
}

/// Build cache key with multiple segments
pub fn build_cache_key_multi(prefix: CachePrefix, segments: &[&str]) -> String {
    let key = segments.join(":");
    format!("{}:{}", prefix.as_str(), key)
}

/// Generate cache key for task
pub fn task_cache_key(task_id: i64) -> String {
    build_cache_key(CachePrefix::Task, &task_id.to_string())
}

/// Generate cache key for task by name
pub fn task_name_cache_key(name: &str) -> String {
    build_cache_key(CachePrefix::Task, name)
}

/// Generate cache key for worker
pub fn worker_cache_key(worker_id: &str) -> String {
    build_cache_key(CachePrefix::Worker, worker_id)
}

/// Generate cache key for system stats
pub fn system_stats_cache_key() -> String {
    build_cache_key(CachePrefix::SystemStats, "current")
}

/// Generate cache key for task run
pub fn task_run_cache_key(run_id: i64) -> String {
    build_cache_key(CachePrefix::TaskRun, &run_id.to_string())
}

/// Generate cache key for task dependencies
pub fn task_dependencies_cache_key(task_id: i64) -> String {
    build_cache_key(CachePrefix::Dependencies, &task_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_prefix_display() {
        assert_eq!(CachePrefix::Task.as_str(), "task");
        assert_eq!(CachePrefix::Worker.as_str(), "worker");
        assert_eq!(CachePrefix::SystemStats.as_str(), "system_stats");
    }

    #[test]
    fn test_cache_key_building() {
        assert_eq!(task_cache_key(123), "task:123");
        assert_eq!(worker_cache_key("worker-1"), "worker:worker-1");
        assert_eq!(system_stats_cache_key(), "system_stats:current");
        assert_eq!(
            build_cache_key_multi(CachePrefix::Task, &["list", "active"]),
            "task:list:active"
        );
    }

    #[test]
    fn test_cache_stats() {
        let mut stats = CacheStats::default();
        stats.hits = 80;
        stats.misses = 20;
        stats.errors = 2;

        assert!((stats.hit_rate() - 0.8).abs() < f64::EPSILON);
        assert!((stats.miss_rate() - 0.2).abs() < f64::EPSILON);
        assert!((stats.error_rate() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_ttl_default() {
        let ttl = CacheTtl::default();
        assert_eq!(ttl.task, std::time::Duration::from_secs(300));
        assert_eq!(ttl.worker, std::time::Duration::from_secs(30));
        assert_eq!(ttl.system_stats, std::time::Duration::from_secs(60));
    }
}
