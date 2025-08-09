use redis::{Client, Connection, RedisResult};
use scheduler_core::{errors::SchedulerError, SchedulerResult};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, warn};

use super::config::RedisStreamConfig;
use super::metrics_collector::RedisStreamMetrics;

pub struct RedisConnectionManager {
    client: Client,
    config: RedisStreamConfig,
    metrics: Arc<RedisStreamMetrics>,
}

impl RedisConnectionManager {
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
        manager.test_connection().await?;
        debug!(
            "Successfully connected to Redis at {}:{}",
            manager.config.host, manager.config.port
        );

        Ok(manager)
    }
    pub fn set_metrics(&mut self, metrics: Arc<RedisStreamMetrics>) {
        self.metrics = metrics;
    }
    pub async fn get_connection(&self) -> SchedulerResult<Connection> {
        self.get_connection_with_retry().await
    }
    async fn get_connection_with_retry(&self) -> SchedulerResult<Connection> {
        let mut last_error = None;

        for attempt in 0..self.config.max_retry_attempts {
            match self.client.get_connection() {
                Ok(conn) => {
                    if attempt > 0 {
                        debug!(
                            "Successfully reconnected to Redis after {} attempts",
                            attempt + 1
                        );
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
    async fn test_connection(&self) -> SchedulerResult<()> {
        let mut conn = self.get_connection().await?;
        let result: RedisResult<String> = redis::cmd("PING").query(&mut conn);
        match result {
            Ok(response) if response == "PONG" => {
                debug!("Redis connection test successful");
                Ok(())
            }
            Ok(response) => {
                let error_msg = format!("Unexpected PING response: {response}");
                error!("{}", error_msg);
                Err(SchedulerError::MessageQueue(error_msg))
            }
            Err(e) => {
                let error_msg = format!("Redis PING failed: {e}");
                error!("{}", error_msg);
                Err(SchedulerError::MessageQueue(error_msg))
            }
        }
    }
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
    pub async fn health_check(&self) -> bool {
        match self.test_connection().await {
            Ok(()) => true,
            Err(e) => {
                warn!("Redis health check failed: {}", e);
                false
            }
        }
    }
    pub async fn ping(&self) -> SchedulerResult<()> {
        self.test_connection().await
    }
}
