use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tracing::{debug, error, info, warn};

use scheduler_core::SchedulerResult;
use scheduler_domain::entities::{WorkerInfo, WorkerStatus};
use scheduler_domain::repositories::WorkerRepository;

use crate::retry_service::RetryService;

#[derive(Debug, Clone)]
pub struct WorkerFailureDetectorConfig {
    pub heartbeat_timeout_seconds: i64,
    pub detection_interval_seconds: u64,
    pub auto_cleanup_offline_workers: bool,
    pub offline_cleanup_threshold_seconds: i64,
}

impl Default for WorkerFailureDetectorConfig {
    fn default() -> Self {
        Self {
            heartbeat_timeout_seconds: 90,          // 90秒心跳超时
            detection_interval_seconds: 30,         // 30秒检测一次
            auto_cleanup_offline_workers: true,     // 自动清理离线Worker
            offline_cleanup_threshold_seconds: 300, // 5分钟后清理离线Worker
        }
    }
}

#[async_trait]
pub trait WorkerFailureDetectorService: Send + Sync {
    async fn start_detection(&self) -> SchedulerResult<()>;
    async fn stop_detection(&self) -> SchedulerResult<()>;
    async fn detect_failed_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;
    async fn handle_failed_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()>;
    async fn cleanup_offline_workers(&self) -> SchedulerResult<u64>;
}

pub struct WorkerFailureDetector {
    worker_repo: Arc<dyn WorkerRepository>,
    retry_service: Arc<dyn RetryService>,
    config: WorkerFailureDetectorConfig,
    running: Arc<tokio::sync::RwLock<bool>>,
}

impl WorkerFailureDetector {
    pub fn new(
        worker_repo: Arc<dyn WorkerRepository>,
        retry_service: Arc<dyn RetryService>,
        config: Option<WorkerFailureDetectorConfig>,
    ) -> Self {
        Self {
            worker_repo,
            retry_service,
            config: config.unwrap_or_default(),
            running: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }
    fn is_worker_failed(&self, worker: &WorkerInfo, now: DateTime<Utc>) -> bool {
        if worker.status != WorkerStatus::Alive {
            return false; // 已经标记为Down的Worker不需要重复处理
        }

        let time_since_heartbeat = now - worker.last_heartbeat;
        time_since_heartbeat.num_seconds() > self.config.heartbeat_timeout_seconds
    }
    fn should_cleanup_worker(&self, worker: &WorkerInfo, now: DateTime<Utc>) -> bool {
        if worker.status != WorkerStatus::Down {
            return false; // 只清理已经标记为Down的Worker
        }

        let time_since_heartbeat = now - worker.last_heartbeat;
        time_since_heartbeat.num_seconds() > self.config.offline_cleanup_threshold_seconds
    }
    async fn detection_loop(&self) -> SchedulerResult<()> {
        info!("启动Worker失效检测循环");

        let interval_duration = Duration::from_secs(self.config.detection_interval_seconds);

        loop {
            if !*self.running.read().await {
                info!("收到停止信号，退出Worker失效检测循环");
                break;
            }
            match self.detect_failed_workers().await {
                Ok(failed_workers) => {
                    if !failed_workers.is_empty() {
                        info!("检测到 {} 个失效的Worker", failed_workers.len());

                        for worker in failed_workers {
                            if let Err(e) = self.handle_failed_worker(&worker).await {
                                error!("处理失效Worker {} 时出错: {}", worker.id, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Worker失效检测时出错: {}", e);
                }
            }
            if self.config.auto_cleanup_offline_workers {
                match self.cleanup_offline_workers().await {
                    Ok(cleaned_count) => {
                        if cleaned_count > 0 {
                            info!("清理了 {} 个离线Worker", cleaned_count);
                        }
                    }
                    Err(e) => {
                        error!("清理离线Worker时出错: {}", e);
                    }
                }
            }
            tokio::time::sleep(interval_duration).await;
        }

        Ok(())
    }
}

#[async_trait]
impl WorkerFailureDetectorService for WorkerFailureDetector {
    async fn start_detection(&self) -> SchedulerResult<()> {
        info!("启动Worker失效检测服务");
        {
            let mut running = self.running.write().await;
            *running = true;
        }
        self.detection_loop().await
    }
    async fn stop_detection(&self) -> SchedulerResult<()> {
        info!("停止Worker失效检测服务");

        let mut running = self.running.write().await;
        *running = false;

        Ok(())
    }
    async fn detect_failed_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
        debug!("开始检测失效的Worker");

        let now = Utc::now();
        let all_workers = self.worker_repo.list().await?;
        let mut failed_workers = Vec::new();

        for worker in all_workers {
            if self.is_worker_failed(&worker, now) {
                warn!(
                    "检测到失效Worker: {} (上次心跳: {})",
                    worker.id,
                    worker.last_heartbeat.format("%Y-%m-%d %H:%M:%S UTC")
                );
                failed_workers.push(worker);
            }
        }

        if !failed_workers.is_empty() {
            info!("检测到 {} 个失效Worker", failed_workers.len());
        }

        Ok(failed_workers)
    }
    async fn handle_failed_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
        info!("处理失效Worker: {}", worker.id);
        if let Err(e) = self
            .worker_repo
            .update_status(&worker.id, WorkerStatus::Down)
            .await
        {
            error!("更新失效Worker {} 状态失败: {}", worker.id, e);
            return Err(e);
        }

        info!("已将Worker {} 标记为Down状态", worker.id);
        match self.retry_service.handle_worker_failure(&worker.id).await {
            Ok(reassigned_tasks) => {
                if !reassigned_tasks.is_empty() {
                    info!(
                        "成功重新分配失效Worker {} 上的 {} 个任务",
                        worker.id,
                        reassigned_tasks.len()
                    );
                } else {
                    debug!("失效Worker {} 上没有需要重新分配的任务", worker.id);
                }
            }
            Err(e) => {
                error!("重新分配失效Worker {} 上的任务失败: {}", worker.id, e);
            }
        }

        info!("完成失效Worker {} 的处理", worker.id);
        Ok(())
    }
    async fn cleanup_offline_workers(&self) -> SchedulerResult<u64> {
        debug!("开始清理离线Worker");

        let now = Utc::now();
        let all_workers = self.worker_repo.list().await?;
        let mut cleanup_count = 0;

        for worker in all_workers {
            if self.should_cleanup_worker(&worker, now) {
                info!(
                    "清理离线Worker: {} (离线时间: {}分钟)",
                    worker.id,
                    (now - worker.last_heartbeat).num_minutes()
                );

                match self.worker_repo.unregister(&worker.id).await {
                    Ok(()) => {
                        cleanup_count += 1;
                        debug!("成功清理离线Worker: {}", worker.id);
                    }
                    Err(e) => {
                        error!("清理离线Worker {} 失败: {}", worker.id, e);
                    }
                }
            }
        }

        if cleanup_count > 0 {
            info!("清理了 {} 个离线Worker", cleanup_count);
        }

        Ok(cleanup_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mocks::{MockRetryService, MockWorkerRepository};
    use chrono::Duration;

    #[test]
    fn test_is_worker_failed() {
        let config = WorkerFailureDetectorConfig::default();
        let detector = WorkerFailureDetector {
            worker_repo: Arc::new(MockWorkerRepository::new()),
            retry_service: Arc::new(MockRetryService::new()),
            config,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        };

        let now = Utc::now();
        let normal_worker = WorkerInfo {
            id: "worker-1".to_string(),
            hostname: "host1".to_string(),
            ip_address: "127.0.0.1".parse().unwrap(),
            supported_task_types: vec!["shell".to_string()],
            max_concurrent_tasks: 5,
            status: WorkerStatus::Alive,
            last_heartbeat: now - Duration::seconds(30), // 30秒前
            registered_at: now - Duration::hours(1),
            current_task_count: 0,
        };

        assert!(!detector.is_worker_failed(&normal_worker, now));
        let failed_worker = WorkerInfo {
            id: "worker-2".to_string(),
            hostname: "host2".to_string(),
            ip_address: "127.0.0.1".parse().unwrap(),
            supported_task_types: vec!["shell".to_string()],
            max_concurrent_tasks: 5,
            status: WorkerStatus::Alive,
            last_heartbeat: now - Duration::seconds(120), // 120秒前，超过90秒阈值
            registered_at: now - Duration::hours(1),
            current_task_count: 0,
        };

        assert!(detector.is_worker_failed(&failed_worker, now));
        let down_worker = WorkerInfo {
            id: "worker-3".to_string(),
            hostname: "host3".to_string(),
            ip_address: "127.0.0.1".parse().unwrap(),
            supported_task_types: vec!["shell".to_string()],
            max_concurrent_tasks: 5,
            status: WorkerStatus::Down,
            last_heartbeat: now - Duration::seconds(120),
            registered_at: now - Duration::hours(1),
            current_task_count: 0,
        };

        assert!(!detector.is_worker_failed(&down_worker, now));
    }

    #[test]
    fn test_should_cleanup_worker() {
        let config = WorkerFailureDetectorConfig::default();
        let detector = WorkerFailureDetector {
            worker_repo: Arc::new(MockWorkerRepository::new()),
            retry_service: Arc::new(MockRetryService::new()),
            config,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        };

        let now = Utc::now();
        let recent_down_worker = WorkerInfo {
            id: "worker-1".to_string(),
            hostname: "host1".to_string(),
            ip_address: "127.0.0.1".parse().unwrap(),
            supported_task_types: vec!["shell".to_string()],
            max_concurrent_tasks: 5,
            status: WorkerStatus::Down,
            last_heartbeat: now - Duration::seconds(200), // 200秒前，小于300秒阈值
            registered_at: now - Duration::hours(1),
            current_task_count: 0,
        };

        assert!(!detector.should_cleanup_worker(&recent_down_worker, now));
        let old_down_worker = WorkerInfo {
            id: "worker-2".to_string(),
            hostname: "host2".to_string(),
            ip_address: "127.0.0.1".parse().unwrap(),
            supported_task_types: vec!["shell".to_string()],
            max_concurrent_tasks: 5,
            status: WorkerStatus::Down,
            last_heartbeat: now - Duration::seconds(400), // 400秒前，超过300秒阈值
            registered_at: now - Duration::hours(1),
            current_task_count: 0,
        };

        assert!(detector.should_cleanup_worker(&old_down_worker, now));
        let alive_worker = WorkerInfo {
            id: "worker-3".to_string(),
            hostname: "host3".to_string(),
            ip_address: "127.0.0.1".parse().unwrap(),
            supported_task_types: vec!["shell".to_string()],
            max_concurrent_tasks: 5,
            status: WorkerStatus::Alive,
            last_heartbeat: now - Duration::seconds(400),
            registered_at: now - Duration::hours(1),
            current_task_count: 0,
        };

        assert!(!detector.should_cleanup_worker(&alive_worker, now));
    }
}
