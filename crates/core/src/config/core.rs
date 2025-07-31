//! 核心配置抽象
//!
//! 此模块提供了配置管理系统的核心抽象和基础组件。
//! 这些抽象为整个配置系统提供了统一的接口和类型安全保障。
//!
//! # 核心组件
//!
//! - `ConfigValue`: 带有元数据的配置值包装器
//! - `ConfigSource`: 配置来源跟踪和标识
//! - `TypedConfig`: 提供编译时类型安全的配置包装器
//! - `ConfigurationService`: 配置服务的核心特征抽象
//! - `ConfigCache`: 用于性能优化的配置缓存
//!
//! # 设计原则
//!
//! 遵循 KISS (Keep It Simple, Stupid) 和 SOLID 设计原则：
//! - **单一职责**: 每个组件都有明确的职责
//! - **开闭原则**: 通过特征抽象支持扩展
//! - **依赖倒置**: 依赖抽象而非具体实现
//! - **接口隔离**: 提供最小化的必要接口
//!
//! # 使用示例
//!
//! ```
//! use scheduler_core::config::core::{TypedConfig, ConfigurationService, ConfigValue};
//! use std::sync::Arc;
//!
//! // 创建配置服务
//! let config_service = Arc::new(MyConfigService::new());
//! 
//! // 创建类型安全的配置包装器
//! let db_url_config = TypedConfig::<String>::new(
//!     config_service.clone(), 
//!     "database.url".to_string()
//! );
//! 
//! // 获取配置值
//! let db_url = db_url_config.get().await?;
//! println!("Database URL: {:?}", db_url);
//! 
//! // 设置配置值
//! db_url_config.set(&"postgresql://localhost/mydb".to_string()).await?;
//! ```

use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use crate::{SchedulerError, SchedulerResult};

/// 配置值包装器
///
/// 包含配置值及其相关元数据的结构体，提供了完整的配置值跟踪能力。
///
/// # 字段说明
///
/// - `value`: 配置的实际值，使用 JSON Value 类型存储
/// - `source`: 配置的来源，用于跟踪配置值的来源
/// - `last_updated`: 配置的最后更新时间，用于缓存和监控
///
/// # 使用场景
///
/// - 配置值的完整生命周期管理
/// - 配置来源的跟踪和审计
/// - 配置更新时间的监控
/// - 配置缓存的有效性检查
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValue {
    /// 配置值
    pub value: Value,
    /// 配置来源
    pub source: ConfigSource,
    /// 最后更新时间
    pub last_updated: std::time::SystemTime,
}

/// 配置来源枚举
///
/// 用于跟踪和标识配置值的来源，支持配置的审计和调试。
///
/// # 变体说明
///
/// - `File`: 来自文件配置
/// - `Environment`: 来自环境变量
/// - `Default`: 来自默认值
/// - `Runtime`: 来自运行时设置
///
/// # 使用场景
///
/// - 配置来源的优先级管理
/// - 配置值的审计和调试
/// - 配置更新策略的制定
/// - 配置冲突的解决
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfigSource {
    /// 文件配置来源
    File(String),
    /// 环境变量配置来源
    Environment,
    /// 默认值配置来源
    Default,
    /// 运行时配置来源
    Runtime,
}

/// 类型安全的配置包装器
///
/// 提供编译时类型安全的配置访问接口，确保配置值的类型正确性。
/// 使用泛型和 PhantomData 来确保类型安全，避免运行时类型错误。
///
/// # 类型参数
///
/// - `T`: 配置值的类型，必须支持序列化和反序列化
///
/// # 特性
///
/// - **类型安全**: 编译时确保配置值的类型正确性
/// - **异步支持**: 所有操作都支持异步执行
/// - **错误处理**: 提供详细的错误信息和上下文
/// - **线程安全**: 支持多线程环境下的并发访问
///
/// # 使用示例
///
/// ```
/// use scheduler_core::config::core::{TypedConfig, ConfigurationService};
/// use std::sync::Arc;
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct DatabaseConfig {
///     url: String,
///     pool_size: u32,
/// }
///
/// let config_service = Arc::new(MyConfigService::new());
/// 
/// // 创建数据库配置的类型安全包装器
/// let db_config = TypedConfig::<DatabaseConfig>::new(
///     config_service, 
///     "database".to_string()
/// );
/// 
/// // 获取配置值，返回类型为 DatabaseConfig
/// let config = db_config.get().await?;
/// if let Some(db_config) = config {
///     println!("Database URL: {}", db_config.url);
///     println!("Pool size: {}", db_config.pool_size);
/// }
/// ```
pub struct TypedConfig<T> {
    /// 配置服务实例
    service: Arc<dyn ConfigurationService>,
    /// 配置键名
    key: String,
    /// 类型标记 PhantomData
    _phantom: PhantomData<T>,
}

impl<T> TypedConfig<T>
where
    T: Serialize + DeserializeOwned + Send + Sync,
{
    /// 创建新的类型安全配置包装器
    ///
    /// 初始化一个类型安全的配置包装器实例。
    ///
    /// # 参数
    ///
    /// * `service` - 配置服务实例，必须实现 ConfigurationService 特征
    /// * `key` - 配置键名，用于标识配置项
    ///
    /// # 返回值
    ///
    /// 返回一个新的 TypedConfig 实例
    ///
    /// # 示例
    ///
    /// ```
    /// use scheduler_core::config::core::{TypedConfig, ConfigurationService};
    /// use std::sync::Arc;
    ///
    /// let config_service = Arc::new(MyConfigService::new());
    /// let typed_config = TypedConfig::<String>::new(
    ///     config_service, 
    ///     "database.url".to_string()
    /// );
    /// ```
    pub fn new(service: Arc<dyn ConfigurationService>, key: String) -> Self {
        Self {
            service,
            key,
            _phantom: PhantomData,
        }
    }

    /// 获取配置值（类型安全）
    ///
    /// 从配置服务中获取指定键的配置值，并进行类型安全的反序列化。
    ///
    /// # 返回值
    ///
    /// - `Ok(Some(T))`: 成功获取配置值
    /// - `Ok(None)`: 配置键不存在
    /// - `Err(SchedulerError)`: 获取或反序列化失败
    ///
    /// # 错误处理
    ///
    /// 如果配置值存在但无法反序列化为指定类型，将返回 Configuration 错误。
    ///
    /// # 示例
    ///
    /// ```
    /// let typed_config = TypedConfig::<String>::new(service, "app.name".to_string());
    /// match typed_config.get().await? {
    ///     Some(name) => println!("Application name: {}", name),
    ///     None => println!("Application name not configured"),
    /// }
    /// ```
    pub async fn get(&self) -> SchedulerResult<Option<T>> {
        match self.service.get_config_value(&self.key).await? {
            Some(config_value) => {
                let typed_value: T = serde_json::from_value(config_value.value).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to deserialize config '{}': {}",
                        self.key, e
                    ))
                })?;
                Ok(Some(typed_value))
            }
            None => Ok(None),
        }
    }

    /// 获取配置值或使用默认值
    ///
    /// 尝试获取配置值，如果配置不存在则返回提供的默认值。
    ///
    /// # 参数
    ///
    /// * `default` - 当配置不存在时使用的默认值
    ///
    /// # 返回值
    ///
    /// 返回配置值或默认值，此操作不会失败（除非发生系统错误）
    ///
    /// # 示例
    ///
    /// ```
    /// let typed_config = TypedConfig::<u32>::new(service, "server.port".to_string());
    /// let port = typed_config.get_or_default(8080).await?;
    /// println!("Server port: {}", port);
    /// ```
    pub async fn get_or_default(&self, default: T) -> SchedulerResult<T> {
        match self.get().await? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }

    /// 设置配置值（类型安全）
    ///
    /// 将类型安全的配置值设置到配置服务中。
    ///
    /// # 参数
    ///
    /// * `value` - 要设置的配置值
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 设置成功
    /// - `Err(SchedulerError)`: 序列化或设置失败
    ///
    /// # 错误处理
    ///
    /// 如果值无法序列化为 JSON，将返回 Configuration 错误。
    ///
    /// # 示例
    ///
    /// ```
    /// let typed_config = TypedConfig::<String>::new(service, "app.name".to_string());
    /// typed_config.set(&"My Application".to_string()).await?;
    /// ```
    pub async fn set(&self, value: &T) -> SchedulerResult<()> {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!(
                "Failed to serialize config '{}': {}",
                self.key, e
            ))
        })?;
        let config_value = ConfigValue {
            value: json_value,
            source: ConfigSource::Runtime,
            last_updated: std::time::SystemTime::now(),
        };
        self.service
            .set_config_value(&self.key, &config_value)
            .await
    }

    /// 获取配置键名
    ///
    /// 返回此配置包装器关联的配置键名。
    ///
    /// # 返回值
    ///
    /// 返回配置键名的字符串切片
    ///
    /// # 示例
    ///
    /// ```
    /// let typed_config = TypedConfig::<String>::new(service, "database.url".to_string());
    /// println!("Config key: {}", typed_config.key());
    /// ```
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// 配置服务特征
///
/// 定义了配置系统的核心抽象接口，所有配置服务实现都必须遵循此特征。
/// 这个特征提供了配置值的获取、设置、删除和列表功能。
///
/// # 设计原则
///
/// - **抽象化**: 通过特征抽象实现配置服务的解耦
/// - **可扩展**: 支持不同类型的配置存储后端
/// - **异步支持**: 所有操作都支持异步执行
/// - **错误处理**: 统一的错误处理机制
///
/// # 必须实现的方法
///
/// - `get_config_value`: 获取配置值
/// - `set_config_value`: 设置配置值
/// - `delete_config`: 删除配置
/// - `list_config_keys`: 列出所有配置键
/// - `reload_config`: 重新加载配置
/// - `get_config_source`: 获取配置来源
///
/// # 实现示例
///
/// ```
/// use scheduler_core::config::core::{ConfigurationService, ConfigValue, ConfigSource};
/// use scheduler_core::{SchedulerResult, SchedulerError};
/// use async_trait::async_trait;
/// use std::collections::HashMap;
///
/// #[derive(Debug)]
/// struct InMemoryConfigService {
///     configs: HashMap<String, ConfigValue>,
/// }
///
/// #[async_trait]
/// impl ConfigurationService for InMemoryConfigService {
///     async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<ConfigValue>> {
///         Ok(self.configs.get(key).cloned())
///     }
///     
///     async fn set_config_value(&self, key: &str, value: &ConfigValue) -> SchedulerResult<()> {
///         self.configs.insert(key.to_string(), value.clone());
///         Ok(())
///     }
///     
///     async fn delete_config(&self, key: &str) -> SchedulerResult<bool> {
///         Ok(self.configs.remove(key).is_some())
///     }
///     
///     async fn list_config_keys(&self) -> SchedulerResult<Vec<String>> {
///         Ok(self.configs.keys().cloned().collect())
///     }
///     
///     async fn reload_config(&self) -> SchedulerResult<()> {
///         // 内存配置不需要重新加载
///         Ok(())
///     }
///     
///     async fn get_config_source(&self, key: &str) -> SchedulerResult<Option<ConfigSource>> {
///         Ok(self.configs.get(key).map(|v| v.source.clone()))
///     }
/// }
/// ```
#[async_trait]
pub trait ConfigurationService: Send + Sync {
    /// 获取配置值
    ///
    /// 根据键名获取配置值，返回包含元数据的 ConfigValue。
    ///
    /// # 参数
    ///
    /// * `key` - 配置键名
    ///
    /// # 返回值
    ///
    /// - `Ok(Some(ConfigValue))`: 找到配置值
    /// - `Ok(None)`: 配置键不存在
    /// - `Err(SchedulerError)`: 获取失败
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<ConfigValue>>;

    /// 设置配置值
    ///
    /// 设置配置值，包含完整的元数据信息。
    ///
    /// # 参数
    ///
    /// * `key` - 配置键名
    /// * `value` - 配置值及其元数据
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 设置成功
    /// - `Err(SchedulerError)`: 设置失败
    async fn set_config_value(&self, key: &str, value: &ConfigValue) -> SchedulerResult<()>;

    /// 删除配置
    ///
    /// 删除指定键名的配置。
    ///
    /// # 参数
    ///
    /// * `key` - 配置键名
    ///
    /// # 返回值
    ///
    /// - `Ok(true)`: 配置存在并被删除
    /// - `Ok(false)`: 配置不存在
    /// - `Err(SchedulerError)`: 删除失败
    async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;

    /// 列出所有配置键
    ///
    /// 获取系统中所有配置键的列表。
    ///
    /// # 返回值
    ///
    /// - `Ok(Vec<String>)`: 配置键列表
    /// - `Err(SchedulerError)`: 获取失败
    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;

    /// 重新加载配置
    ///
    /// 重新加载配置，通常用于从外部源刷新配置。
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 重新加载成功
    /// - `Err(SchedulerError)`: 重新加载失败
    async fn reload_config(&self) -> SchedulerResult<()>;

    /// 获取配置来源
    ///
    /// 获取指定配置键的来源信息。
    ///
    /// # 参数
    ///
    /// * `key` - 配置键名
    ///
    /// # 返回值
    ///
    /// - `Ok(Some(ConfigSource))`: 配置来源信息
    /// - `Ok(None)`: 配置不存在
    /// - `Err(SchedulerError)`: 获取失败
    async fn get_config_source(&self, key: &str) -> SchedulerResult<Option<ConfigSource>>;
}

/// 配置缓存
///
/// 用于提高配置系统性能的内存缓存组件。
/// 使用 TTL (Time To Live) 机制来确保缓存数据的时效性。
///
/// # 特性
///
/// - **内存缓存**: 减少对底层存储的访问次数
/// - **TTL 支持**: 自动过期机制确保数据新鲜度
/// - **线程安全**: 支持多线程环境下的并发访问
/// - **异步支持**: 所有操作都支持异步执行
///
/// # 使用场景
///
/// - 频繁访问的配置项缓存
/// - 减少数据库或文件系统的访问压力
/// - 提高配置读取性能
///
/// # 性能考虑
///
/// - 默认 TTL 为 5 分钟
/// - 使用 RwLock 实现读写分离
/// - 缓存过期检查在读取时进行
///
/// # 示例
///
/// ```
/// use scheduler_core::config::core::ConfigCache;
/// use std::time::Duration;
///
/// // 创建 TTL 为 1 分钟的缓存
/// let cache = ConfigCache::new(Duration::from_secs(60));
/// 
/// // 设置缓存值
/// let config_value = create_config_value();
/// cache.set("database.url".to_string(), config_value).await;
/// 
/// // 获取缓存值
/// if let Some(value) = cache.get("database.url").await {
///     println!("Cached value: {:?}", value);
/// } else {
///     println!("Cache miss or expired");
/// }
/// 
/// // 清空缓存
/// cache.clear().await;
/// ```
pub struct ConfigCache {
    /// 缓存存储，使用 HashMap 存储配置值
    pub cache: Arc<tokio::sync::RwLock<std::collections::HashMap<String, ConfigValue>>>,
    /// 缓存过期时间
    pub ttl: std::time::Duration,
}

impl ConfigCache {
    /// 创建新的配置缓存
    ///
    /// 初始化一个具有指定 TTL 的配置缓存实例。
    ///
    /// # 参数
    ///
    /// * `ttl` - 缓存过期时间
    ///
    /// # 返回值
    ///
    /// 返回一个新的 ConfigCache 实例
    ///
    /// # 示例
    ///
    /// ```
    /// use scheduler_core::config::core::ConfigCache;
    /// use std::time::Duration;
    ///
    /// // 创建 TTL 为 1 分钟的缓存
    /// let cache = ConfigCache::new(Duration::from_secs(60));
    /// ```
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            ttl,
        }
    }

    /// 获取缓存值
    ///
    /// 从缓存中获取指定键的配置值，如果值已过期则返回 None。
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键名
    ///
    /// # 返回值
    ///
    /// - `Some(ConfigValue)`: 找到且未过期的缓存值
    /// - `None`: 缓存未命中或已过期
    ///
    /// # 示例
    ///
    /// ```
    /// let cache = ConfigCache::default();
    /// if let Some(value) = cache.get("database.url").await {
    ///     println!("Cached value: {:?}", value);
    /// }
    /// ```
    pub async fn get(&self, key: &str) -> Option<ConfigValue> {
        let cache = self.cache.read().await;
        cache.get(key).cloned().and_then(|config_value| {
            if config_value
                .last_updated
                .elapsed()
                .unwrap_or(std::time::Duration::ZERO)
                < self.ttl
            {
                Some(config_value)
            } else {
                None
            }
        })
    }

    /// 设置缓存值
    ///
    /// 将配置值存入缓存中。
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键名
    /// * `value` - 要缓存的配置值
    ///
    /// # 示例
    ///
    /// ```
    /// let cache = ConfigCache::default();
    /// let config_value = create_config_value();
    /// cache.set("database.url".to_string(), config_value).await;
    /// ```
    pub async fn set(&self, key: String, value: ConfigValue) {
        let mut cache = self.cache.write().await;
        cache.insert(key, value);
    }

    /// 清空缓存
    ///
    /// 清空缓存中的所有配置值。
    ///
    /// # 示例
    ///
    /// ```
    /// let cache = ConfigCache::default();
    /// cache.clear().await;
    /// ```
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

impl Default for ConfigCache {
    /// 创建默认配置缓存
    ///
    /// 返回一个具有默认 TTL (5 分钟) 的配置缓存实例。
    ///
    /// # 返回值
    ///
    /// 返回默认的 ConfigCache 实例
    fn default() -> Self {
        Self::new(std::time::Duration::from_secs(300)) // 5 minutes
    }
}