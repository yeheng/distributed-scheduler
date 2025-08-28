use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use scheduler_config::AppConfig;

/// 配置对象缓存池，用于减少频繁的配置解析和克隆操作
pub struct ConfigCache {
    /// 缓存的配置对象
    cache: Arc<RwLock<HashMap<String, CachedConfig>>>,
    /// 默认过期时间
    default_ttl: Duration,
    /// 最大缓存项数
    max_entries: usize,
    /// 统计信息
    stats: Arc<RwLock<CacheStats>>,
}

/// 缓存的配置项
#[derive(Clone)]
struct CachedConfig {
    data: Arc<dyn CacheableConfig>,
    created_at: Instant,
    ttl: Duration,
    access_count: u64,
    last_accessed: Instant,
}

/// 可缓存的配置接口
pub trait CacheableConfig: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    fn cache_key(&self) -> String;
    fn serialize_for_cache(&self) -> serde_json::Value;
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub total_entries: usize,
    pub hit_rate: f64,
}

impl ConfigCache {
    /// 创建新的配置缓存
    pub fn new(default_ttl: Duration, max_entries: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
            max_entries,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// 从缓存中获取配置，如果不存在或过期则返回None
    pub fn get<T: CacheableConfig + Clone + 'static>(&self, key: &str) -> Option<Arc<T>> {
        let now = Instant::now();

        if let Ok(mut cache) = self.cache.write() {
            if let Some(cached) = cache.get_mut(key) {
                // 检查是否过期
                if now.duration_since(cached.created_at) > cached.ttl {
                    // 过期，移除
                    cache.remove(key);
                    self.increment_miss();
                    return None;
                }

                // 更新访问统计
                cached.access_count += 1;
                cached.last_accessed = now;

                // 尝试转换类型
                if let Some(config) = cached.data.as_any().downcast_ref::<T>() {
                    self.increment_hit();
                    return Some(Arc::new(config.clone()));
                }
            }
        }

        self.increment_miss();
        None
    }

    /// 将配置存入缓存
    pub fn put<T: CacheableConfig + Clone + 'static>(&self, config: T) {
        self.put_with_ttl(config, self.default_ttl);
    }

    /// 将配置存入缓存，指定TTL
    pub fn put_with_ttl<T: CacheableConfig + Clone + 'static>(&self, config: T, ttl: Duration) {
        let key = config.cache_key();
        let now = Instant::now();

        if let Ok(mut cache) = self.cache.write() {
            // 检查是否需要清理过期项
            if cache.len() >= self.max_entries {
                self.evict_expired(&mut cache, now);

                // 如果仍然满了，移除最少访问的项
                if cache.len() >= self.max_entries {
                    self.evict_lru(&mut cache);
                }
            }

            let cached = CachedConfig {
                data: Arc::new(config),
                created_at: now,
                ttl,
                access_count: 0,
                last_accessed: now,
            };

            cache.insert(key, cached);

            // 更新统计
            if let Ok(mut stats) = self.stats.write() {
                stats.total_entries = cache.len();
            }
        }
    }

    /// 移除缓存项
    pub fn remove(&self, key: &str) -> bool {
        if let Ok(mut cache) = self.cache.write() {
            let removed = cache.remove(key).is_some();

            if removed {
                if let Ok(mut stats) = self.stats.write() {
                    stats.total_entries = cache.len();
                }
            }

            removed
        } else {
            false
        }
    }

    /// 清空缓存
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();

            if let Ok(mut stats) = self.stats.write() {
                stats.total_entries = 0;
            }
        }
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        if let Ok(stats) = self.stats.read() {
            let mut stats = stats.clone();
            stats.hit_rate = if stats.hits + stats.misses > 0 {
                stats.hits as f64 / (stats.hits + stats.misses) as f64
            } else {
                0.0
            };
            stats
        } else {
            CacheStats::default()
        }
    }

    /// 手动清理过期项
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        if let Ok(mut cache) = self.cache.write() {
            self.evict_expired(&mut cache, now);

            if let Ok(mut stats) = self.stats.write() {
                stats.total_entries = cache.len();
            }
        }
    }

    // 内部方法：清理过期项
    fn evict_expired(&self, cache: &mut HashMap<String, CachedConfig>, now: Instant) {
        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter(|(_, cached)| now.duration_since(cached.created_at) > cached.ttl)
            .map(|(key, _)| key.clone())
            .collect();

        for key in keys_to_remove {
            cache.remove(&key);
            if let Ok(mut stats) = self.stats.write() {
                stats.evictions += 1;
            }
        }
    }

    // 内部方法：LRU淘汰
    fn evict_lru(&self, cache: &mut HashMap<String, CachedConfig>) {
        if let Some((lru_key, _)) = cache
            .iter()
            .min_by_key(|(_, cached)| cached.last_accessed)
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            cache.remove(&lru_key);
            if let Ok(mut stats) = self.stats.write() {
                stats.evictions += 1;
            }
        }
    }

    // 增加命中计数
    fn increment_hit(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.hits += 1;
        }
    }

    // 增加未命中计数
    fn increment_miss(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.misses += 1;
        }
    }
}

// 为AppConfig实现CacheableConfig
// TODO: 等scheduler_config crate完善后启用
impl CacheableConfig for AppConfig {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn cache_key(&self) -> String {
        // 使用配置的哈希值作为缓存键，确保相同配置使用相同缓存
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        // 使用关键配置字段生成哈希
        self.database.url.hash(&mut hasher);
        self.api.bind_address.hash(&mut hasher);
        self.worker.worker_id.hash(&mut hasher);

        format!("app_config_{:x}", hasher.finish())
    }

    fn serialize_for_cache(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// 全局配置缓存管理器
// TODO: 等scheduler_config crate完善后启用
pub struct GlobalConfigCache {
    cache: ConfigCache,
}

impl GlobalConfigCache {
    /// 创建全局配置缓存
    pub fn new(default_ttl: Duration, max_entries: usize) -> Self {
        Self {
            cache: ConfigCache::new(default_ttl, max_entries),
        }
    }

    /// 获取应用配置
    pub fn get_app_config(&self, cache_key: &str) -> Option<Arc<AppConfig>> {
        self.cache.get(cache_key)
    }

    /// 缓存应用配置
    pub fn cache_app_config(&self, config: AppConfig) {
        self.cache.put(config);
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        self.cache.stats()
    }

    /// 清理过期配置
    pub fn cleanup(&self) {
        self.cache.cleanup_expired();
    }

    /// 清空所有缓存
    pub fn clear(&self) {
        self.cache.clear();
    }
}

/// 配置缓存构建器
pub struct ConfigCacheBuilder {
    default_ttl: Duration,
    max_entries: usize,
}

impl ConfigCacheBuilder {
    pub fn new() -> Self {
        Self {
            default_ttl: Duration::from_secs(300), // 5分钟默认TTL
            max_entries: 100,
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    pub fn build(self) -> ConfigCache {
        ConfigCache::new(self.default_ttl, self.max_entries)
    }
}

impl Default for ConfigCacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[derive(Clone, Debug, PartialEq)]
    struct TestConfig {
        name: String,
        value: i32,
    }

    impl CacheableConfig for TestConfig {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn cache_key(&self) -> String {
            format!("test_config_{}", self.name)
        }

        fn serialize_for_cache(&self) -> serde_json::Value {
            serde_json::json!({
                "name": self.name,
                "value": self.value
            })
        }
    }

    #[test]
    fn test_config_cache_put_and_get() {
        let cache = ConfigCache::new(Duration::from_secs(60), 10);

        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };

        // 缓存配置
        cache.put(config.clone());

        // 获取配置
        let cached_config: Option<Arc<TestConfig>> = cache.get("test_config_test");
        assert!(cached_config.is_some());
        assert_eq!(cached_config.unwrap().value, 42);

        // 统计信息
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_config_cache_expiration() {
        let cache = ConfigCache::new(Duration::from_millis(100), 10);

        let config = TestConfig {
            name: "expire_test".to_string(),
            value: 100,
        };

        // 缓存配置
        cache.put(config);

        // 立即获取应该成功
        let cached: Option<Arc<TestConfig>> = cache.get("test_config_expire_test");
        assert!(cached.is_some());

        // 等待过期
        thread::sleep(Duration::from_millis(150));

        // 再次获取应该失败
        let expired: Option<Arc<TestConfig>> = cache.get("test_config_expire_test");
        assert!(expired.is_none());
    }

    #[test]
    fn test_config_cache_lru_eviction() {
        let cache = ConfigCache::new(Duration::from_secs(60), 2);

        let config1 = TestConfig {
            name: "config1".to_string(),
            value: 1,
        };
        let config2 = TestConfig {
            name: "config2".to_string(),
            value: 2,
        };
        let config3 = TestConfig {
            name: "config3".to_string(),
            value: 3,
        };

        // 缓存三个配置，应该触发LRU淘汰
        cache.put(config1.clone());
        cache.put(config2.clone());
        cache.put(config3.clone());

        // config1应该被淘汰
        let cached1: Option<Arc<TestConfig>> = cache.get("test_config_config1");
        assert!(cached1.is_none());

        // config2和config3应该还在
        let cached2: Option<Arc<TestConfig>> = cache.get("test_config_config2");
        let cached3: Option<Arc<TestConfig>> = cache.get("test_config_config3");
        assert!(cached2.is_some());
        assert!(cached3.is_some());
    }

    #[test]
    fn test_global_config_cache() {
        let global_cache = GlobalConfigCache::new(Duration::from_secs(60), 10);

        // 创建测试配置
        let config = AppConfig::default();
        let cache_key = config.cache_key();

        // 缓存配置
        global_cache.cache_app_config(config.clone());

        // 获取配置
        let cached = global_cache.get_app_config(&cache_key);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().database.url, config.database.url);
    }

    #[test]
    fn test_config_cache_builder() {
        let cache = ConfigCacheBuilder::new()
            .with_ttl(Duration::from_secs(30))
            .with_max_entries(50)
            .build();

        let config = TestConfig {
            name: "builder_test".to_string(),
            value: 999,
        };

        cache.put(config.clone());

        let cached: Option<Arc<TestConfig>> = cache.get("test_config_builder_test");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().value, 999);
    }
}
