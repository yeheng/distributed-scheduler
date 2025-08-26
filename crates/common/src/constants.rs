//! # 系统常量定义
//! 
//! 包含分布式任务调度系统的所有常量定义

/// 系统名称
pub const SYSTEM_NAME: &str = "distributed-scheduler";

/// 系统版本
pub const SYSTEM_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 默认超时时间（秒）
pub const DEFAULT_TIMEOUT_SECONDS: u32 = 300;

/// 最大重试次数
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// 默认心跳间隔（秒）
pub const DEFAULT_HEARTBEAT_INTERVAL_SECONDS: u64 = 30;

/// Worker失联检测超时（秒）
pub const WORKER_TIMEOUT_SECONDS: u64 = 90;

/// 任务执行状态检查间隔（秒）
pub const TASK_STATUS_CHECK_INTERVAL_SECONDS: u64 = 5;

/// 默认数据库连接池大小
pub const DEFAULT_DB_POOL_SIZE: u32 = 10;

/// 默认消息队列连接池大小
pub const DEFAULT_MQ_POOL_SIZE: u32 = 5;

/// 默认API服务端口
pub const DEFAULT_API_PORT: u16 = 8080;

/// 默认Worker端口
pub const DEFAULT_WORKER_PORT: u16 = 8081;

/// 默认调度器端口
pub const DEFAULT_DISPATCHER_PORT: u16 = 8082;

/// JWT Token默认过期时间（小时）
pub const DEFAULT_JWT_EXPIRATION_HOURS: i64 = 24;

/// API密钥最小长度
pub const MIN_API_KEY_LENGTH: usize = 32;

/// 任务名称最大长度
pub const MAX_TASK_NAME_LENGTH: usize = 255;

/// Worker ID最大长度
pub const MAX_WORKER_ID_LENGTH: usize = 64;

/// 任务参数JSON最大大小（字节）
pub const MAX_TASK_PARAMS_SIZE: usize = 1024 * 1024; // 1MB

/// 错误消息最大长度
pub const MAX_ERROR_MESSAGE_LENGTH: usize = 2048;

/// 默认配置文件路径
pub const DEFAULT_CONFIG_PATH: &str = "config";

/// 环境变量前缀
pub const ENV_PREFIX: &str = "SCHEDULER";

/// 日志级别环境变量名
pub const LOG_LEVEL_ENV: &str = "SCHEDULER_LOG_LEVEL";

/// 数据库URL环境变量名
pub const DATABASE_URL_ENV: &str = "SCHEDULER_DATABASE_URL";

/// 消息队列URL环境变量名
pub const MESSAGE_QUEUE_URL_ENV: &str = "SCHEDULER_MESSAGE_QUEUE_URL";

/// HTTP请求默认用户代理
pub const DEFAULT_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"), 
    "/", 
    env!("CARGO_PKG_VERSION")
);

/// 支持的Cron表达式字段数
pub const CRON_FIELDS_COUNT: usize = 6;

/// 任务依赖最大深度
pub const MAX_DEPENDENCY_DEPTH: usize = 10;

/// 分片任务最大分片数
pub const MAX_SHARD_COUNT: u32 = 1000;

/// 默认批处理大小
pub const DEFAULT_BATCH_SIZE: usize = 100;