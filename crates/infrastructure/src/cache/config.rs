//! Cache configuration for Redis-based caching

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Redis cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Redis connection URL
    pub redis_url: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout_seconds: u64,
    /// Command timeout in seconds
    pub command_timeout_seconds: u64,
    /// Cache key prefix for this instance
    pub key_prefix: Option<String>,
    /// Default TTL for cache entries
    pub default_ttl_seconds: u64,
    /// Whether to use cluster mode
    pub use_cluster: bool,
    /// Cache-specific TTL configurations
    pub ttl: CacheTtlConfig,
}

/// Cache TTL configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheTtlConfig {
    /// Task definitions cache TTL in seconds
    pub task_seconds: u64,
    /// Worker status cache TTL in seconds
    pub worker_seconds: u64,
    /// System stats cache TTL in seconds
    pub system_stats_seconds: u64,
    /// Task run cache TTL in seconds
    pub task_run_seconds: u64,
    /// Dependencies cache TTL in seconds
    pub dependencies_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            redis_url: "redis://localhost:6379".to_string(),
            max_connections: 10,
            connection_timeout_seconds: 5,
            command_timeout_seconds: 3,
            key_prefix: Some("scheduler".to_string()),
            default_ttl_seconds: 300,
            use_cluster: false,
            ttl: CacheTtlConfig::default(),
        }
    }
}

impl Default for CacheTtlConfig {
    fn default() -> Self {
        Self {
            task_seconds: 300,         // 5 minutes
            worker_seconds: 30,        // 30 seconds
            system_stats_seconds: 60,  // 1 minute
            task_run_seconds: 600,     // 10 minutes
            dependencies_seconds: 120, // 2 minutes
        }
    }
}

impl CacheConfig {
    /// Validate cache configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.enabled {
            if self.redis_url.is_empty() {
                return Err(anyhow::anyhow!("Redis URL cannot be empty"));
            }

            if !self.redis_url.starts_with("redis://") && !self.redis_url.starts_with("rediss://") {
                return Err(anyhow::anyhow!(
                    "Redis URL must start with redis:// or rediss://"
                ));
            }

            if self.max_connections == 0 {
                return Err(anyhow::anyhow!("Max connections must be greater than 0"));
            }

            if self.connection_timeout_seconds == 0 {
                return Err(anyhow::anyhow!("Connection timeout must be greater than 0"));
            }

            if self.command_timeout_seconds == 0 {
                return Err(anyhow::anyhow!("Command timeout must be greater than 0"));
            }

            if self.default_ttl_seconds == 0 {
                return Err(anyhow::anyhow!("Default TTL must be greater than 0"));
            }

            self.ttl.validate()?;
        }

        Ok(())
    }
}

impl CacheTtlConfig {
    /// Validate TTL configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.task_seconds == 0 {
            return Err(anyhow::anyhow!("Task TTL must be greater than 0"));
        }

        if self.worker_seconds == 0 {
            return Err(anyhow::anyhow!("Worker TTL must be greater than 0"));
        }

        if self.system_stats_seconds == 0 {
            return Err(anyhow::anyhow!("System stats TTL must be greater than 0"));
        }

        if self.task_run_seconds == 0 {
            return Err(anyhow::anyhow!("Task run TTL must be greater than 0"));
        }

        if self.dependencies_seconds == 0 {
            return Err(anyhow::anyhow!("Dependencies TTL must be greater than 0"));
        }

        Ok(())
    }

    /// Convert to CacheTtl
    pub fn to_cache_ttl(&self) -> super::CacheTtl {
        super::CacheTtl {
            task: Duration::from_secs(self.task_seconds),
            worker: Duration::from_secs(self.worker_seconds),
            system_stats: Duration::from_secs(self.system_stats_seconds),
            task_run: Duration::from_secs(self.task_run_seconds),
            dependencies: Duration::from_secs(self.dependencies_seconds),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.default_ttl_seconds, 300);
    }

    #[test]
    fn test_cache_config_validation() {
        let mut config = CacheConfig::default();
        config.enabled = true;

        // Valid configuration
        assert!(config.validate().is_ok());

        // Invalid Redis URL
        config.redis_url = "".to_string();
        assert!(config.validate().is_err());

        // Invalid max connections
        config.redis_url = "redis://localhost:6379".to_string();
        config.max_connections = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ttl_config_conversion() {
        let ttl_config = CacheTtlConfig::default();
        let cache_ttl = ttl_config.to_cache_ttl();

        assert_eq!(cache_ttl.task, Duration::from_secs(300));
        assert_eq!(cache_ttl.worker, Duration::from_secs(30));
        assert_eq!(cache_ttl.system_stats, Duration::from_secs(60));
        assert_eq!(cache_ttl.task_run, Duration::from_secs(600));
        assert_eq!(cache_ttl.dependencies, Duration::from_secs(120));
    }
}
