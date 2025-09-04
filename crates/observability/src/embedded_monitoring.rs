use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::{
    log_rotation::{LogRotationConfig, LogRotationManager, LogRotationStats},
    metrics_collector::{HealthStatus, MetricsCollector, PerformanceMetrics},
    structured_logger::StructuredLogger,
};

/// 嵌入式监控服务配置
#[derive(Debug, Clone)]
pub struct EmbeddedMonitoringConfig {
    /// 是否启用日志轮转
    pub enable_log_rotation: bool,
    /// 日志轮转配置
    pub log_rotation: LogRotationConfig,
    /// 性能指标收集间隔（秒）
    pub metrics_collection_interval_seconds: u64,
    /// 健康检查间隔（秒）
    pub health_check_interval_seconds: u64,
    /// 是否启用性能回归检测
    pub enable_performance_regression_detection: bool,
    /// 性能警告阈值
    pub performance_thresholds: PerformanceThresholds,
}

/// 性能警告阈值
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    /// CPU使用率警告阈值（百分比）
    pub cpu_warning_threshold: f64,
    /// CPU使用率严重阈值（百分比）
    pub cpu_critical_threshold: f64,
    /// 内存使用警告阈值（MB）
    pub memory_warning_threshold: f64,
    /// 内存使用严重阈值（MB）
    pub memory_critical_threshold: f64,
    /// 任务执行时间警告阈值（秒）
    pub task_duration_warning_threshold: f64,
    /// 错误率警告阈值（百分比）
    pub error_rate_warning_threshold: f64,
}

impl Default for EmbeddedMonitoringConfig {
    fn default() -> Self {
        Self {
            enable_log_rotation: true,
            log_rotation: LogRotationConfig::default(),
            metrics_collection_interval_seconds: 60, // 1分钟
            health_check_interval_seconds: 30, // 30秒
            enable_performance_regression_detection: true,
            performance_thresholds: PerformanceThresholds::default(),
        }
    }
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            cpu_warning_threshold: 70.0,
            cpu_critical_threshold: 90.0,
            memory_warning_threshold: 4096.0, // 4GB
            memory_critical_threshold: 8192.0, // 8GB
            task_duration_warning_threshold: 300.0, // 5分钟
            error_rate_warning_threshold: 10.0, // 10%
        }
    }
}

/// 嵌入式监控服务
pub struct EmbeddedMonitoringService {
    config: EmbeddedMonitoringConfig,
    metrics_collector: Arc<MetricsCollector>,
    log_rotation_manager: Option<LogRotationManager>,
    is_running: Arc<tokio::sync::Mutex<bool>>,
    start_time: SystemTime,
}

impl EmbeddedMonitoringService {
    /// 创建新的监控服务
    pub async fn new(
        config: EmbeddedMonitoringConfig,
        metrics_collector: Arc<MetricsCollector>,
    ) -> Result<Self> {
        let log_rotation_manager = if config.enable_log_rotation {
            Some(LogRotationManager::new(config.log_rotation.clone())?)
        } else {
            None
        };

        Ok(Self {
            config,
            metrics_collector,
            log_rotation_manager,
            is_running: Arc::new(tokio::sync::Mutex::new(false)),
            start_time: SystemTime::now(),
        })
    }

    /// 启动监控服务
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.is_running.lock().await;
            if *running {
                return Ok(());
            }
            *running = true;
        }

        info!("Starting embedded monitoring service");

        // 启动日志轮转管理器
        if let Some(ref log_manager) = self.log_rotation_manager {
            log_manager.start().await
                .context("Failed to start log rotation manager")?;
            info!("Log rotation manager started");
        }

        // 启动性能指标收集任务
        self.start_metrics_collection_task().await;

        // 启动健康检查任务
        self.start_health_check_task().await;

        // 启动性能回归检测任务
        if self.config.enable_performance_regression_detection {
            self.start_regression_detection_task().await;
        }

        info!("Embedded monitoring service started successfully");
        Ok(())
    }

    /// 停止监控服务
    pub async fn stop(&self) -> Result<()> {
        {
            let mut running = self.is_running.lock().await;
            *running = false;
        }

        info!("Stopping embedded monitoring service");

        // 停止日志轮转管理器
        if let Some(ref log_manager) = self.log_rotation_manager {
            log_manager.stop()
                .context("Failed to stop log rotation manager")?;
            info!("Log rotation manager stopped");
        }

        info!("Embedded monitoring service stopped");
        Ok(())
    }

    /// 启动性能指标收集任务
    async fn start_metrics_collection_task(&self) {
        let metrics_collector = Arc::clone(&self.metrics_collector);
        let is_running = Arc::clone(&self.is_running);
        let interval_seconds = self.config.metrics_collection_interval_seconds;
        let thresholds = self.config.performance_thresholds.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_seconds));

            loop {
                interval.tick().await;

                // 检查是否应该停止
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 收集性能指标
                match metrics_collector.collect_system_metrics().await {
                    Ok(metrics) => {
                        // 检查性能阈值
                        Self::check_performance_thresholds(&metrics, &thresholds);
                        
                        // 记录指标到日志
                        StructuredLogger::log_performance_metrics(
                            "embedded_monitoring",
                            "system_metrics_collection",
                            interval_seconds * 1000, // 转换为毫秒
                            None,
                            Some(metrics.memory_usage_mb as u64),
                        );
                    }
                    Err(e) => {
                        error!("Failed to collect system metrics: {}", e);
                    }
                }
            }

            info!("Metrics collection task stopped");
        });
    }

    /// 启动健康检查任务
    async fn start_health_check_task(&self) {
        let metrics_collector = Arc::clone(&self.metrics_collector);
        let is_running = Arc::clone(&self.is_running);
        let interval_seconds = self.config.health_check_interval_seconds;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_seconds));

            loop {
                interval.tick().await;

                // 检查是否应该停止
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 执行健康检查
                let health_status = metrics_collector.get_health_status().await;
                
                match health_status {
                    HealthStatus::Healthy => {
                        // 只在调试级别记录健康状态
                        tracing::debug!("System health check: healthy");
                    }
                    HealthStatus::Warning => {
                        warn!("System health check: warning");
                        StructuredLogger::log_performance_warning(
                            "embedded_monitoring",
                            "health_check",
                            "system_health",
                            0.0,
                            0.0,
                            "status",
                        );
                    }
                    HealthStatus::Critical => {
                        error!("System health check: critical");
                        let error = std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "System health is critical"
                        );
                        StructuredLogger::log_system_error_with_stack(
                            "embedded_monitoring",
                            "health_check",
                            &error,
                            None,
                        );
                    }
                    HealthStatus::Unknown => {
                        warn!("System health check: unknown");
                    }
                }
            }

            info!("Health check task stopped");
        });
    }

    /// 启动性能回归检测任务
    async fn start_regression_detection_task(&self) {
        let metrics_collector = Arc::clone(&self.metrics_collector);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // 5分钟检查一次

            loop {
                interval.tick().await;

                // 检查是否应该停止
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 检测性能回归
                match metrics_collector.detect_performance_regressions().await {
                    regressions if !regressions.is_empty() => {
                        for regression in regressions {
                            warn!("Performance regression detected: {}", regression);
                            StructuredLogger::log_performance_warning(
                                "embedded_monitoring",
                                "regression_detection",
                                "performance_regression",
                                0.0,
                                0.0,
                                &regression,
                            );
                        }
                    }
                    _ => {
                        // 没有检测到回归，不记录日志
                    }
                }
            }

            info!("Regression detection task stopped");
        });
    }

    /// 检查性能阈值
    fn check_performance_thresholds(metrics: &PerformanceMetrics, thresholds: &PerformanceThresholds) {
        // 检查CPU使用率
        if metrics.cpu_usage_percent >= thresholds.cpu_critical_threshold {
            error!(
                "CPU usage critical: {:.1}% (threshold: {:.1}%)",
                metrics.cpu_usage_percent, thresholds.cpu_critical_threshold
            );
            StructuredLogger::log_performance_warning(
                "embedded_monitoring",
                "threshold_check",
                "cpu_usage",
                metrics.cpu_usage_percent,
                thresholds.cpu_critical_threshold,
                "percent",
            );
        } else if metrics.cpu_usage_percent >= thresholds.cpu_warning_threshold {
            warn!(
                "CPU usage warning: {:.1}% (threshold: {:.1}%)",
                metrics.cpu_usage_percent, thresholds.cpu_warning_threshold
            );
        }

        // 检查内存使用率
        if metrics.memory_usage_mb >= thresholds.memory_critical_threshold {
            error!(
                "Memory usage critical: {:.1}MB (threshold: {:.1}MB)",
                metrics.memory_usage_mb, thresholds.memory_critical_threshold
            );
            StructuredLogger::log_performance_warning(
                "embedded_monitoring",
                "threshold_check",
                "memory_usage",
                metrics.memory_usage_mb,
                thresholds.memory_critical_threshold,
                "MB",
            );
        } else if metrics.memory_usage_mb >= thresholds.memory_warning_threshold {
            warn!(
                "Memory usage warning: {:.1}MB (threshold: {:.1}MB)",
                metrics.memory_usage_mb, thresholds.memory_warning_threshold
            );
        }
    }

    /// 记录任务执行详细信息
    pub fn log_task_execution_detailed(
        &self,
        task_run_id: i64,
        task_id: i64,
        task_name: &str,
        task_type: &str,
        worker_id: &str,
        parameters: Option<&str>,
        timeout_seconds: u32,
        retry_count: u32,
        max_retries: u32,
    ) {
        StructuredLogger::log_task_execution_detailed(
            task_run_id,
            task_id,
            task_name,
            task_type,
            worker_id,
            parameters,
            timeout_seconds,
            retry_count,
            max_retries,
        );
    }

    /// 记录任务执行步骤
    pub fn log_task_execution_step(
        &self,
        task_run_id: i64,
        task_name: &str,
        step: &str,
        step_data: Option<&str>,
        duration_ms: Option<u64>,
    ) {
        StructuredLogger::log_task_execution_step(
            task_run_id,
            task_name,
            step,
            step_data,
            duration_ms,
        );
    }

    /// 记录任务资源使用情况
    pub fn log_task_resource_usage(
        &self,
        task_run_id: i64,
        task_name: &str,
        memory_usage_mb: Option<f64>,
        cpu_usage_percent: Option<f64>,
        disk_io_mb: Option<f64>,
        network_io_mb: Option<f64>,
    ) {
        StructuredLogger::log_task_resource_usage(
            task_run_id,
            task_name,
            memory_usage_mb,
            cpu_usage_percent,
            disk_io_mb,
            network_io_mb,
        );
    }

    /// 记录任务执行失败详细信息
    pub fn log_task_execution_failure_detailed(
        &self,
        task_run_id: i64,
        task_name: &str,
        task_type: &str,
        worker_id: &str,
        error_type: &str,
        error_message: &str,
        duration_ms: u64,
        retry_count: u32,
        will_retry: bool,
        stack_trace: Option<&str>,
    ) {
        StructuredLogger::log_task_execution_failure_detailed(
            task_run_id,
            task_name,
            task_type,
            worker_id,
            error_type,
            error_message,
            duration_ms,
            retry_count,
            will_retry,
            stack_trace,
        );
    }

    /// 获取监控统计信息
    pub async fn get_monitoring_stats(&self) -> Result<MonitoringStats> {
        let uptime = self.start_time.elapsed()
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        let performance_summary = self.metrics_collector.get_performance_summary().await?;
        let health_status = self.metrics_collector.get_health_status().await;

        let log_rotation_stats = if let Some(ref log_manager) = self.log_rotation_manager {
            Some(log_manager.get_rotation_stats()?)
        } else {
            None
        };

        Ok(MonitoringStats {
            uptime_seconds: uptime,
            health_status,
            performance_summary,
            log_rotation_stats,
            is_running: *self.is_running.lock().await,
        })
    }

    /// 记录嵌入式应用指标
    pub fn record_embedded_metrics(
        &self,
        active_tasks: u64,
        queue_size: u64,
        database_connections: u64,
        memory_queue_size: u64,
    ) {
        self.metrics_collector.record_embedded_metrics(
            active_tasks,
            queue_size,
            database_connections,
            memory_queue_size,
        );
    }
}

/// 监控统计信息
#[derive(Debug, Clone)]
pub struct MonitoringStats {
    pub uptime_seconds: u64,
    pub health_status: HealthStatus,
    pub performance_summary: String,
    pub log_rotation_stats: Option<LogRotationStats>,
    pub is_running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_embedded_monitoring_service_creation() {
        let metrics_collector = Arc::new(MetricsCollector::new().unwrap());
        let config = EmbeddedMonitoringConfig::default();
        
        let service = EmbeddedMonitoringService::new(config, metrics_collector).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_embedded_monitoring_service_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");
        
        let metrics_collector = Arc::new(MetricsCollector::new().unwrap());
        let config = EmbeddedMonitoringConfig {
            enable_log_rotation: true,
            log_rotation: LogRotationConfig {
                log_file_path: log_path,
                max_file_size_bytes: 1024,
                max_files: 5,
                check_interval_seconds: 1,
                compress_rotated_files: false,
            },
            ..Default::default()
        };
        
        let service = EmbeddedMonitoringService::new(config, metrics_collector).await.unwrap();
        
        // 启动服务
        assert!(service.start().await.is_ok());
        
        // 等待一小段时间让任务启动
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 停止服务
        assert!(service.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_get_monitoring_stats() {
        let metrics_collector = Arc::new(MetricsCollector::new().unwrap());
        let config = EmbeddedMonitoringConfig {
            enable_log_rotation: false,
            ..Default::default()
        };
        
        let service = EmbeddedMonitoringService::new(config, metrics_collector).await.unwrap();
        
        let stats = service.get_monitoring_stats().await;
        assert!(stats.is_ok());
        
        let stats = stats.unwrap();
        assert!(!stats.is_running); // 服务未启动
        assert!(stats.log_rotation_stats.is_none()); // 日志轮转未启用
    }

    #[test]
    fn test_performance_thresholds_default() {
        let thresholds = PerformanceThresholds::default();
        assert_eq!(thresholds.cpu_warning_threshold, 70.0);
        assert_eq!(thresholds.cpu_critical_threshold, 90.0);
        assert_eq!(thresholds.memory_warning_threshold, 4096.0);
        assert_eq!(thresholds.memory_critical_threshold, 8192.0);
        assert_eq!(thresholds.task_duration_warning_threshold, 300.0);
        assert_eq!(thresholds.error_rate_warning_threshold, 10.0);
    }

    #[test]
    fn test_embedded_monitoring_config_default() {
        let config = EmbeddedMonitoringConfig::default();
        assert!(config.enable_log_rotation);
        assert_eq!(config.metrics_collection_interval_seconds, 60);
        assert_eq!(config.health_check_interval_seconds, 30);
        assert!(config.enable_performance_regression_detection);
    }

    #[test]
    fn test_check_performance_thresholds() {
        let thresholds = PerformanceThresholds::default();
        
        // 正常指标
        let normal_metrics = PerformanceMetrics {
            cpu_usage_percent: 50.0,
            memory_usage_mb: 2048.0,
            disk_usage_mb: 1024.0,
            network_io_mb: 100.0,
            timestamp: SystemTime::now(),
        };
        
        // 这个测试主要验证函数不会panic
        EmbeddedMonitoringService::check_performance_thresholds(&normal_metrics, &thresholds);
        
        // 高CPU使用率
        let high_cpu_metrics = PerformanceMetrics {
            cpu_usage_percent: 95.0,
            memory_usage_mb: 2048.0,
            disk_usage_mb: 1024.0,
            network_io_mb: 100.0,
            timestamp: SystemTime::now(),
        };
        
        EmbeddedMonitoringService::check_performance_thresholds(&high_cpu_metrics, &thresholds);
        
        // 高内存使用率
        let high_memory_metrics = PerformanceMetrics {
            cpu_usage_percent: 50.0,
            memory_usage_mb: 9000.0,
            disk_usage_mb: 1024.0,
            network_io_mb: 100.0,
            timestamp: SystemTime::now(),
        };
        
        EmbeddedMonitoringService::check_performance_thresholds(&high_memory_metrics, &thresholds);
    }
}