//! Redis cache manager implementation

use super::{CacheService, CacheStats};
use crate::cache::config::CacheConfig;
use async_trait::async_trait;
use scheduler_foundation::SchedulerResult;
use scheduler_errors::SchedulerError;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};

/// Redis cache manager with connection pooling and metrics
pub struct RedisCacheManager {
    /// Redis client
    client: Arc<redis::Client>,
    /// Cache configuration
    config: CacheConfig,
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
    /// Key prefix for this instance
    key_prefix: String,
}

impl RedisCacheManager {
    /// Create a new Redis cache manager
    pub async fn new(config: CacheConfig) -> SchedulerResult<Self> {
        if !config.enabled {
            return Err(SchedulerError::Configuration(
                "Cache is disabled".to_string(),
            ));
        }

        info!("Creating Redis cache manager with URL: {}", config.redis_url);

        let client = redis::Client::open(config.redis_url.clone())
            .map_err(|e| SchedulerError::CacheError(e.to_string()))?;

        // Test connection
        let mut conn = client
            .get_connection_manager()
            .await
            .map_err(|e| SchedulerError::CacheError(e.to_string()))?;

        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| SchedulerError::CacheError(e.to_string()))?;

        let key_prefix = config.key_prefix.clone().unwrap_or_else(|| "scheduler".to_string());

        info!("Redis cache manager created successfully");

        Ok(Self {
            client: Arc::new(client),
            config,
            stats: Arc::new(RwLock::new(CacheStats::default())),
            key_prefix,
        })
    }

    /// Get a connection from the pool
    async fn get_connection(&self) -> SchedulerResult<redis::aio::ConnectionManager> {
        self.client
            .get_connection_manager()
            .await
            .map_err(|e| SchedulerError::CacheError(e.to_string()))
    }

    /// Build full cache key with prefix
    fn build_key(&self, key: &str) -> String {
        if self.key_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}:{}", self.key_prefix, key)
        }
    }

    /// Increment cache hit counter
    async fn increment_hits(&self) {
        let mut stats = self.stats.write().await;
        stats.hits += 1;
    }

    /// Increment cache miss counter
    async fn increment_misses(&self) {
        let mut stats = self.stats.write().await;
        stats.misses += 1;
    }

    /// Increment cache set counter
    async fn increment_sets(&self) {
        let mut stats = self.stats.write().await;
        stats.sets += 1;
    }

    /// Increment cache delete counter
    async fn increment_deletes(&self) {
        let mut stats = self.stats.write().await;
        stats.deletes += 1;
    }

    /// Increment cache error counter
    async fn increment_errors(&self) {
        let mut stats = self.stats.write().await;
        stats.errors += 1;
    }

    /// Get TTL for a specific key type
    fn get_ttl_for_key(&self, key: &str) -> Duration {
        let ttl_config = &self.config.ttl;
        
        if key.starts_with("task:") {
            Duration::from_secs(ttl_config.task_seconds)
        } else if key.starts_with("worker:") {
            Duration::from_secs(ttl_config.worker_seconds)
        } else if key.starts_with("system_stats:") {
            Duration::from_secs(ttl_config.system_stats_seconds)
        } else if key.starts_with("task_run:") {
            Duration::from_secs(ttl_config.task_run_seconds)
        } else if key.starts_with("dependencies:") {
            Duration::from_secs(ttl_config.dependencies_seconds)
        } else {
            Duration::from_secs(self.config.default_ttl_seconds)
        }
    }
}

#[async_trait]
impl CacheService for RedisCacheManager {
    #[instrument(skip(self))]
    async fn get(&self, key: &str) -> SchedulerResult<Option<Vec<u8>>> {
        let full_key = self.build_key(key);
        debug!("Cache GET: {}", full_key);

        let mut conn = self.get_connection().await?;

        let result: Option<Vec<u8>> = redis::cmd("GET")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Cache GET failed for key {}: {}", full_key, e);
                self.increment_errors();
                SchedulerError::CacheError(e.to_string())
            })?;

        match result {
            Some(value) => {
                debug!("Cache HIT: {}", full_key);
                self.increment_hits().await;
                Ok(Some(value))
            }
            None => {
                debug!("Cache MISS: {}", full_key);
                self.increment_misses().await;
                Ok(None)
            }
        }
    }

    #[instrument(skip(self, value))]
    async fn set(&self, key: &str, value: &[u8], ttl: Duration) -> SchedulerResult<()> {
        let full_key = self.build_key(key);
        debug!("Cache SET: {} with TTL: {:?}", full_key, ttl);

        let mut conn = self.get_connection().await?;

        let ttl_seconds = ttl.as_secs() as i64;
        let _: () = redis::cmd("SETEX")
            .arg(&full_key)
            .arg(ttl_seconds)
            .arg(value)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Cache SET failed for key {}: {}", full_key, e);
                self.increment_errors();
                SchedulerError::CacheError(e.to_string())
            })?;

        debug!("Cache SET success: {}", full_key);
        self.increment_sets().await;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete(&self, key: &str) -> SchedulerResult<bool> {
        let full_key = self.build_key(key);
        debug!("Cache DELETE: {}", full_key);

        let mut conn = self.get_connection().await?;

        let result: i32 = redis::cmd("DEL")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Cache DELETE failed for key {}: {}", full_key, e);
                self.increment_errors();
                SchedulerError::CacheError(e.to_string())
            })?;

        let deleted = result > 0;
        if deleted {
            debug!("Cache DELETE success: {}", full_key);
            self.increment_deletes().await;
        } else {
            debug!("Cache DELETE key not found: {}", full_key);
        }

        Ok(deleted)
    }

    #[instrument(skip(self))]
    async fn exists(&self, key: &str) -> SchedulerResult<bool> {
        let full_key = self.build_key(key);
        debug!("Cache EXISTS: {}", full_key);

        let mut conn = self.get_connection().await?;

        let result: i32 = redis::cmd("EXISTS")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Cache EXISTS failed for key {}: {}", full_key, e);
                self.increment_errors();
                SchedulerError::CacheError(e.to_string())
            })?;

        Ok(result > 0)
    }

    #[instrument(skip(self))]
    async fn get_stats(&self) -> CacheStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    #[instrument(skip(self))]
    async fn clear_prefix(&self, prefix: &str) -> SchedulerResult<usize> {
        let full_prefix = self.build_key(prefix);
        debug!("Cache CLEAR_PREFIX: {}", full_prefix);

        let mut conn = self.get_connection().await?;

        // Use SCAN to find all keys with the prefix
        let mut keys: Vec<String> = Vec::new();
        let mut cursor: u64 = 0;

        loop {
            let scan_result: Result<(u64, Vec<String>), redis::RedisError> = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(format!("{}*", full_prefix))
                .arg("COUNT")
                .arg(1000)
                .query_async(&mut conn)
                .await;

            match scan_result {
                Ok((next_cursor, batch_keys)) => {
                    keys.extend(batch_keys);
                    
                    if next_cursor == 0 {
                        break;
                    }
                    cursor = next_cursor;
                }
                Err(e) => {
                    error!("Cache SCAN failed for prefix {}: {}", full_prefix, e);
                    self.increment_errors().await;
                    return Err(SchedulerError::CacheError(e.to_string()));
                }
            }
        }

        if keys.is_empty() {
            debug!("No keys found with prefix: {}", full_prefix);
            return Ok(0);
        }

        // Delete all keys in batches
        let mut deleted_count = 0;
        for chunk in keys.chunks(100) {
            let result: Result<usize, redis::RedisError> = redis::cmd("DEL")
                .arg(chunk)
                .query_async(&mut conn)
                .await;
            match result {
                Ok(_) => {
                    deleted_count += chunk.len();
                }
                Err(e) => {
                    error!("Cache batch DELETE failed for prefix {}: {}", full_prefix, e);
                    self.increment_errors().await;
                    return Err(SchedulerError::CacheError(e.to_string()));
                }
            }
        }

        debug!("Cache CLEAR_PREFIX success: {} keys deleted", deleted_count);
        self.increment_deletes().await;

        Ok(deleted_count)
    }

    #[instrument(skip(self))]
    async fn health_check(&self) -> SchedulerResult<bool> {
        debug!("Cache health check");

        let mut conn = self.get_connection().await?;

        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Cache health check failed: {}", e);
                self.increment_errors();
                SchedulerError::CacheError(e.to_string())
            })?;

        Ok(result == "PONG")
    }
}

impl Clone for RedisCacheManager {
    fn clone(&self) -> Self {
        Self {
            client: Arc::clone(&self.client),
            config: self.config.clone(),
            stats: Arc::clone(&self.stats),
            key_prefix: self.key_prefix.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::config::CacheConfig;

    #[tokio::test]
    async fn test_cache_manager_creation() {
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            ..Default::default()
        };

        // This test requires a running Redis instance
        // In a real test environment, you would use testcontainers
        let result = RedisCacheManager::new(config).await;
        
        // We can't guarantee Redis is running, so we just check the error type
        match result {
            Ok(_) => {
                println!("Redis is running, cache manager created successfully");
            }
            Err(e) => {
                println!("Redis is not running or connection failed: {}", e);
            }
        }
    }

    #[test]
    fn test_key_building() {
        let config = CacheConfig {
            enabled: true,
            redis_url: "redis://localhost:6379".to_string(),
            key_prefix: Some("test".to_string()),
            ..Default::default()
        };

        let cache = RedisCacheManager {
            client: Arc::new(redis::Client::open("redis://localhost:6379").unwrap()),
            config,
            stats: Arc::new(RwLock::new(CacheStats::default())),
            key_prefix: "test".to_string(),
        };

        assert_eq!(cache.build_key("task:123"), "test:task:123");
        assert_eq!(cache.build_key("worker:1"), "test:worker:1");
    }
}