//! # 共享Trait定义
//! 
//! 包含系统中常用的trait定义

use async_trait::async_trait;
use scheduler_errors::SchedulerResult;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::types::{HealthStatus, ResourceUsage, Timestamp, VersionInfo};

/// 可序列化的trait，组合了常用的序列化trait
pub trait Serializable: Debug + Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync {}

/// 为所有满足条件的类型自动实现Serializable
impl<T> Serializable for T where T: Debug + Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync {}

/// 健康检查trait
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// 执行健康检查
    async fn health_check(&self) -> SchedulerResult<HealthStatus>;

    /// 获取详细的健康状态信息
    async fn detailed_health(&self) -> SchedulerResult<serde_json::Value> {
        let status = self.health_check().await?;
        Ok(serde_json::json!({
            "status": status,
            "timestamp": chrono::Utc::now(),
        }))
    }
}

/// 生命周期管理trait
#[async_trait]
pub trait Lifecycle: Send + Sync {
    /// 启动组件
    async fn start(&mut self) -> SchedulerResult<()>;

    /// 停止组件
    async fn stop(&mut self) -> SchedulerResult<()>;

    /// 重启组件
    async fn restart(&mut self) -> SchedulerResult<()> {
        self.stop().await?;
        self.start().await
    }

    /// 检查组件是否正在运行
    fn is_running(&self) -> bool;
}

/// 资源监控trait
#[async_trait]
pub trait ResourceMonitor: Send + Sync {
    /// 获取当前资源使用情况
    async fn get_resource_usage(&self) -> SchedulerResult<ResourceUsage>;

    /// 获取资源使用历史
    async fn get_resource_history(&self, duration_seconds: u64) -> SchedulerResult<Vec<ResourceUsage>>;
}

/// 可配置trait
pub trait Configurable<T>: Send + Sync {
    /// 获取当前配置
    fn get_config(&self) -> &T;

    /// 更新配置
    fn update_config(&mut self, config: T) -> SchedulerResult<()>;

    /// 验证配置
    fn validate_config(&self, config: &T) -> SchedulerResult<()>;
}

/// 可观测trait
#[async_trait]
pub trait Observable: Send + Sync {
    /// 获取指标数据
    async fn get_metrics(&self) -> SchedulerResult<serde_json::Value>;

    /// 记录事件
    async fn record_event(&self, event: &str, metadata: Option<serde_json::Value>) -> SchedulerResult<()>;
}

/// 可缓存trait
#[async_trait]
pub trait Cacheable<K, V>: Send + Sync 
where 
    K: Send + Sync + Clone + std::hash::Hash + Eq,
    V: Send + Sync + Clone,
{
    /// 获取缓存值
    async fn get(&self, key: &K) -> SchedulerResult<Option<V>>;

    /// 设置缓存值
    async fn set(&self, key: K, value: V, ttl_seconds: Option<u64>) -> SchedulerResult<()>;

    /// 删除缓存值
    async fn delete(&self, key: &K) -> SchedulerResult<bool>;

    /// 清空缓存
    async fn clear(&self) -> SchedulerResult<()>;

    /// 获取缓存大小
    async fn size(&self) -> SchedulerResult<usize>;
}

/// 可重试trait
#[async_trait]
pub trait Retryable: Send + Sync {
    type Input: Send + Sync + Clone;
    type Output: Send + Sync;
    type Error: Send + Sync + std::error::Error;

    /// 执行操作
    async fn execute(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;

    /// 获取最大重试次数
    fn max_retries(&self) -> u32;

    /// 获取重试间隔（毫秒）
    fn retry_interval_ms(&self) -> u64;

    /// 是否应该重试
    fn should_retry(&self, error: &Self::Error, attempt: u32) -> bool;

    /// 带重试的执行
    async fn execute_with_retry(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        let mut last_error = None;
        
        for attempt in 0..=self.max_retries() {
            match self.execute(input.clone()).await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    last_error = Some(error);
                    
                    if attempt < self.max_retries() {
                        let should_retry = self.should_retry(last_error.as_ref().unwrap(), attempt);
                        if should_retry {
                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                self.retry_interval_ms()
                            )).await;
                            continue;
                        }
                    }
                    break;
                }
            }
        }

        Err(last_error.unwrap())
    }
}

/// 版本信息提供者trait
pub trait VersionProvider: Send + Sync {
    /// 获取版本信息
    fn get_version_info(&self) -> VersionInfo;
}

/// 标识符生成器trait
pub trait IdGenerator<T>: Send + Sync {
    /// 生成新的标识符
    fn generate(&self) -> T;

    /// 验证标识符格式
    fn validate(&self, id: &T) -> bool;
}

/// 时间提供者trait（便于测试）
pub trait TimeProvider: Send + Sync {
    /// 获取当前时间
    fn now(&self) -> Timestamp;
}

/// 默认时间提供者
#[derive(Debug, Clone, Default)]
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn now(&self) -> Timestamp {
        chrono::Utc::now()
    }
}

/// 可序列化和可克隆的trait组合
pub trait SerializableClone: Serializable + Clone {}
impl<T> SerializableClone for T where T: Serializable + Clone {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestStruct {
        id: u64,
        name: String,
    }

    #[test]
    fn test_serializable_trait() {
        let test = TestStruct {
            id: 1,
            name: "test".to_string(),
        };

        // 测试是否正确实现了Serializable
        fn check_serializable<T: Serializable>(_t: T) {}
        check_serializable(test);
    }

    #[test]
    fn test_time_provider() {
        let provider = SystemTimeProvider::default();
        let now1 = provider.now();
        let now2 = provider.now();
        
        // 时间应该是递增的（虽然可能相等）
        assert!(now2 >= now1);
    }
}