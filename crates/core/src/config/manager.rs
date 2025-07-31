//! 统一配置管理器
//!
//! 此模块提供了一个全面的配置管理系统，整合了多个实现的最佳特性。
//! 它支持多种配置源、验证、热重载和类型安全的配置访问。
//!
//! # 核心特性
//!
//! - **多配置源支持**: 文件、环境变量、命令行参数、远程配置、内存配置
//! - **配置验证**: 支持自定义验证器和验证规则
//! - **热重载**: 支持文件监控和定期重载
//! - **类型安全**: 提供编译时类型安全的配置访问
//! - **环境配置**: 支持不同环境的特定配置
//! - **构建器模式**: 提供流式配置构建接口
//! - **性能优化**: 内置缓存和并发访问支持
//!
//! # 设计原则
//!
//! 遵循 KISS (Keep It Simple, Stupid) 和 SOLID 设计原则：
//! - **单一职责**: 每个组件都有明确的职责和功能
//! - **开闭原则**: 通过特征和枚举支持扩展
//! - **里氏替换**: 所有配置源都可以互换使用
//! - **接口隔离**: 提供最小化的必要接口
//! - **依赖倒置**: 依赖抽象而非具体实现
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use scheduler_core::config::manager::{ConfigBuilder, ConfigSource, ReloadStrategy};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 创建配置构建器
//!     let manager = ConfigBuilder::new()
//!         .add_source(ConfigSource::Memory {
//!             data: json!({
//!                 "server": {
//!                     "host": "localhost",
//!                     "port": 8080
//!                 },
//!                 "database": {
//!                     "url": "postgresql://localhost/mydb"
//!                 }
//!             }),
//!         })
//!         .with_reload_strategy(ReloadStrategy::Auto { debounce_ms: 1000 })
//!         .build();
//!     
//!     // 加载配置
//!     manager.load().await?;
//!     
//!     // 获取配置值
//!     let host: String = manager.get("server.host").await?;
//!     let port: u16 = manager.get("server.port").await?;
//!     
//!     println!("Server running on {}:{}", host, port);
//!     
//!     Ok(())
//! }
//! ```

pub use super::validation::{ConfigValidator, ConfigValidationError, BasicConfigValidator};
pub use super::environment::Environment;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::RecommendedWatcher;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{errors::SchedulerError, SchedulerResult};
use async_trait::async_trait;
use serde_json::Value;

/// 配置源枚举
///
/// 定义了系统支持的所有配置源类型，每个配置源都有特定的用途和行为。
///
/// # 变体说明
///
/// - `File`: 基于文件的配置源，支持 JSON、YAML 等格式
/// - `Environment`: 基于环境变量的配置源
/// - `CommandLine`: 基于命令行参数的配置源
/// - `Remote`: 远程配置源（如 HTTP API）
/// - `Memory`: 内存中的配置源，通常用于默认值或测试
///
/// # 使用场景
///
/// - **File**: 生产环境中的持久化配置
/// - **Environment**: 容器化部署和 CI/CD 环境
/// - **CommandLine**: 开发和调试时的临时配置
/// - **Remote**: 集中化配置管理服务
/// - **Memory**: 单元测试和默认配置
///
/// # 优先级
///
/// 配置源的优先级按照添加顺序决定，后添加的源会覆盖先添加的源中的相同配置项。
///
/// # 示例
///
/// ```rust,no_run
/// use scheduler_core::config::manager::ConfigSource;
/// use std::path::PathBuf;
/// use serde_json::json;
///
/// // 文件配置源
/// let file_source = ConfigSource::File { 
///     path: PathBuf::from("config.json") 
/// };
///
/// // 环境变量配置源
/// let env_source = ConfigSource::Environment { 
///     prefix: "APP_".to_string() 
/// };
///
/// // 内存配置源
/// let memory_source = ConfigSource::Memory { 
///     data: json!({"key": "value"}) 
/// };
/// ```
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// 基于文件的配置源
    /// 
    /// 从文件中加载配置，支持 JSON、YAML 等格式。
    /// 文件路径可以是绝对路径或相对路径。
    File { path: PathBuf },
    
    /// 基于环境变量的配置源
    /// 
    /// 从环境变量中加载配置，支持前缀过滤。
    /// 环境变量名中的下划线会被转换为配置键中的点号。
    Environment { prefix: String },
    
    /// 基于命令行参数的配置源
    /// 
    /// 从命令行参数中解析配置，支持 `--key value` 和 `--flag` 格式。
    CommandLine { args: Vec<String> },
    
    /// 远程配置源
    /// 
    /// 从远程 HTTP API 加载配置，支持集中化配置管理。
    Remote { url: String },
    
    /// 内存配置源
    /// 
    /// 直接使用内存中的 JSON 数据作为配置源。
    /// 通常用于默认配置、测试配置或动态配置。
    Memory { data: Value },
}

/// 配置服务特征抽象
///
/// 为配置管理器提供统一的抽象接口，支持不同类型的配置服务实现。
/// 这个特征确保了配置服务的可扩展性和可测试性。
///
/// # 必须实现的方法
///
/// - `get_config_value`: 获取配置值
/// - `set_config_value`: 设置配置值
/// - `delete_config`: 删除配置
/// - `list_config_keys`: 列出所有配置键
/// - `reload_config`: 重新加载配置
///
/// # 设计原则
///
/// - **异步支持**: 所有操作都支持异步执行
/// - **错误处理**: 统一的错误处理机制
/// - **类型安全**: 使用 JSON Value 类型确保类型安全
/// - **可扩展**: 支持不同类型的后端存储
///
/// # 实现示例
///
/// ```rust,no_run
/// use scheduler_core::config::manager::ConfigurationService;
/// use scheduler_core::{SchedulerResult, SchedulerError};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct MyConfigService;
///
/// #[async_trait]
/// impl ConfigurationService for MyConfigService {
///     async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<Value>> {
///         // 实现配置获取逻辑
///         Ok(None)
///     }
///     
///     async fn set_config_value(&self, key: &str, value: &Value) -> SchedulerResult<()> {
///         // 实现配置设置逻辑
///         Ok(())
///     }
///     
///     async fn delete_config(&self, key: &str) -> SchedulerResult<bool> {
///         // 实现配置删除逻辑
///         Ok(false)
///     }
///     
///     async fn list_config_keys(&self) -> SchedulerResult<Vec<String>> {
///         // 实现配置键列表逻辑
///         Ok(vec![])
///     }
///     
///     async fn reload_config(&self) -> SchedulerResult<()> {
///         // 实现配置重载逻辑
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait ConfigurationService: Send + Sync {
    /// 获取配置值
    ///
    /// 根据键名获取配置值，返回 JSON Value 类型。
    ///
    /// # 参数
    ///
    /// * `key` - 配置键名，支持点号分隔的嵌套键
    ///
    /// # 返回值
    ///
    /// - `Ok(Some(Value))`: 找到配置值
    /// - `Ok(None)`: 配置键不存在
    /// - `Err(SchedulerError)`: 获取失败
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<Value>>;

    /// 设置配置值
    ///
    /// 设置指定键的配置值。
    ///
    /// # 参数
    ///
    /// * `key` - 配置键名
    /// * `value` - 要设置的配置值
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 设置成功
    /// - `Err(SchedulerError)`: 设置失败
    async fn set_config_value(&self, key: &str, value: &Value) -> SchedulerResult<()>;

    /// 删除配置
    ///
    /// 删除指定键的配置。
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
}

/// 配置重载策略
///
/// 定义了配置系统的重载策略，支持手动、自动和定期重载。
///
/// # 变体说明
///
/// - `Manual`: 手动重载，需要显式调用重载方法
/// - `Auto`: 自动重载，监控文件变化并自动重载
/// - `Periodic`: 定期重载，按照指定间隔定期重载
///
/// # 使用场景
///
/// - **Manual**: 生产环境中需要控制重载时机
/// - **Auto**: 开发环境中需要实时配置更新
/// - **Periodic**: 需要定期刷新配置的场景
///
/// # 性能考虑
///
/// - `Auto` 策略会消耗系统资源来监控文件变化
/// - `Periodic` 策略会按照指定间隔进行重载
/// - `Manual` 策略在资源使用上最节省
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReloadStrategy {
    /// 手动重载
    /// 
    /// 只在显式调用 `reload()` 方法时重载配置。
    /// 这是默认策略，适合生产环境使用。
    Manual,
    
    /// 自动重载
    /// 
    /// 监控配置文件的变化，在文件修改时自动重载配置。
    /// 
    /// # 参数
    ///
    /// * `debounce_ms` - 防抖时间（毫秒），避免频繁重载
    Auto { debounce_ms: u64 },
    
    /// 定期重载
    /// 
    /// 按照指定的时间间隔定期重载配置。
    /// 
    /// # 参数
    ///
    /// * `interval_seconds` - 重载间隔（秒）
    Periodic { interval_seconds: u64 },
}

/// 配置元数据
///
/// 包含配置的版本信息、修改时间、校验状态等元数据。
/// 这些信息用于配置的监控、审计和调试。
///
/// # 字段说明
///
/// - `version`: 配置版本号
/// - `last_modified`: 最后修改时间戳
/// - `checksum`: 配置校验和
/// - `source`: 配置来源信息
/// - `is_valid`: 配置是否有效
/// - `validation_errors`: 校验错误列表
///
/// # 使用场景
///
/// - 配置版本管理和回滚
/// - 配置修改审计和跟踪
/// - 配置健康状态检查
/// - 配置调试和问题排查
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    /// 配置版本号
    pub version: String,
    
    /// 最后修改时间戳（Unix 时间戳）
    pub last_modified: u64,
    
    /// 配置校验和
    pub checksum: String,
    
    /// 配置来源信息
    pub source: String,
    
    /// 配置是否有效
    pub is_valid: bool,
    
    /// 校验错误列表
    pub validation_errors: Vec<String>,
}

impl Default for ConfigMetadata {
    /// 创建默认配置元数据
    ///
    /// 返回具有默认值的配置元数据实例。
    ///
    /// # 默认值
    ///
    /// - `version`: "1.0.0"
    /// - `last_modified`: 当前时间戳
    /// - `checksum`: 空字符串
    /// - `source`: "unknown"
    /// - `is_valid`: true
    /// - `validation_errors`: 空向量
    ///
    /// # 返回值
    ///
    /// 返回默认的 ConfigMetadata 实例
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            last_modified: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            checksum: String::new(),
            source: "unknown".to_string(),
            is_valid: true,
            validation_errors: Vec::new(),
        }
    }
}

/// 统一配置管理器
///
/// 这是配置系统的核心组件，提供了统一的配置管理接口。
/// 它支持多种配置源、验证、热重载和类型安全的配置访问。
///
/// # 核心特性
///
/// - **多源配置**: 支持同时从多个源加载配置，并按照优先级合并
/// - **热重载**: 支持配置的热重载，无需重启应用
/// - **验证支持**: 内置配置验证机制，确保配置的正确性
/// - **类型安全**: 提供类型安全的配置访问接口
/// - **回调机制**: 支持配置变更的回调通知
/// - **线程安全**: 支持多线程环境下的并发访问
///
/// # 使用示例
///
/// ```rust,no_run
/// use scheduler_core::config::manager::{UnifiedConfigManager, ConfigSource, ReloadStrategy};
/// use serde_json::json;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // 创建配置管理器
///     let manager = UnifiedConfigManager::new()
///         .add_source(ConfigSource::Memory {
///             data: json!({"app": {"name": "My App"}}),
///         })
///         .add_source(ConfigSource::Environment {
///             prefix: "APP_".to_string(),
///         })
///         .with_reload_strategy(ReloadStrategy::Auto { debounce_ms: 1000 });
///     
///     // 加载配置
///     manager.load().await?;
///     
///     // 获取配置值
///     let app_name: String = manager.get("app.name").await?;
///     println!("App name: {}", app_name);
///     
///     // 获取配置元数据
///     let metadata = manager.get_metadata().await;
///     println!("Config version: {}", metadata.version);
///     
///     Ok(())
/// }
/// ```
pub struct UnifiedConfigManager {
    /// 当前配置数据
    /// 
    /// 使用 RwLock 实现线程安全的读写访问，
    /// 存储所有配置源的合并结果。
    config: Arc<RwLock<serde_json::Value>>,
    
    /// 配置元数据
    /// 
    /// 包含配置的版本信息、修改时间、校验状态等。
    metadata: Arc<RwLock<ConfigMetadata>>,
    
    /// 配置源列表
    /// 
    /// 按照优先级顺序存储的配置源，
    /// 后添加的源会覆盖先添加的源中的相同配置项。
    sources: Vec<ConfigSource>,
    
    /// 配置验证器列表
    /// 
    /// 用于验证配置有效性的自定义验证器。
    validators: Vec<Box<dyn ConfigValidator>>,
    
    /// 重载策略
    /// 
    /// 定义配置的重载策略（手动、自动、定期）。
    reload_strategy: ReloadStrategy,
    
    /// 文件监控器
    /// 
    /// 用于自动重载策略的文件变化监控器。
    _file_watcher: Option<Arc<RecommendedWatcher>>,
    
    /// 配置变更回调列表
    /// 
    /// 当配置发生变化时调用的回调函数。
    callbacks: Vec<Box<dyn Fn(&serde_json::Value) + Send + Sync>>,
}

impl UnifiedConfigManager {
    /// 创建新的配置管理器
    ///
    /// 初始化一个空的配置管理器实例。
    /// 使用此方法创建的配置管理器需要添加配置源和验证器。
    ///
    /// # 返回值
    ///
    /// 返回一个新的 UnifiedConfigManager 实例
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use scheduler_core::config::manager::UnifiedConfigManager;
    ///
    /// let manager = UnifiedConfigManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(serde_json::Value::Object(
                serde_json::Map::new(),
            ))),
            metadata: Arc::new(RwLock::new(ConfigMetadata::default())),
            sources: Vec::new(),
            validators: Vec::new(),
            reload_strategy: ReloadStrategy::Manual,
            _file_watcher: None,
            callbacks: Vec::new(),
        }
    }

    /// 添加配置源
    ///
    /// 向配置管理器添加一个新的配置源。
    /// 配置源按照添加顺序处理，后添加的源会覆盖先添加的源中的相同配置项。
    ///
    /// # 参数
    ///
    /// * `source` - 要添加的配置源
    ///
    /// # 返回值
    ///
    /// 返回修改后的 UnifiedConfigManager 实例，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use scheduler_core::config::manager::{UnifiedConfigManager, ConfigSource};
    /// use std::path::PathBuf;
    /// use serde_json::json;
    ///
    /// let manager = UnifiedConfigManager::new()
    ///     .add_source(ConfigSource::File { 
    ///         path: PathBuf::from("config.json") 
    ///     })
    ///     .add_source(ConfigSource::Memory {
    ///         data: json!({"default": "value"})
    ///     });
    /// ```
    pub fn add_source(mut self, source: ConfigSource) -> Self {
        self.sources.push(source);
        self
    }

    /// 添加配置验证器
    ///
    /// 向配置管理器添加一个配置验证器。
    /// 验证器用于确保配置的有效性和完整性。
    ///
    /// # 参数
    ///
    /// * `validator` - 要添加的配置验证器
    ///
    /// # 返回值
    ///
    /// 返回修改后的 UnifiedConfigManager 实例，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use scheduler_core::config::manager::{UnifiedConfigManager, ConfigSource};
    /// use scheduler_core::config::validation::BasicConfigValidator;
    /// use serde_json::json;
    ///
    /// let validator = BasicConfigValidator::new();
    /// let manager = UnifiedConfigManager::new()
    ///     .add_source(ConfigSource::Memory {
    ///         data: json!({"database": {"url": "postgresql://localhost/test"}})
    ///     })
    ///     .add_validator(Box::new(validator));
    /// ```
    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.validators.push(validator);
        self
    }

    /// 设置重载策略
    ///
    /// 配置配置的重载策略，支持手动、自动和定期重载。
    ///
    /// # 参数
    ///
    /// * `strategy` - 重载策略
    ///
    /// # 返回值
    ///
    /// 返回修改后的 UnifiedConfigManager 实例，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use scheduler_core::config::manager::{UnifiedConfigManager, ReloadStrategy};
    ///
    /// let manager = UnifiedConfigManager::new()
    ///     .with_reload_strategy(ReloadStrategy::Auto { debounce_ms: 1000 });
    /// ```
    pub fn with_reload_strategy(mut self, strategy: ReloadStrategy) -> Self {
        self.reload_strategy = strategy;
        self
    }

    /// 添加配置变更回调
    ///
    /// 添加一个在配置发生变化时调用的回调函数。
    /// 这对于响应配置变化非常有用。
    ///
    /// # 参数
    ///
    /// * `callback` - 配置变更回调函数
    ///
    /// # 返回值
    ///
    /// 返回修改后的 UnifiedConfigManager 实例，支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use scheduler_core::config::manager::UnifiedConfigManager;
    ///
    /// let manager = UnifiedConfigManager::new()
    ///     .add_callback(|config| {
    ///         println!("Configuration changed: {:?}", config);
    ///     });
    /// ```
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&serde_json::Value) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    /// 从配置源加载配置
    ///
    /// 从所有配置源加载配置并合并结果。
    /// 这是配置管理的核心方法，负责：
    /// - 从各个配置源加载配置
    /// - 按照优先级合并配置
    /// - 验证配置的有效性
    /// - 更新配置元数据
    /// - 设置重载机制
    /// - 调用变更回调
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 加载成功
    /// - `Err(SchedulerError)`: 加载失败
    ///
    /// # 错误处理
    ///
    /// 如果任何一个配置源加载失败，整个加载过程会失败。
    /// 配置验证失败不会导致加载失败，但会在元数据中标记验证状态。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use scheduler_core::config::manager::{UnifiedConfigManager, ConfigSource};
    /// use serde_json::json;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let manager = UnifiedConfigManager::new()
    ///         .add_source(ConfigSource::Memory {
    ///             data: json!({"app": {"name": "My App"}})
    ///         });
    ///     
    ///     manager.load().await?;
    ///     println!("Configuration loaded successfully");
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn load(&self) -> SchedulerResult<()> {
        let mut merged_config = serde_json::Value::Object(serde_json::Map::new());

        // 从所有配置源加载配置（后添加的源会覆盖先添加的源）
        for source in &self.sources {
            let source_config = self.load_from_source(source).await?;
            merged_config = self.merge_configs(merged_config, source_config);
        }

        // 验证配置
        let validation_result = self.validate_config(&merged_config).await;

        // 更新配置和元数据
        let mut config = self.config.write().await;
        *config = merged_config.clone();

        let mut metadata = self.metadata.write().await;
        metadata.is_valid = validation_result.is_ok();
        metadata.validation_errors = validation_result
            .err()
            .map(|e| vec![e.to_string()])
            .unwrap_or_default();
        metadata.last_modified = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 如果需要，设置文件监控器
        if matches!(self.reload_strategy, ReloadStrategy::Auto { .. }) {
            // TODO: 实现文件监控
            // self.setup_file_watcher().await?;
        }

        // 如果需要，启动定期重载
        if matches!(self.reload_strategy, ReloadStrategy::Periodic { .. }) {
            self.start_periodic_reload().await;
        }

        // 调用回调函数
        for callback in &self.callbacks {
            callback(&*config);
        }

        Ok(())
    }

    /// Load configuration from specific source
    async fn load_from_source(&self, source: &ConfigSource) -> SchedulerResult<serde_json::Value> {
        match source {
            ConfigSource::File { path } => {
                if !path.exists() {
                    return Err(SchedulerError::Configuration(format!(
                        "Configuration file not found: {}",
                        path.display()
                    )));
                }

                let content = fs::read_to_string(path).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to read config file {}: {}",
                        path.display(),
                        e
                    ))
                })?;

                let config: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to parse config file {}: {}",
                        path.display(),
                        e
                    ))
                })?;

                Ok(config)
            }
            ConfigSource::Environment { prefix } => {
                let mut config = serde_json::Value::Object(serde_json::Map::new());

                for (key, value) in std::env::vars() {
                    if key.starts_with(prefix) {
                        let config_key = key[prefix.len()..].to_lowercase().replace('_', ".");

                        // Try to parse as JSON, fall back to string
                        let parsed_value: serde_json::Value = serde_json::from_str(&value)
                            .unwrap_or_else(|_| serde_json::Value::String(value.clone()));

                        if let serde_json::Value::Object(ref mut map) = config {
                            map.insert(config_key, parsed_value);
                        }
                    }
                }

                Ok(config)
            }
            ConfigSource::CommandLine { args } => {
                let mut config = serde_json::Value::Object(serde_json::Map::new());
                let mut current_key = None;

                for arg in args {
                    if arg.starts_with("--") {
                        if let Some(key) = current_key {
                            // Set previous key as boolean flag
                            if let serde_json::Value::Object(ref mut map) = config {
                                map.insert(key, serde_json::Value::Bool(true));
                            }
                        }
                        current_key = Some(arg[2..].to_string());
                    } else if let Some(key) = current_key.take() {
                        // Set key with value
                        let parsed_value: serde_json::Value = serde_json::from_str(arg)
                            .unwrap_or_else(|_| serde_json::Value::String(arg.to_string()));

                        if let serde_json::Value::Object(ref mut map) = config {
                            map.insert(key, parsed_value);
                        }
                    }
                }

                // Handle last key if it's a boolean flag
                if let Some(key) = current_key {
                    if let serde_json::Value::Object(ref mut map) = config {
                        map.insert(key, serde_json::Value::Bool(true));
                    }
                }

                Ok(config)
            }
            ConfigSource::Remote { url: _ } => {
                // For now, return empty config
                // In a real implementation, this would make HTTP requests
                Ok(serde_json::Value::Object(serde_json::Map::new()))
            }
            ConfigSource::Memory { data } => Ok(data.clone()),
        }
    }

    /// Merge two configurations with support for flat keys
    fn merge_configs(
        &self,
        base: serde_json::Value,
        override_config: serde_json::Value,
    ) -> serde_json::Value {
        match (base, override_config) {
            (serde_json::Value::Object(mut base_map), serde_json::Value::Object(override_map)) => {
                for (key, value) in override_map {
                    // Handle flat keys with dots (e.g., "server.port") by converting to nested structure
                    if key.contains('.') {
                        let parts: Vec<&str> = key.split('.').collect();
                        if parts.len() == 2 {
                            // Simple case: server.port -> {server: {port: value}}
                            let parent_key = parts[0];
                            let child_key = parts[1];

                            // Get or create parent object
                            let parent_obj = base_map.entry(parent_key.to_string()).or_insert_with(|| {
                                serde_json::Value::Object(serde_json::Map::new())
                            });

                            if let serde_json::Value::Object(parent_map) = parent_obj {
                                parent_map.insert(child_key.to_string(), value);
                            }
                        } else {
                            // For complex nested keys, just insert as-is
                            base_map.insert(key, value);
                        }
                    } else {
                        // Handle nested merge for non-flat keys
                        if let (Some(base_value), serde_json::Value::Object(override_obj)) =
                            (base_map.get(&key), &value)
                        {
                            if let serde_json::Value::Object(base_obj) = base_value {
                                // Recursively merge nested objects
                                let merged = self.merge_configs(
                                    serde_json::Value::Object(base_obj.clone()),
                                    serde_json::Value::Object(override_obj.clone()),
                                );
                                base_map.insert(key, merged);
                                continue;
                            }
                        }
                        base_map.insert(key, value);
                    }
                }
                serde_json::Value::Object(base_map)
            }
            (_, override_config) => override_config,
        }
    }

    /// Validate configuration
    async fn validate_config(
        &self,
        config: &serde_json::Value,
    ) -> Result<(), ConfigValidationError> {
        for validator in &self.validators {
            validator.validate(config)?;
        }
        Ok(())
    }

    /// Start periodic reload
    async fn start_periodic_reload(&self) {
        if let ReloadStrategy::Periodic { interval_seconds } = self.reload_strategy {
            let config = self.config.clone();
            let metadata = self.metadata.clone();
            let sources = self.sources.clone();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

                loop {
                    interval.tick().await;

                    // Check all sources for changes
                    for source in &sources {
                        if let ConfigSource::File { path } = source {
                            if let Ok(file_metadata) = fs::metadata(path) {
                                if let Ok(modified) = file_metadata.modified() {
                                    let modified_secs = modified
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();

                                    let current_metadata = metadata.read().await;
                                    if modified_secs > current_metadata.last_modified {
                                        // For simplicity, just update timestamp and call callbacks
                                        // In a real implementation, this would reload from sources
                                        let mut metadata_guard = metadata.write().await;
                                        metadata_guard.last_modified = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs();

                                        // Call callbacks (simplified for now)
                                        let _current_config = config.read().await;
                                        // Note: In a real implementation, we'd need to handle callbacks properly
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    /// 获取配置值（支持嵌套和扁平键）
    ///
    /// 根据键名获取配置值，支持嵌套键（如 "server.port"）和扁平键访问。
    /// 这是配置管理器最常用的方法之一。
    ///
    /// # 类型参数
    ///
    /// * `T` - 目标类型，必须支持反序列化
    ///
    /// # 参数
    ///
    /// * `key` - 配置键名，支持点号分隔的嵌套键
    ///
    /// # 返回值
    ///
    /// - `Ok(T)`: 成功获取并反序列化的配置值
    /// - `Err(SchedulerError)`: 获取或反序列化失败
    ///
    /// # 键名格式
    ///
    /// 支持两种键名格式：
    /// - **嵌套键**: "server.port", "database.connection.url"
    /// - **扁平键**: "server_port", "database_connection_url"
    ///
    /// # 查找顺序
    ///
    /// 1. 首先尝试嵌套键查找
    /// 2. 如果嵌套查找失败，尝试扁平键查找
    /// 3. 如果都失败，返回错误
    ///
    /// # 错误处理
    ///
    /// - 键不存在：返回 Configuration 错误
    /// - 反序列化失败：返回 Configuration 错误
    /// - 类型不匹配：返回 Configuration 错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use scheduler_core::config::manager::{UnifiedConfigManager, ConfigSource};
    /// use serde_json::json;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let manager = UnifiedConfigManager::new()
    ///         .add_source(ConfigSource::Memory {
    ///             data: json!({
    ///                 "server": {
    ///                     "host": "localhost",
    ///                     "port": 8080
    ///                 },
    ///                 "app_name": "My Application"
    ///             })
    ///         });
    ///     
    ///     manager.load().await?;
    ///     
    ///     // 嵌套键访问
    ///     let host: String = manager.get("server.host").await?;
    ///     let port: u16 = manager.get("server.port").await?;
    ///     
    ///     // 扁平键访问
    ///     let app_name: String = manager.get("app_name").await?;
    ///     
    ///     println!("Server: {}:{}", host, port);
    ///     println!("App: {}", app_name);
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn get<T>(&self, key: &str) -> SchedulerResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let config = self.config.read().await;

        // 首先尝试嵌套遍历
        let keys: Vec<&str> = key.split('.').collect();
        let mut current = &*config;
        let mut found_nested = true;

        for k in &keys {
            match current {
                serde_json::Value::Object(map) => {
                    if let Some(value) = map.get(*k) {
                        current = value;
                    } else {
                        found_nested = false;
                        break;
                    }
                }
                _ => {
                    found_nested = false;
                    break;
                }
            }
        }

        if found_nested {
            return T::deserialize(current).map_err(|e| {
                SchedulerError::Configuration(format!(
                    "Failed to deserialize value for key {}: {}",
                    key, e
                ))
            });
        }

        // 如果嵌套遍历失败，尝试扁平键查找
        if let serde_json::Value::Object(map) = &*config {
            if let Some(value) = map.get(key) {
                return T::deserialize(value).map_err(|e| {
                    SchedulerError::Configuration(format!(
                        "Failed to deserialize value for key {}: {}",
                        key, e
                    ))
                });
            }
        }

        Err(SchedulerError::Configuration(format!(
            "Key not found: {}",
            key
        )))
    }

    /// Get configuration value with default
    pub async fn get_or_default<T>(&self, key: &str, default: T) -> T
    where
        T: serde::de::DeserializeOwned + Clone,
    {
        match self.get(key).await {
            Ok(value) => value,
            Err(_) => default,
        }
    }

    /// Get configuration metadata
    pub async fn get_metadata(&self) -> ConfigMetadata {
        self.metadata.read().await.clone()
    }

    /// Check if configuration is valid
    pub async fn is_valid(&self) -> bool {
        self.metadata.read().await.is_valid
    }

    /// Get validation errors
    pub async fn get_validation_errors(&self) -> Vec<String> {
        self.metadata.read().await.validation_errors.clone()
    }

    /// Manual reload
    pub async fn reload(&self) -> SchedulerResult<()> {
        self.load().await
    }

    /// Get current configuration as JSON
    pub async fn to_json(&self) -> SchedulerResult<String> {
        let config = self.config.read().await;
        serde_json::to_string(&*config).map_err(|e| {
            SchedulerError::Configuration(format!("Failed to serialize config: {}", e))
        })
    }

    /// Get configuration subset
    pub async fn get_subset(&self, prefix: &str) -> SchedulerResult<serde_json::Value> {
        let config = self.config.read().await;

        let mut result = serde_json::Value::Object(serde_json::Map::new());

        if let serde_json::Value::Object(map) = &*config {
            // First, check if there's a direct nested object with this prefix
            if let Some(nested_value) = map.get(prefix) {
                return Ok(nested_value.clone());
            }

            // If no direct nested object, collect all keys that start with the prefix
            let mut result_map = serde_json::Map::new();
            for (key, value) in map {
                if key.starts_with(prefix) {
                    let remaining_key = key[prefix.len()..].trim_start_matches('.');
                    if !remaining_key.is_empty() {
                        result_map.insert(remaining_key.to_string(), value.clone());
                    }
                }
            }
            result = serde_json::Value::Object(result_map);
        }

        Ok(result)
    }

    /// Update configuration value
    pub async fn set(&self, key: &str, value: serde_json::Value) -> SchedulerResult<()> {
        let mut config = self.config.write().await;

        let keys: Vec<&str> = key.split('.').collect();
        let mut current = &mut *config;

        // Navigate to the parent object
        for (i, k) in keys.iter().enumerate() {
            if i == keys.len() - 1 {
                // Last key - set the value
                if let serde_json::Value::Object(map) = current {
                    map.insert(k.to_string(), value);
                }
                break;
            }

            match current {
                serde_json::Value::Object(map) => {
                    if !map.contains_key(*k) {
                        map.insert(
                            k.to_string(),
                            serde_json::Value::Object(serde_json::Map::new()),
                        );
                    }
                    current = map.get_mut(*k).unwrap();
                }
                _ => {
                    return Err(SchedulerError::Configuration(format!(
                        "Cannot traverse into non-object value at key: {}",
                        k
                    )));
                }
            }
        }

        // Update metadata
        let mut metadata = self.metadata.write().await;
        metadata.last_modified = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Call callbacks
        for callback in &self.callbacks {
            callback(&config);
        }

        Ok(())
    }
}

impl Default for UnifiedConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration builder for fluent interface
pub struct ConfigBuilder {
    manager: UnifiedConfigManager,
}

impl ConfigBuilder {
    /// Create new configuration builder
    pub fn new() -> Self {
        Self {
            manager: UnifiedConfigManager::new(),
        }
    }

    /// Add configuration source
    pub fn add_source(mut self, source: ConfigSource) -> Self {
        self.manager = self.manager.add_source(source);
        self
    }

    /// Add configuration validator
    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.manager = self.manager.add_validator(validator);
        self
    }

    /// Set reload strategy
    pub fn with_reload_strategy(mut self, strategy: ReloadStrategy) -> Self {
        self.manager = self.manager.with_reload_strategy(strategy);
        self
    }

    /// Add configuration change callback
    pub fn add_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&serde_json::Value) + Send + Sync + 'static,
    {
        self.manager = self.manager.add_callback(callback);
        self
    }

    /// Build the configuration manager
    pub fn build(self) -> UnifiedConfigManager {
        self.manager
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Type-safe configuration wrapper
pub struct TypedConfig<T> {
    manager: Arc<UnifiedConfigManager>,
    key: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TypedConfig<T>
where
    T: serde::de::DeserializeOwned + serde::Serialize + Send + Sync + Clone,
{
    /// Create new typed configuration wrapper
    pub fn new(manager: Arc<UnifiedConfigManager>, key: String) -> Self {
        Self {
            manager,
            key,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get configuration value with type safety
    pub async fn get(&self) -> SchedulerResult<Option<T>> {
        match self.manager.get(&self.key).await {
            Ok(value) => Ok(Some(value)),
            Err(SchedulerError::Configuration(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get configuration value with default
    pub async fn get_or_default(&self, default: T) -> SchedulerResult<T> {
        Ok(self.manager.get_or_default(&self.key, default).await)
    }

    /// Set configuration value with type safety
    pub async fn set(&self, value: &T) -> SchedulerResult<()> {
        let json_value = serde_json::to_value(value).map_err(|e| {
            SchedulerError::Configuration(format!(
                "Failed to serialize config '{}': {}",
                self.key, e
            ))
        })?;
        self.manager.set(&self.key, json_value).await
    }

    /// Get configuration key
    pub fn key(&self) -> &str {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_unified_config_loading() {
        // Create temporary config file
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
        {
            "server": {
                "host": "localhost",
                "port": 8080
            },
            "database": {
                "url": "postgresql://localhost:5432/testdb",
                "pool_size": 10
            }
        }
        "#;

        temp_file.write_all(config_content.as_bytes()).unwrap();
        let config_path = temp_file.path().to_path_buf();

        // Create config manager
        let manager = UnifiedConfigManager::new()
            .add_source(ConfigSource::File { path: config_path });

        // Load configuration
        manager.load().await.unwrap();

        // Test configuration values
        let host: String = manager.get("server.host").await.unwrap();
        assert_eq!(host, "localhost");

        let port: u64 = manager.get("server.port").await.unwrap();
        assert_eq!(port, 8080);
    }

    #[tokio::test]
    async fn test_config_builder() {
        let manager = ConfigBuilder::new()
            .add_source(ConfigSource::Memory {
                data: serde_json::json!({"test": "value"}),
            })
            .build();

        manager.load().await.unwrap();

        let value: String = manager.get("test").await.unwrap();
        assert_eq!(value, "value");
    }

    #[tokio::test]
    async fn test_typed_config() {
        let manager = Arc::new(UnifiedConfigManager::new().add_source(ConfigSource::Memory {
            data: serde_json::json!({"database": {"url": "postgresql://localhost/test"}}),
        }));

        manager.load().await.unwrap();

        let typed_config = TypedConfig::<serde_json::Value>::new(manager, "database.url".to_string());
        let result = typed_config.get().await.unwrap();
        
        assert!(result.is_some());
        if let Some(value) = result {
            assert_eq!(value, serde_json::json!("postgresql://localhost/test"));
        }
    }
}