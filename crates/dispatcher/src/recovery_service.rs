use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tracing::{debug, error, info, warn};

use scheduler_core::{traits::MessageQueue, SchedulerError, SchedulerResult};
use scheduler_domain::entities::{TaskRun, TaskRunStatus, WorkerStatus};
use scheduler_domain::repositories::{TaskRunRepository, WorkerRepository};

#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub db_max_retry_attempts: u32,
    pub db_retry_interval_seconds: u64,
    pub mq_max_retry_attempts: u32,
    pub mq_retry_interval_seconds: u64,
    pub startup_recovery_timeout_seconds: u64,
    pub worker_heartbeat_timeout_seconds: i64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            db_max_retry_attempts: 5,
            db_retry_interval_seconds: 10,
            mq_max_retry_attempts: 5,
            mq_retry_interval_seconds: 10,
            startup_recovery_timeout_seconds: 300, // 5分钟
            worker_heartbeat_timeout_seconds: 90,  // 90秒
        }
    }
}

#[async_trait]
pub trait RecoveryService: Send + Sync {
    async fn recover_system_state(&self) -> SchedulerResult<RecoveryReport>;
    async fn recover_interrupted_tasks(&self) -> SchedulerResult<Vec<TaskRun>>;
    async fn recover_worker_states(&self) -> SchedulerResult<Vec<String>>;
    async fn reconnect_database(&self) -> SchedulerResult<()>;
    async fn reconnect_message_queue(&self) -> SchedulerResult<()>;
    async fn check_system_health(&self) -> SchedulerResult<SystemHealthStatus>;
}

#[derive(Debug, Clone)]
pub struct RecoveryReport {
    pub recovered_tasks: Vec<TaskRun>,
    pub failed_workers: Vec<String>,
    pub recovery_duration_ms: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SystemHealthStatus {
    pub database_healthy: bool,
    pub message_queue_healthy: bool,
    pub active_workers: u32,
    pub pending_tasks: u32,
    pub running_tasks: u32,
    pub last_check_time: DateTime<Utc>,
}

pub struct SystemRecoveryService {
    task_run_repo: Arc<dyn TaskRunRepository>,
    worker_repo: Arc<dyn WorkerRepository>,
    message_queue: Arc<dyn MessageQueue>,
    config: RecoveryConfig,
}

impl SystemRecoveryService {
    pub fn new(
        task_run_repo: Arc<dyn TaskRunRepository>,
        worker_repo: Arc<dyn WorkerRepository>,
        message_queue: Arc<dyn MessageQueue>,
        config: Option<RecoveryConfig>,
    ) -> Self {
        Self {
            task_run_repo,
            worker_repo,
            message_queue,
            config: config.unwrap_or_default(),
        }
    }
    async fn recover_running_tasks(&self) -> SchedulerResult<Vec<TaskRun>> {
        info!("开始恢复运行中的任务状态");

        let running_tasks = self.task_run_repo.get_running_runs().await?;
        let mut recovered_tasks = Vec::new();

        for task_run in running_tasks {
            if let Some(worker_id) = &task_run.worker_id {
                match self.worker_repo.get_by_id(worker_id).await? {
                    Some(worker) => {
                        if worker.status == WorkerStatus::Alive {
                            let now = Utc::now();
                            let time_since_heartbeat = now - worker.last_heartbeat;

                            if time_since_heartbeat.num_seconds()
                                > self.config.worker_heartbeat_timeout_seconds
                            {
                                warn!(
                                    "任务运行 {} 的Worker {} 心跳超时，将任务标记为失败",
                                    task_run.id, worker_id
                                );

                                self.task_run_repo
                                    .update_status(task_run.id, TaskRunStatus::Failed, None)
                                    .await?;

                                self.task_run_repo
                                    .update_result(
                                        task_run.id,
                                        None,
                                        Some("系统恢复时发现Worker心跳超时"),
                                    )
                                    .await?;

                                recovered_tasks.push(task_run);
                            } else {
                                debug!(
                                    "任务运行 {} 的Worker {} 状态正常，继续运行",
                                    task_run.id, worker_id
                                );
                            }
                        } else {
                            warn!(
                                "任务运行 {} 的Worker {} 已离线，将任务标记为失败",
                                task_run.id, worker_id
                            );

                            self.task_run_repo
                                .update_status(task_run.id, TaskRunStatus::Failed, None)
                                .await?;

                            self.task_run_repo
                                .update_result(
                                    task_run.id,
                                    None,
                                    Some("系统恢复时发现Worker已离线"),
                                )
                                .await?;

                            recovered_tasks.push(task_run);
                        }
                    }
                    None => {
                        warn!(
                            "任务运行 {} 的Worker {} 不存在，将任务标记为失败",
                            task_run.id, worker_id
                        );

                        self.task_run_repo
                            .update_status(task_run.id, TaskRunStatus::Failed, None)
                            .await?;

                        self.task_run_repo
                            .update_result(task_run.id, None, Some("系统恢复时发现Worker不存在"))
                            .await?;

                        recovered_tasks.push(task_run);
                    }
                }
            } else {
                warn!(
                    "任务运行 {} 没有Worker ID但状态为运行中，将标记为失败",
                    task_run.id
                );

                self.task_run_repo
                    .update_status(task_run.id, TaskRunStatus::Failed, None)
                    .await?;

                self.task_run_repo
                    .update_result(task_run.id, None, Some("系统恢复时发现任务没有分配Worker"))
                    .await?;

                recovered_tasks.push(task_run);
            }
        }

        info!(
            "完成运行中任务状态恢复，处理了 {} 个任务",
            recovered_tasks.len()
        );
        Ok(recovered_tasks)
    }
    async fn recover_dispatched_tasks(&self) -> SchedulerResult<Vec<TaskRun>> {
        info!("开始恢复已分发但未开始的任务");

        let dispatched_tasks = self
            .task_run_repo
            .get_by_status(TaskRunStatus::Dispatched)
            .await?;
        let mut recovered_tasks = Vec::new();

        for task_run in dispatched_tasks {
            let now = Utc::now();
            let time_since_created = now - task_run.created_at;

            if time_since_created.num_seconds()
                > self.config.startup_recovery_timeout_seconds as i64
            {
                warn!(
                    "任务运行 {} 已分发超过 {} 秒但未开始执行，将重新设置为待调度状态",
                    task_run.id, self.config.startup_recovery_timeout_seconds
                );
                self.task_run_repo
                    .update_status(task_run.id, TaskRunStatus::Pending, None)
                    .await?;

                recovered_tasks.push(task_run);
            } else {
                debug!("任务运行 {} 分发时间正常，保持已分发状态", task_run.id);
            }
        }

        info!(
            "完成已分发任务状态恢复，处理了 {} 个任务",
            recovered_tasks.len()
        );
        Ok(recovered_tasks)
    }
    async fn attempt_database_reconnection(&self) -> SchedulerResult<()> {
        info!("开始尝试数据库重连");

        for attempt in 1..=self.config.db_max_retry_attempts {
            info!(
                "数据库重连尝试 {}/{}",
                attempt, self.config.db_max_retry_attempts
            );
            match self.task_run_repo.get_pending_runs(Some(1)).await {
                Ok(_) => {
                    info!("数据库重连成功");
                    return Ok(());
                }
                Err(e) => {
                    error!("数据库重连尝试 {} 失败: {}", attempt, e);

                    if attempt < self.config.db_max_retry_attempts {
                        info!("等待 {} 秒后重试", self.config.db_retry_interval_seconds);
                        tokio::time::sleep(Duration::from_secs(
                            self.config.db_retry_interval_seconds,
                        ))
                        .await;
                    }
                }
            }
        }

        Err(SchedulerError::DatabaseOperation(
            "数据库重连失败，已达到最大重试次数".to_string(),
        ))
    }
    async fn attempt_message_queue_reconnection(&self) -> SchedulerResult<()> {
        info!("开始尝试消息队列重连");

        for attempt in 1..=self.config.mq_max_retry_attempts {
            info!(
                "消息队列重连尝试 {}/{}",
                attempt, self.config.mq_max_retry_attempts
            );
            match self.message_queue.get_queue_size("test_connection").await {
                Ok(_) => {
                    info!("消息队列重连成功");
                    return Ok(());
                }
                Err(e) => {
                    error!("消息队列重连尝试 {} 失败: {}", attempt, e);

                    if attempt < self.config.mq_max_retry_attempts {
                        info!("等待 {} 秒后重试", self.config.mq_retry_interval_seconds);
                        tokio::time::sleep(Duration::from_secs(
                            self.config.mq_retry_interval_seconds,
                        ))
                        .await;
                    }
                }
            }
        }

        Err(SchedulerError::MessageQueue(
            "消息队列重连失败，已达到最大重试次数".to_string(),
        ))
    }
}

#[async_trait]
impl RecoveryService for SystemRecoveryService {
    async fn recover_system_state(&self) -> SchedulerResult<RecoveryReport> {
        info!("开始系统状态恢复");
        let start_time = std::time::Instant::now();
        let mut errors = Vec::new();
        let mut all_recovered_tasks = Vec::new();
        match self.recover_running_tasks().await {
            Ok(mut tasks) => {
                all_recovered_tasks.append(&mut tasks);
            }
            Err(e) => {
                let error_msg = format!("恢复运行中任务失败: {e}");
                error!("{}", error_msg);
                errors.push(error_msg);
            }
        }
        match self.recover_dispatched_tasks().await {
            Ok(mut tasks) => {
                all_recovered_tasks.append(&mut tasks);
            }
            Err(e) => {
                let error_msg = format!("恢复已分发任务失败: {e}");
                error!("{}", error_msg);
                errors.push(error_msg);
            }
        }
        let failed_workers = match self.recover_worker_states().await {
            Ok(workers) => workers,
            Err(e) => {
                let error_msg = format!("恢复Worker状态失败: {e}");
                error!("{}", error_msg);
                errors.push(error_msg);
                Vec::new()
            }
        };

        let recovery_duration = start_time.elapsed();
        let report = RecoveryReport {
            recovered_tasks: all_recovered_tasks,
            failed_workers,
            recovery_duration_ms: recovery_duration.as_millis() as u64,
            errors,
        };

        info!(
            "系统状态恢复完成，耗时 {}ms，恢复任务 {} 个，失效Worker {} 个",
            report.recovery_duration_ms,
            report.recovered_tasks.len(),
            report.failed_workers.len()
        );

        Ok(report)
    }
    async fn recover_interrupted_tasks(&self) -> SchedulerResult<Vec<TaskRun>> {
        info!("开始恢复中断的任务");

        let mut interrupted_tasks = Vec::new();
        let mut running_tasks = self.recover_running_tasks().await?;
        interrupted_tasks.append(&mut running_tasks);
        let mut dispatched_tasks = self.recover_dispatched_tasks().await?;
        interrupted_tasks.append(&mut dispatched_tasks);

        info!(
            "完成中断任务恢复，共处理 {} 个任务",
            interrupted_tasks.len()
        );
        Ok(interrupted_tasks)
    }
    async fn recover_worker_states(&self) -> SchedulerResult<Vec<String>> {
        info!("开始恢复Worker状态");

        let all_workers = self.worker_repo.list().await?;
        let mut failed_workers = Vec::new();
        let now = Utc::now();

        for worker in all_workers {
            let time_since_heartbeat = now - worker.last_heartbeat;

            if time_since_heartbeat.num_seconds() > self.config.worker_heartbeat_timeout_seconds {
                if worker.status == WorkerStatus::Alive {
                    warn!(
                        "Worker {} 心跳超时 {} 秒，将标记为Down状态",
                        worker.id,
                        time_since_heartbeat.num_seconds()
                    );
                    if let Err(e) = self
                        .worker_repo
                        .update_status(&worker.id, WorkerStatus::Down)
                        .await
                    {
                        error!("更新Worker {} 状态失败: {}", worker.id, e);
                    } else {
                        failed_workers.push(worker.id.clone());
                    }
                }
            } else if worker.status == WorkerStatus::Down {
                debug!(
                    "Worker {} 状态为Down但最近有心跳，可能需要人工检查",
                    worker.id
                );
            }
        }

        info!(
            "完成Worker状态恢复，发现 {} 个失效Worker",
            failed_workers.len()
        );
        Ok(failed_workers)
    }
    async fn reconnect_database(&self) -> SchedulerResult<()> {
        self.attempt_database_reconnection().await
    }
    async fn reconnect_message_queue(&self) -> SchedulerResult<()> {
        self.attempt_message_queue_reconnection().await
    }
    async fn check_system_health(&self) -> SchedulerResult<SystemHealthStatus> {
        debug!("检查系统健康状态");

        let now = Utc::now();
        let mut status = SystemHealthStatus {
            database_healthy: false,
            message_queue_healthy: false,
            active_workers: 0,
            pending_tasks: 0,
            running_tasks: 0,
            last_check_time: now,
        };
        match self.task_run_repo.get_pending_runs(Some(1)).await {
            Ok(_) => {
                status.database_healthy = true;
                debug!("数据库连接正常");
            }
            Err(e) => {
                warn!("数据库连接异常: {}", e);
            }
        }
        match self.message_queue.get_queue_size("health_check").await {
            Ok(_) => {
                status.message_queue_healthy = true;
                debug!("消息队列连接正常");
            }
            Err(e) => {
                warn!("消息队列连接异常: {}", e);
            }
        }
        match self.worker_repo.get_alive_workers().await {
            Ok(workers) => {
                status.active_workers = workers.len() as u32;
                debug!("活跃Worker数量: {}", status.active_workers);
            }
            Err(e) => {
                warn!("获取活跃Worker数量失败: {}", e);
            }
        }
        match self.task_run_repo.get_pending_runs(None).await {
            Ok(tasks) => {
                status.pending_tasks = tasks.len() as u32;
                debug!("待处理任务数量: {}", status.pending_tasks);
            }
            Err(e) => {
                warn!("获取待处理任务数量失败: {}", e);
            }
        }
        match self.task_run_repo.get_running_runs().await {
            Ok(tasks) => {
                status.running_tasks = tasks.len() as u32;
                debug!("运行中任务数量: {}", status.running_tasks);
            }
            Err(e) => {
                warn!("获取运行中任务数量失败: {}", e);
            }
        }

        debug!("系统健康状态检查完成: {:?}", status);
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_config_default() {
        let config = RecoveryConfig::default();
        assert_eq!(config.db_max_retry_attempts, 5);
        assert_eq!(config.db_retry_interval_seconds, 10);
        assert_eq!(config.mq_max_retry_attempts, 5);
        assert_eq!(config.mq_retry_interval_seconds, 10);
        assert_eq!(config.startup_recovery_timeout_seconds, 300);
        assert_eq!(config.worker_heartbeat_timeout_seconds, 90);
    }

    #[test]
    fn test_recovery_report_creation() {
        let report = RecoveryReport {
            recovered_tasks: vec![],
            failed_workers: vec!["worker-1".to_string()],
            recovery_duration_ms: 1000,
            errors: vec!["test error".to_string()],
        };

        assert_eq!(report.recovered_tasks.len(), 0);
        assert_eq!(report.failed_workers.len(), 1);
        assert_eq!(report.recovery_duration_ms, 1000);
        assert_eq!(report.errors.len(), 1);
    }

    #[test]
    fn test_system_health_status_creation() {
        let now = Utc::now();
        let status = SystemHealthStatus {
            database_healthy: true,
            message_queue_healthy: true,
            active_workers: 5,
            pending_tasks: 10,
            running_tasks: 3,
            last_check_time: now,
        };

        assert!(status.database_healthy);
        assert!(status.message_queue_healthy);
        assert_eq!(status.active_workers, 5);
        assert_eq!(status.pending_tasks, 10);
        assert_eq!(status.running_tasks, 3);
        assert_eq!(status.last_check_time, now);
    }
}
