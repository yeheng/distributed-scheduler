use std::collections::HashMap;
use std::sync::Arc;

use crate::config::core::ConfigValue;

/// Configuration cache with TTL support
pub struct ConfigCache {
    cache: Arc<tokio::sync::RwLock<HashMap<String, CacheEntry>>>,
    ttl: std::time::Duration,
}

#[derive(Clone)]
struct CacheEntry {
    value: ConfigValue,
    timestamp: std::time::SystemTime,
}

impl ConfigCache {
    /// Create new configuration cache
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            ttl,
        }
    }

    /// Get value from cache
    pub async fn get(&self, key: &str) -> Option<ConfigValue> {
        let cache = self.cache.read().await;

        if let Some(entry) = cache.get(key) {
            if entry
                .timestamp
                .elapsed()
                .unwrap_or(std::time::Duration::MAX)
                < self.ttl
            {
                return Some(entry.value.clone());
            }
        }

        None
    }

    /// Set value in cache
    pub async fn set(&self, key: String, value: ConfigValue) {
        let mut cache = self.cache.write().await;
        cache.insert(
            key,
            CacheEntry {
                value,
                timestamp: std::time::SystemTime::now(),
            },
        );
    }

    /// Clear all entries from cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Remove specific entry from cache
    pub async fn remove(&self, key: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
    }

    /// Clean expired entries
    pub async fn clean_expired(&self) {
        let mut cache = self.cache.write().await;
        cache.retain(|_, entry| {
            entry
                .timestamp
                .elapsed()
                .unwrap_or(std::time::Duration::MAX)
                < self.ttl
        });
    }

    /// Get cache size
    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Check if cache contains key
    pub async fn contains_key(&self, key: &str) -> bool {
        let cache = self.cache.read().await;
        cache.contains_key(key)
    }
}

impl Default for ConfigCache {
    fn default() -> Self {
        Self::new(std::time::Duration::from_secs(300)) // 5 minutes
    }
}

#[cfg(test)]
mod tests {
    use crate::config::ConfigSource;

    use super::*;

    #[tokio::test]
    async fn test_config_cache() {
        let cache = ConfigCache::new(std::time::Duration::from_millis(100));

        // Test empty cache
        assert!(cache.get("non_existent").await.is_none());
        assert_eq!(cache.size().await, 0);

        // Test set and get
        let value = ConfigValue {
            value: serde_json::Value::String("test_value".to_string()),
            source: ConfigSource::Default,
            last_updated: std::time::SystemTime::now(),
        };

        cache.set("test_key".to_string(), value.clone()).await;
        assert_eq!(cache.size().await, 1);
        assert!(cache.contains_key("test_key").await);

        let cached_value = cache.get("test_key").await.unwrap();
        assert_eq!(cached_value.value, value.value);

        // Test remove
        cache.remove("test_key").await;
        assert_eq!(cache.size().await, 0);
        assert!(!cache.contains_key("test_key").await);

        // Test clear
        cache.set("key1".to_string(), value.clone()).await;
        cache.set("key2".to_string(), value.clone()).await;
        assert_eq!(cache.size().await, 2);

        cache.clear().await;
        assert_eq!(cache.size().await, 0);
    }

    #[tokio::test]
    async fn test_cache_ttl() {
        let cache = ConfigCache::new(std::time::Duration::from_millis(50));

        let value = ConfigValue {
            value: serde_json::Value::String("test_value".to_string()),
            source: ConfigSource::Default,
            last_updated: std::time::SystemTime::now(),
        };

        cache.set("test_key".to_string(), value.clone()).await;
        assert!(cache.get("test_key").await.is_some());

        // Wait for TTL to expire
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Value should be expired
        assert!(cache.get("test_key").await.is_none());
    }
}
