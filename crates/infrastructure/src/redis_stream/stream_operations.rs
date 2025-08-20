use scheduler_errors::SchedulerError;
use scheduler_errors::SchedulerResult;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

use super::config::RedisStreamConfig;
use super::connection_manager::RedisConnectionManager;
use super::metrics_collector::RedisStreamMetrics;

pub struct RedisStreamOperations {
    connection_manager: Arc<RedisConnectionManager>,
    config: RedisStreamConfig,
    metrics: Arc<RedisStreamMetrics>,
}

impl RedisStreamOperations {
    pub fn new(
        connection_manager: Arc<RedisConnectionManager>,
        config: RedisStreamConfig,
        metrics: Arc<RedisStreamMetrics>,
    ) -> Self {
        Self {
            connection_manager,
            config,
            metrics,
        }
    }
    pub async fn create_queue(&self, queue_name: &str, durable: bool) -> SchedulerResult<()> {
        let start = Instant::now();
        debug!("Creating queue: {} (durable: {})", queue_name, durable);
        self.ensure_stream_exists(queue_name).await?;
        let group_name = self.get_consumer_group_name(queue_name);
        self.ensure_consumer_group_exists(queue_name, &group_name)
            .await?;

        let duration = start.elapsed();
        self.metrics
            .record_operation_duration("create_queue", duration.as_millis() as f64);

        info!(
            "Successfully created queue: {} in {:?}",
            queue_name, duration
        );
        Ok(())
    }
    pub async fn delete_queue(&self, queue_name: &str) -> SchedulerResult<()> {
        let start = Instant::now();
        debug!("Deleting queue: {}", queue_name);
        let group_name = self.get_consumer_group_name(queue_name);
        if let Err(e) = self.delete_consumer_group(queue_name, &group_name).await {
            warn!("Failed to delete consumer group {}: {}", group_name, e);
        }
        let mut cmd = redis::cmd("DEL");
        cmd.arg(queue_name);
        let deleted_count: i64 = self.connection_manager.execute_command(&mut cmd).await?;

        let duration = start.elapsed();
        self.metrics
            .record_operation_duration("delete_queue", duration.as_millis() as f64);

        if deleted_count > 0 {
            info!(
                "Successfully deleted queue: {} in {:?}",
                queue_name, duration
            );
        } else {
            warn!("Queue {} was not found or already deleted", queue_name);
        }

        Ok(())
    }
    pub async fn get_queue_size(&self, queue_name: &str) -> SchedulerResult<u64> {
        let start = Instant::now();
        debug!("Getting size of queue: {}", queue_name);

        let mut cmd = redis::cmd("XLEN");
        cmd.arg(queue_name);
        let size: u64 = self.connection_manager.execute_command(&mut cmd).await?;

        let duration = start.elapsed();
        self.metrics
            .record_operation_duration("get_queue_size", duration.as_millis() as f64);

        debug!(
            "Queue {} has {} messages, query took {:?}",
            queue_name, size, duration
        );
        Ok(size)
    }
    pub async fn purge_queue(&self, queue_name: &str) -> SchedulerResult<u64> {
        let start = Instant::now();
        debug!("Purging queue: {}", queue_name);
        let original_size = self.get_queue_size(queue_name).await?;
        let mut cmd = redis::cmd("DEL");
        cmd.arg(queue_name);
        let _: i64 = self.connection_manager.execute_command(&mut cmd).await?;
        self.ensure_stream_exists(queue_name).await?;
        let group_name = self.get_consumer_group_name(queue_name);
        self.ensure_consumer_group_exists(queue_name, &group_name)
            .await?;

        let duration = start.elapsed();
        self.metrics
            .record_operation_duration("purge_queue", duration.as_millis() as f64);

        info!(
            "Successfully purged queue: {}, removed {} messages in {:?}",
            queue_name, original_size, duration
        );
        Ok(original_size)
    }
    pub async fn ensure_stream_exists(&self, stream_name: &str) -> SchedulerResult<()> {
        debug!("Ensuring stream exists: {}", stream_name);
        let mut cmd = redis::cmd("XINFO");
        cmd.arg("STREAM").arg(stream_name);

        match self
            .connection_manager
            .execute_command::<redis::Value>(&mut cmd)
            .await
        {
            Ok(_) => {
                debug!("Stream {} already exists", stream_name);
                Ok(())
            }
            Err(_) => {
                debug!("Stream {} does not exist, creating it", stream_name);
                self.create_empty_stream(stream_name).await
            }
        }
    }
    pub async fn ensure_consumer_group_exists(
        &self,
        stream_name: &str,
        group_name: &str,
    ) -> SchedulerResult<()> {
        debug!(
            "Ensuring consumer group exists: {} for stream: {}",
            group_name, stream_name
        );

        let mut cmd = redis::cmd("XGROUP");
        cmd.arg("CREATE")
            .arg(stream_name)
            .arg(group_name)
            .arg("0") // 从Stream开始读取
            .arg("MKSTREAM"); // 如果Stream不存在则创建

        match self
            .connection_manager
            .execute_command::<String>(&mut cmd)
            .await
        {
            Ok(_) => {
                debug!("Successfully created consumer group: {}", group_name);
                Ok(())
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("BUSYGROUP") {
                    debug!("Consumer group {} already exists", group_name);
                    Ok(())
                } else {
                    Err(SchedulerError::MessageQueue(format!(
                        "Failed to create consumer group {group_name}: {e}"
                    )))
                }
            }
        }
    }
    fn get_consumer_group_name(&self, queue_name: &str) -> String {
        format!("{}_{}", self.config.consumer_group_prefix, queue_name)
    }

    async fn create_empty_stream(&self, stream_name: &str) -> SchedulerResult<()> {
        let mut cmd = redis::cmd("XADD");
        cmd.arg(stream_name).arg("*").arg("temp").arg("value");
        let temp_id: String = self.connection_manager.execute_command(&mut cmd).await?;
        let mut del_cmd = redis::cmd("XDEL");
        del_cmd.arg(stream_name).arg(&temp_id);
        let _: i64 = self
            .connection_manager
            .execute_command(&mut del_cmd)
            .await?;

        debug!("Created empty stream: {}", stream_name);
        Ok(())
    }

    async fn delete_consumer_group(
        &self,
        stream_name: &str,
        group_name: &str,
    ) -> SchedulerResult<()> {
        let mut cmd = redis::cmd("XGROUP");
        cmd.arg("DESTROY").arg(stream_name).arg(group_name);

        let destroyed: i64 = self.connection_manager.execute_command(&mut cmd).await?;

        if destroyed > 0 {
            debug!("Successfully deleted consumer group: {}", group_name);
        } else {
            debug!(
                "Consumer group {} was not found or already deleted",
                group_name
            );
        }

        Ok(())
    }
}
