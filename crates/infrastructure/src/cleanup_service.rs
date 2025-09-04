use chrono::{DateTime, Duration, Utc};
use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};
use scheduler_errors::SchedulerResult;
use std::sync::Arc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// 数据清理配置
#[derive(Debug, Clone)]
pub struct CleanupConfig {
    /// 清理间隔（秒）
    pub cleanup_interval_seconds: u64,
    /// 任务运行记录保留天数
    pub task_run_retention_days: i64,
    /// 已完成任务运行记录保留天数
    pub completed_task_run_retention_days: i64,
    /// 失败任务运行记录保留天数
    pub failed_task_run_retention_days: i64,
    /// 是否启用自动清理
    pub enabled: bool,
    /// 每次清理的最大记录数
    pub max_cleanup_batch_size: usize,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            cleanup_interval_seconds: 3600, // 1小时
            task_run_retention_days: 30,    // 30天
            completed_task_run_retention_days: 7, // 已完成的保留7天
            failed_task_run_retention_days: 30,   // 失败的保留30天
            enabled: true,
            max_cleanup_batch_size: 1000,
        }
    }
}

/// 数据清理服务
/// 
/// 负责自动清理过期的任务运行记录，防止数据库无限增长
pub struct CleanupService {
    _task_repository: Arc<dyn TaskRepository>,
    task_run_repository: Arc<dyn TaskRunRepository>,
    config: CleanupConfig,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl CleanupService {
    /// 创建新的清理服务实例
    pub fn new(
        task_repository: Arc<dyn TaskRepository>,
        task_run_repository: Arc<dyn TaskRunRepository>,
        config: CleanupConfig,
    ) -> Self {
        Self {
            _task_repository: task_repository,
            task_run_repository,
            config,
            shutdown_tx: None,
            cleanup_handle: None,
        }
    }

    /// 启动清理服务
    pub async fn start(&mut self) -> SchedulerResult<()> {
        if !self.config.enabled {
            info!("Cleanup service is disabled");
            return Ok(());
        }

        info!("Starting cleanup service with config: {:?}", self.config);

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        let task_run_repository = self.task_run_repository.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut cleanup_interval = interval(std::time::Duration::from_secs(config.cleanup_interval_seconds));
            
            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        if let Err(e) = Self::perform_cleanup(&task_run_repository, &config).await {
                            error!("Cleanup failed: {}", e);
                        }
                    }
                    _ = &mut shutdown_rx => {
                        info!("Cleanup service shutdown requested");
                        break;
                    }
                }
            }
            
            info!("Cleanup service stopped");
        });

        self.cleanup_handle = Some(handle);
        info!("Cleanup service started successfully");
        Ok(())
    }

    /// 停止清理服务
    pub async fn stop(&mut self) -> SchedulerResult<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(handle) = self.cleanup_handle.take() {
            if let Err(e) = handle.await {
                warn!("Error waiting for cleanup service to stop: {}", e);
            }
        }

        info!("Cleanup service stopped");
        Ok(())
    }

    /// 执行一次清理操作
    pub async fn cleanup_once(&self) -> SchedulerResult<CleanupStats> {
        if !self.config.enabled {
            return Ok(CleanupStats::default());
        }

        Self::perform_cleanup(&self.task_run_repository, &self.config).await
    }

    /// 执行清理操作
    async fn perform_cleanup(
        task_run_repository: &Arc<dyn TaskRunRepository>,
        config: &CleanupConfig,
    ) -> SchedulerResult<CleanupStats> {
        let start_time = std::time::Instant::now();
        let mut stats = CleanupStats::default();

        info!("Starting cleanup operation");

        // 清理已完成的任务运行记录
        let completed_cutoff = Utc::now() - Duration::days(config.completed_task_run_retention_days);
        stats.completed_cleaned = Self::cleanup_task_runs_by_status_and_date(
            task_run_repository,
            &["COMPLETED", "SUCCESS"],
            completed_cutoff,
            config.max_cleanup_batch_size,
        ).await?;

        // 清理失败的任务运行记录
        let failed_cutoff = Utc::now() - Duration::days(config.failed_task_run_retention_days);
        stats.failed_cleaned = Self::cleanup_task_runs_by_status_and_date(
            task_run_repository,
            &["FAILED", "TIMEOUT", "CANCELLED"],
            failed_cutoff,
            config.max_cleanup_batch_size,
        ).await?;

        // 清理所有旧的任务运行记录
        let general_cutoff = Utc::now() - Duration::days(config.task_run_retention_days);
        stats.general_cleaned = Self::cleanup_old_task_runs(
            task_run_repository,
            general_cutoff,
            config.max_cleanup_batch_size,
        ).await?;

        stats.duration = start_time.elapsed();
        stats.total_cleaned = stats.completed_cleaned + stats.failed_cleaned + stats.general_cleaned;

        info!(
            "Cleanup completed: {} total records cleaned in {:?} (completed: {}, failed: {}, general: {})",
            stats.total_cleaned,
            stats.duration,
            stats.completed_cleaned,
            stats.failed_cleaned,
            stats.general_cleaned
        );

        Ok(stats)
    }

    /// 根据状态和日期清理任务运行记录
    async fn cleanup_task_runs_by_status_and_date(
        task_run_repository: &Arc<dyn TaskRunRepository>,
        statuses: &[&str],
        cutoff_date: DateTime<Utc>,
        batch_size: usize,
    ) -> SchedulerResult<usize> {
        let mut total_cleaned = 0;

        for status in statuses {
            debug!("Cleaning up {} task runs older than {}", status, cutoff_date);
            
            // 这里需要扩展TaskRunRepository接口来支持按状态和日期删除
            // 由于当前接口限制，我们先实现一个简化版本
            let cleaned = Self::cleanup_task_runs_by_criteria(
                task_run_repository,
                Some(status),
                Some(cutoff_date),
                batch_size,
            ).await?;
            
            total_cleaned += cleaned;
            debug!("Cleaned {} {} task runs", cleaned, status);
        }

        Ok(total_cleaned)
    }

    /// 清理旧的任务运行记录
    async fn cleanup_old_task_runs(
        task_run_repository: &Arc<dyn TaskRunRepository>,
        cutoff_date: DateTime<Utc>,
        batch_size: usize,
    ) -> SchedulerResult<usize> {
        debug!("Cleaning up all task runs older than {}", cutoff_date);
        
        Self::cleanup_task_runs_by_criteria(
            task_run_repository,
            None,
            Some(cutoff_date),
            batch_size,
        ).await
    }

    /// 根据条件清理任务运行记录
    async fn cleanup_task_runs_by_criteria(
        task_run_repository: &Arc<dyn TaskRunRepository>,
        status: Option<&str>,
        cutoff_date: Option<DateTime<Utc>>,
        batch_size: usize,
    ) -> SchedulerResult<usize> {
        // 尝试将repository转换为SqliteTaskRunRepository以使用扩展方法
        // 这是一个临时解决方案，理想情况下应该扩展trait接口
        
        if let Some(sqlite_repo) = task_run_repository.as_any().downcast_ref::<crate::database::sqlite::SqliteTaskRunRepository>() {
            match (status, cutoff_date) {
                (Some(status), Some(cutoff_date)) => {
                    sqlite_repo.cleanup_runs_by_status_and_date(status, cutoff_date, batch_size).await
                }
                (None, Some(cutoff_date)) => {
                    sqlite_repo.cleanup_runs_before_date(cutoff_date, batch_size).await
                }
                _ => {
                    warn!("Invalid cleanup criteria provided");
                    Ok(0)
                }
            }
        } else {
            warn!("Cleanup operation requires SqliteTaskRunRepository");
            Ok(0)
        }
    }
}

/// 清理统计信息
#[derive(Debug, Default)]
pub struct CleanupStats {
    /// 清理的已完成任务数
    pub completed_cleaned: usize,
    /// 清理的失败任务数
    pub failed_cleaned: usize,
    /// 清理的一般任务数
    pub general_cleaned: usize,
    /// 总清理数
    pub total_cleaned: usize,
    /// 清理耗时
    pub duration: std::time::Duration,
}

impl CleanupStats {
    /// 是否有清理操作
    pub fn has_cleanup(&self) -> bool {
        self.total_cleaned > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    // 注意：这些测试需要mock的TaskRepository和TaskRunRepository实现
    // 实际测试应该在集成测试中进行

    #[tokio::test]
    async fn test_cleanup_config_default() {
        let config = CleanupConfig::default();
        assert_eq!(config.cleanup_interval_seconds, 3600);
        assert_eq!(config.task_run_retention_days, 30);
        assert_eq!(config.completed_task_run_retention_days, 7);
        assert_eq!(config.failed_task_run_retention_days, 30);
        assert!(config.enabled);
        assert_eq!(config.max_cleanup_batch_size, 1000);
    }

    #[tokio::test]
    async fn test_cleanup_stats_default() {
        let stats = CleanupStats::default();
        assert_eq!(stats.completed_cleaned, 0);
        assert_eq!(stats.failed_cleaned, 0);
        assert_eq!(stats.general_cleaned, 0);
        assert_eq!(stats.total_cleaned, 0);
        assert!(!stats.has_cleanup());
    }
}