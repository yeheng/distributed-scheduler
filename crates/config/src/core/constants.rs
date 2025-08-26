// 配置常量 - 统一管理重复的配置值
// 遵循DRY原则，避免在多个配置文件中重复定义

/// 超时配置常量
pub mod timeouts {
    
    /// 默认连接超时时间(秒)
    pub const DEFAULT_CONNECTION_TIMEOUT: u64 = 30;
    
    /// 生产环境连接超时时间(秒)
    pub const PRODUCTION_CONNECTION_TIMEOUT: u64 = 60;
    
    /// 默认重试延迟时间(秒)
    pub const DEFAULT_RETRY_DELAY: u64 = 60;
    
    /// 生产环境重试延迟时间(秒)
    pub const PRODUCTION_RETRY_DELAY: u64 = 120;
    
    /// 快速重试延迟时间(秒)
    pub const FAST_RETRY_DELAY: u64 = 2;
    
    /// 最大重试尝试次数
    pub const DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 3;
    
    /// 生产环境最大重试尝试次数
    pub const PRODUCTION_MAX_RETRY_ATTEMPTS: u32 = 5;
    
    /// 默认请求超时时间(秒)
    pub const DEFAULT_REQUEST_TIMEOUT: u64 = 30;
    
    /// 生产环境请求超时时间(秒)  
    pub const PRODUCTION_REQUEST_TIMEOUT: u64 = 60;
}

/// 数据库配置常量
pub mod database {
    /// 默认最大连接数
    pub const DEFAULT_MAX_CONNECTIONS: u32 = 10;
    
    /// 生产环境最大连接数
    pub const PRODUCTION_MAX_CONNECTIONS: u32 = 50;
    
    /// 默认最小连接数
    pub const DEFAULT_MIN_CONNECTIONS: u32 = 1;
    
    /// 生产环境最小连接数
    pub const PRODUCTION_MIN_CONNECTIONS: u32 = 10;
    
    /// 默认空闲连接超时时间(秒)
    pub const DEFAULT_IDLE_TIMEOUT: u64 = 600;
    
    /// 生产环境空闲连接超时时间(秒)
    pub const PRODUCTION_IDLE_TIMEOUT: u64 = 1800;
}

/// API配置常量
pub mod api {
    /// 默认绑定地址
    pub const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
    
    /// 生产环境绑定地址
    pub const PRODUCTION_BIND_ADDRESS: &str = "0.0.0.0:8443";
    
    /// 开发环境绑定地址
    pub const DEVELOPMENT_BIND_ADDRESS: &str = "127.0.0.1:8080";
    
    /// 默认最大请求大小(MB)
    pub const DEFAULT_MAX_REQUEST_SIZE_MB: usize = 10;
    
    /// 生产环境最大请求大小(MB)
    pub const PRODUCTION_MAX_REQUEST_SIZE_MB: usize = 50;
    
    /// 默认CORS源列表
    pub const DEFAULT_CORS_ORIGINS: &[&str] = &[
        "http://localhost:3000",
        "http://localhost:8080",
        "http://127.0.0.1:3000", 
        "http://127.0.0.1:8080"
    ];
}

/// 认证配置常量
pub mod auth {
    /// 默认JWT过期时间(小时)
    pub const DEFAULT_JWT_EXPIRATION_HOURS: u64 = 24;
    
    /// 生产环境JWT过期时间(小时)
    pub const PRODUCTION_JWT_EXPIRATION_HOURS: u64 = 8;
    
    /// 默认刷新令牌过期时间(天)
    pub const DEFAULT_REFRESH_TOKEN_EXPIRATION_DAYS: u32 = 30;
    
    /// 生产环境刷新令牌过期时间(天)
    pub const PRODUCTION_REFRESH_TOKEN_EXPIRATION_DAYS: u32 = 7;
    
    /// JWT最小密钥长度
    pub const MIN_JWT_SECRET_LENGTH: usize = 32;
    
    /// 强密码最小长度
    pub const MIN_STRONG_PASSWORD_LENGTH: usize = 12;
}

/// 限流配置常量
pub mod rate_limiting {
    /// 默认每分钟最大请求数
    pub const DEFAULT_MAX_REQUESTS_PER_MINUTE: u32 = 100;
    
    /// 生产环境每分钟最大请求数
    pub const PRODUCTION_MAX_REQUESTS_PER_MINUTE: u32 = 30;
    
    /// 默认令牌桶填充速率(每秒)
    pub const DEFAULT_REFILL_RATE_PER_SECOND: f64 = 1.0;
    
    /// 生产环境令牌桶填充速率(每秒)
    pub const PRODUCTION_REFILL_RATE_PER_SECOND: f64 = 0.5;
    
    /// 默认突发请求大小
    pub const DEFAULT_BURST_SIZE: u32 = 10;
    
    /// 生产环境突发请求大小  
    pub const PRODUCTION_BURST_SIZE: u32 = 5;
}

/// 日志配置常量
pub mod logging {
    /// 各环境默认日志级别
    pub const DEVELOPMENT_LOG_LEVEL: &str = "debug";
    pub const TESTING_LOG_LEVEL: &str = "info";
    pub const STAGING_LOG_LEVEL: &str = "info";
    pub const PRODUCTION_LOG_LEVEL: &str = "warn";
    
    /// 默认监控端点
    pub const DEFAULT_METRICS_ENDPOINT: &str = "/metrics";
    
    /// 默认Jaeger端点
    pub const DEFAULT_JAEGER_ENDPOINT: &str = "http://localhost:14268/api/traces";
    pub const PRODUCTION_JAEGER_ENDPOINT: &str = "http://jaeger:14268/api/traces";
}

/// Worker配置常量
pub mod worker {
    /// 默认最大并发任务数
    pub const DEFAULT_MAX_CONCURRENT_TASKS: usize = 5;
    
    /// 生产环境最大并发任务数
    pub const PRODUCTION_MAX_CONCURRENT_TASKS: usize = 20;
    
    /// 默认心跳间隔(秒)
    pub const DEFAULT_HEARTBEAT_INTERVAL: u64 = 30;
    
    /// 生产环境心跳间隔(秒)
    pub const PRODUCTION_HEARTBEAT_INTERVAL: u64 = 60;
    
    /// 默认任务轮询间隔(秒)
    pub const DEFAULT_TASK_POLL_INTERVAL: u64 = 5;
    
    /// 生产环境任务轮询间隔(秒)
    pub const PRODUCTION_TASK_POLL_INTERVAL: u64 = 10;
    
    /// 默认支持的任务类型
    pub const DEFAULT_SUPPORTED_TASK_TYPES: &[&str] = &["shell", "http"];
    
    /// 生产环境支持的任务类型
    pub const PRODUCTION_SUPPORTED_TASK_TYPES: &[&str] = &["shell", "http", "python"];
}

/// 调度器配置常量
pub mod dispatcher {
    /// 默认调度间隔(秒)
    pub const DEFAULT_SCHEDULE_INTERVAL: u64 = 10;
    
    /// 生产环境调度间隔(秒)
    pub const PRODUCTION_SCHEDULE_INTERVAL: u64 = 30;
    
    /// 默认最大并发调度数
    pub const DEFAULT_MAX_CONCURRENT_DISPATCHES: usize = 100;
    
    /// 生产环境最大并发调度数
    pub const PRODUCTION_MAX_CONCURRENT_DISPATCHES: usize = 500;
    
    /// 默认Worker超时时间(秒)
    pub const DEFAULT_WORKER_TIMEOUT: u64 = 90;
    
    /// 生产环境Worker超时时间(秒)
    pub const PRODUCTION_WORKER_TIMEOUT: u64 = 180;
    
    /// 默认调度策略
    pub const DEFAULT_DISPATCH_STRATEGY: &str = "round_robin";
    
    /// 生产环境调度策略
    pub const PRODUCTION_DISPATCH_STRATEGY: &str = "load_balancer";
}