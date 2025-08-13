//! Cache factory for creating cached repository instances

use super::{
    config::CacheConfig, manager::RedisCacheManager, metrics::CacheMetrics, repository::*,
    CacheService,
};
use scheduler_foundation::SchedulerResult;
use scheduler_domain::repositories::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Cache factory for creating cached repository instances
pub struct CacheFactory {
    cache_service: Option<Arc<dyn CacheService>>,
    metrics: Option<CacheMetrics>,
    config: CacheConfig,
}

impl CacheFactory {
    /// Create new cache factory
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache_service: None,
            metrics: None,
            config,
        }
    }

    /// Initialize cache service and metrics
    pub async fn initialize(&mut self, registry: Option<&prometheus::Registry>) -> SchedulerResult<()> {
        if !self.config.enabled {
            info!("Cache is disabled, skipping initialization");
            return Ok(());
        }

        info!("Initializing cache service with Redis URL: {}", self.config.redis_url);

        // Create Redis cache manager
        let cache_manager = RedisCacheManager::new(self.config.clone()).await?;
        let cache_service = Arc::new(cache_manager) as Arc<dyn CacheService>;

        // Create metrics if registry is provided
        let metrics = if let Some(registry) = registry {
            Some(CacheMetrics::new(registry))
        } else {
            None
        };

        self.cache_service = Some(cache_service);
        self.metrics = metrics;

        info!("Cache service initialized successfully");
        Ok(())
    }

    /// Get the cache service
    pub fn cache_service(&self) -> Option<Arc<dyn CacheService>> {
        self.cache_service.clone()
    }

    /// Get the metrics
    pub fn metrics(&self) -> Option<&CacheMetrics> {
        self.metrics.as_ref()
    }

    /// Create cached task repository
    pub fn create_cached_task_repository(
        &self,
        inner: Arc<dyn TaskRepository>,
    ) -> Arc<dyn TaskRepository> {
        if let Some(ref cache_service) = self.cache_service {
            let ttl = self.config.ttl.to_cache_ttl().task;
            let cached_repo = CachedTaskRepository::new(inner, cache_service.clone(), ttl);
            Arc::new(cached_repo) as Arc<dyn TaskRepository>
        } else {
            inner
        }
    }

    /// Create cached worker repository
    pub fn create_cached_worker_repository(
        &self,
        inner: Arc<dyn WorkerRepository>,
    ) -> Arc<dyn WorkerRepository> {
        if let Some(ref cache_service) = self.cache_service {
            let ttl = self.config.ttl.to_cache_ttl().worker;
            let cached_repo = CachedWorkerRepository::new(inner, cache_service.clone(), ttl);
            Arc::new(cached_repo) as Arc<dyn WorkerRepository>
        } else {
            inner
        }
    }

    /// Create cached task run repository
    pub fn create_cached_task_run_repository(
        &self,
        inner: Arc<dyn TaskRunRepository>,
    ) -> Arc<dyn TaskRunRepository> {
        if let Some(ref cache_service) = self.cache_service {
            // TODO: Implement CachedTaskRunRepository
            inner
        } else {
            inner
        }
    }

    /// Check if cache is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }
}

/// Cache manager for handling cache lifecycle
pub struct CacheManager {
    factory: Arc<RwLock<CacheFactory>>,
    cache_service: Option<Arc<dyn CacheService>>,
}

impl CacheManager {
    /// Create new cache manager
    pub async fn new(config: CacheConfig, registry: Option<&prometheus::Registry>) -> SchedulerResult<Self> {
        let mut factory = CacheFactory::new(config);
        factory.initialize(registry).await?;

        let cache_service = factory.cache_service();

        Ok(Self {
            factory: Arc::new(RwLock::new(factory)),
            cache_service,
        })
    }

    /// Get cache service
    pub fn cache_service(&self) -> Option<Arc<dyn CacheService>> {
        self.cache_service.clone()
    }

    /// Get cache factory
    pub async fn factory(&self) -> Arc<RwLock<CacheFactory>> {
        self.factory.clone()
    }

    /// Create cached repositories
    pub async fn create_cached_repositories(
        &self,
        task_repo: Arc<dyn TaskRepository>,
        worker_repo: Arc<dyn WorkerRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
    ) -> (Arc<dyn TaskRepository>, Arc<dyn WorkerRepository>, Arc<dyn TaskRunRepository>) {
        let factory = self.factory.read().await;

        let cached_task_repo = factory.create_cached_task_repository(task_repo);
        let cached_worker_repo = factory.create_cached_worker_repository(worker_repo);
        let cached_task_run_repo = factory.create_cached_task_run_repository(task_run_repo);

        (cached_task_repo, cached_worker_repo, cached_task_run_repo)
    }

    /// Clear all cache
    pub async fn clear_all_cache(&self) -> SchedulerResult<()> {
        if let Some(ref cache_service) = self.cache_service {
            info!("Clearing all cache");
            
            // Clear all prefixes
            let prefixes = ["task", "worker", "system_stats", "task_run", "dependencies"];
            for prefix in &prefixes {
                let cleared = cache_service.clear_prefix(prefix).await?;
                if cleared > 0 {
                    info!("Cleared {} keys with prefix '{}'", cleared, prefix);
                }
            }
        } else {
            warn!("Cache is disabled, cannot clear cache");
        }

        Ok(())
    }

    /// Clear task cache
    pub async fn clear_task_cache(&self) -> SchedulerResult<()> {
        if let Some(ref cache_service) = self.cache_service {
            let cleared = cache_service.clear_prefix("task").await?;
            info!("Cleared {} task cache entries", cleared);
        }
        Ok(())
    }

    /// Clear worker cache
    pub async fn clear_worker_cache(&self) -> SchedulerResult<()> {
        if let Some(ref cache_service) = self.cache_service {
            let cleared = cache_service.clear_prefix("worker").await?;
            info!("Cleared {} worker cache entries", cleared);
        }
        Ok(())
    }

    /// Clear system stats cache
    pub async fn clear_system_stats_cache(&self) -> SchedulerResult<()> {
        if let Some(ref cache_service) = self.cache_service {
            let cleared = cache_service.clear_prefix("system_stats").await?;
            info!("Cleared {} system stats cache entries", cleared);
        }
        Ok(())
    }

    /// Health check
    pub async fn health_check(&self) -> SchedulerResult<bool> {
        if let Some(ref cache_service) = self.cache_service {
            cache_service.health_check().await
        } else {
            // Cache is disabled, consider it healthy
            Ok(true)
        }
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Option<super::CacheStats> {
        if let Some(ref cache_service) = self.cache_service {
            Some(cache_service.get_stats().await)
        } else {
            None
        }
    }

    /// Check if cache is enabled
    pub fn is_enabled(&self) -> bool {
        self.cache_service.is_some()
    }
}

impl Clone for CacheManager {
    fn clone(&self) -> Self {
        Self {
            factory: self.factory.clone(),
            cache_service: self.cache_service.clone(),
        }
    }
}

/// Cache configuration helper
pub struct CacheConfigHelper;

impl CacheConfigHelper {
    /// Create default development cache configuration
    pub fn development() -> CacheConfig {
        CacheConfig {
            enabled: false, // Disabled by default in development
            redis_url: "redis://localhost:6379".to_string(),
            max_connections: 5,
            connection_timeout_seconds: 3,
            command_timeout_seconds: 2,
            key_prefix: Some("scheduler-dev".to_string()),
            default_ttl_seconds: 300,
            use_cluster: false,
            ttl: super::config::CacheTtlConfig::default(),
        }
    }

    /// Create default production cache configuration
    pub fn production(redis_url: String) -> CacheConfig {
        CacheConfig {
            enabled: true,
            redis_url,
            max_connections: 20,
            connection_timeout_seconds: 5,
            command_timeout_seconds: 3,
            key_prefix: Some("scheduler-prod".to_string()),
            default_ttl_seconds: 300,
            use_cluster: false,
            ttl: super::config::CacheTtlConfig::default(),
        }
    }

    /// Create high-performance cache configuration
    pub fn high_performance(redis_url: String) -> CacheConfig {
        CacheConfig {
            enabled: true,
            redis_url,
            max_connections: 50,
            connection_timeout_seconds: 2,
            command_timeout_seconds: 1,
            key_prefix: Some("scheduler-hp".to_string()),
            default_ttl_seconds: 600,
            use_cluster: false,
            ttl: super::config::CacheTtlConfig {
                task_seconds: 600,        // 10 minutes
                worker_seconds: 15,       // 15 seconds
                system_stats_seconds: 30, // 30 seconds
                task_run_seconds: 1200,   // 20 minutes
                dependencies_seconds: 300, // 5 minutes
            },
        }
    }

    /// Validate cache configuration
    pub fn validate(config: &CacheConfig) -> anyhow::Result<()> {
        config.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_factory_disabled() {
        let config = CacheConfig {
            enabled: false,
            ..Default::default()
        };

        let mut factory = CacheFactory::new(config);
        factory.initialize(None).await.unwrap();

        assert!(!factory.is_enabled());
        assert!(factory.cache_service().is_none());
    }

    #[tokio::test]
    async fn test_cache_config_helper() {
        let dev_config = CacheConfigHelper::development();
        assert!(!dev_config.enabled);
        assert_eq!(dev_config.key_prefix, Some("scheduler-dev".to_string()));

        let prod_config = CacheConfigHelper::production("redis://localhost:6379".to_string());
        assert!(prod_config.enabled);
        assert_eq!(prod_config.key_prefix, Some("scheduler-prod".to_string()));

        let hp_config = CacheConfigHelper::high_performance("redis://localhost:6379".to_string());
        assert!(hp_config.enabled);
        assert_eq!(hp_config.max_connections, 50);
        assert_eq!(hp_config.ttl.task_seconds, 600);
    }

    #[test]
    fn test_cache_config_validation() {
        let mut config = CacheConfig::default();
        config.enabled = true;
        
        // Valid configuration
        assert!(CacheConfigHelper::validate(&config).is_ok());

        // Invalid Redis URL
        config.redis_url = "".to_string();
        assert!(CacheConfigHelper::validate(&config).is_err());
    }
}