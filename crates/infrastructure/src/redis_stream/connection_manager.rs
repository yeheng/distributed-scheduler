use redis::{Client, Connection, RedisResult};
use scheduler_core::{errors::SchedulerError, SchedulerResult};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, warn};

use super::config::RedisStreamConfig;
use super::metrics_collector::RedisStreamMetrics;

/// Redis连接管理器
///
/// 负责管理到Redis服务器的连接，包括连接建立、重试机制和错误处理
pub struct RedisConnectionManager {
    client: Client,
    config: RedisStreamConfig,
    metrics: Arc<RedisStreamMetrics>,
}

impl RedisConnectionManager {
    /// 创建新的连接管理器
    pub async fn new(config: RedisStreamConfig) -> SchedulerResult<Self> {
        let redis_url = config.build_connection_url();
        let client = Client::open(redis_url).map_err(|e| {
            SchedulerError::MessageQueue(format!("Failed to create Redis client: {e}"))
        })?;

        let manager = Self {
            client,
            config,
            metrics: Arc::new(RedisStreamMetrics::default()),
        };

        // 测试连接
        manager.test_connection().await?;
        debug!("Successfully connected to Redis at {}:{}", manager.config.host, manager.config.port);

        Ok(manager)
    }

    /// 设置指标收集器
    pub fn set_metrics(&mut self, metrics: Arc<RedisStreamMetrics>) {
        self.metrics = metrics;
    }

    /// 获取Redis连接
    pub async fn get_connection(&self) -> SchedulerResult<Connection> {
        self.get_connection_with_retry().await
    }

    /// 带重试机制的连接获取
    async fn get_connection_with_retry(&self) -> SchedulerResult<Connection> {
        let mut last_error = None;

        for attempt in 0..self.config.max_retry_attempts {
            match self.client.get_connection() {
                Ok(conn) => {
                    if attempt > 0 {
                        debug!("Successfully reconnected to Redis after {} attempts", attempt + 1);
                    }
                    self.metrics.set_active_connections(1);
                    return Ok(conn);
                }
                Err(e) => {
                    last_error = Some(e);
                    self.metrics.record_connection_error();
                    
                    if attempt < self.config.max_retry_attempts - 1 {
                        warn!(
                            "Failed to connect to Redis (attempt {}/{}): {}. Retrying in {}s...",
                            attempt + 1,
                            self.config.max_retry_attempts,
                            last_error.as_ref().unwrap(),
                            self.config.retry_delay_seconds
                        );
                        sleep(Duration::from_secs(self.config.retry_delay_seconds)).await;
                    }
                }
            }
        }

        let error_msg = format!(
            "Failed to connect to Redis after {} attempts. Last error: {}",
            self.config.max_retry_attempts,
            last_error.map_or("Unknown".to_string(), |e| e.to_string())
        );
        error!("{}", error_msg);
        Err(SchedulerError::MessageQueue(error_msg))
    }

    /// 测试连接
    async fn test_connection(&self) -> SchedulerResult<()> {
        let mut conn = self.get_connection().await?;
        
        // 执行PING命令测试连接
        let result: RedisResult<String> = redis::cmd("PING").query(&mut conn);
        match result {
            Ok(response) if response == "PONG" => {
                debug!("Redis connection test successful");
                Ok(())
            }
            Ok(response) => {
                let error_msg = format!("Unexpected PING response: {}", response);
                error!("{}", error_msg);
                Err(SchedulerError::MessageQueue(error_msg))
            }
            Err(e) => {
                let error_msg = format!("Redis PING failed: {}", e);
                error!("{}", error_msg);
                Err(SchedulerError::MessageQueue(error_msg))
            }
        }
    }

    /// 执行Redis命令的通用方法
    pub async fn execute_command<T: redis::FromRedisValue>(
        &self,
        cmd: &mut redis::Cmd,
    ) -> SchedulerResult<T> {
        let mut conn = self.get_connection().await?;
        cmd.query(&mut conn).map_err(|e| {
            self.metrics.record_connection_error();
            SchedulerError::MessageQueue(format!("Redis command failed: {e}"))
        })
    }

    /// 检查连接健康状态
    pub async fn health_check(&self) -> bool {
        match self.test_connection().await {
            Ok(()) => true,
            Err(e) => {
                warn!("Redis health check failed: {}", e);
                false
            }
        }
    }

    /// Ping Redis服务器
    pub async fn ping(&self) -> SchedulerResult<()> {
        self.test_connection().await
    }
}