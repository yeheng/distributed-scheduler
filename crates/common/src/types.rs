//! # 共享类型定义
//!
//! 包含系统中常用的类型别名和基础枚举类型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 任务ID类型
pub type TaskId = i64;

/// 任务运行ID类型
pub type TaskRunId = i64;

/// Worker ID类型
pub type WorkerId = String;

/// 任务类型名称
pub type TaskType = String;

/// 任务参数类型
pub type TaskParams = serde_json::Value;

/// 任务结果类型
pub type TaskResult = serde_json::Value;

/// 时间戳类型
pub type Timestamp = DateTime<Utc>;

/// 标签集合类型
pub type Labels = HashMap<String, String>;

/// 元数据类型
pub type Metadata = HashMap<String, serde_json::Value>;

/// 系统环境枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    /// 开发环境
    Development,
    /// 测试环境
    Testing,
    /// 预发环境
    Staging,
    /// 生产环境
    Production,
}

impl Default for Environment {
    fn default() -> Self {
        Self::Development
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::Testing => write!(f, "testing"),
            Self::Staging => write!(f, "staging"),
            Self::Production => write!(f, "production"),
        }
    }
}

impl std::str::FromStr for Environment {
    type Err = scheduler_errors::SchedulerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "testing" | "test" => Ok(Self::Testing),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(scheduler_errors::SchedulerError::Configuration(format!(
                "Invalid environment: {}",
                s
            ))),
        }
    }
}

/// 优先级枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// 低优先级
    Low = 1,
    /// 普通优先级
    Normal = 2,
    /// 高优先级
    High = 3,
    /// 紧急优先级
    Critical = 4,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// 健康状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 警告
    Warning,
    /// 不健康
    Unhealthy,
    /// 未知状态
    Unknown,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// 负载均衡策略枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalanceStrategy {
    /// 轮询
    RoundRobin,
    /// 最少连接
    LeastConnections,
    /// 加权轮询
    WeightedRoundRobin,
    /// 随机选择
    Random,
    /// 最少负载
    LeastLoad,
}

impl Default for LoadBalanceStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

/// 资源使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU使用率（百分比）
    pub cpu_percent: f64,
    /// 内存使用量（字节）
    pub memory_bytes: u64,
    /// 内存使用率（百分比）
    pub memory_percent: f64,
    /// 磁盘使用量（字节）
    pub disk_bytes: u64,
    /// 网络接收字节数
    pub network_in_bytes: u64,
    /// 网络发送字节数
    pub network_out_bytes: u64,
    /// 统计时间
    pub timestamp: Timestamp,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_bytes: 0,
            memory_percent: 0.0,
            disk_bytes: 0,
            network_in_bytes: 0,
            network_out_bytes: 0,
            timestamp: Utc::now(),
        }
    }
}

/// 分片信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardInfo {
    /// 分片索引（从0开始）
    pub index: u32,
    /// 总分片数
    pub total: u32,
}

impl ShardInfo {
    /// 创建新的分片信息
    pub fn new(index: u32, total: u32) -> Self {
        Self { index, total }
    }

    /// 检查分片信息是否有效
    pub fn is_valid(&self) -> bool {
        self.index < self.total && self.total > 0
    }
}

/// 版本信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    /// 版本号
    pub version: String,
    /// 构建时间
    pub build_time: String,
    /// Git提交哈希
    pub git_hash: Option<String>,
    /// 构建环境
    pub build_env: Environment,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_time: chrono::Utc::now().to_rfc3339(),
            git_hash: option_env!("GIT_HASH").map(|s| s.to_string()),
            build_env: Environment::Development,
        }
    }
}
