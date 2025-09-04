use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// 资源监控配置
#[derive(Debug, Clone)]
pub struct ResourceMonitorConfig {
    /// 监控间隔（秒）
    pub monitor_interval_seconds: u64,
    /// 内存使用警告阈值（MB）
    pub memory_warning_threshold_mb: usize,
    /// 内存使用错误阈值（MB）
    pub memory_error_threshold_mb: usize,
    /// CPU使用警告阈值（百分比）
    pub cpu_warning_threshold_percent: f64,
    /// 是否启用监控
    pub enabled: bool,
    /// 历史数据保留数量
    pub history_size: usize,
}

impl Default for ResourceMonitorConfig {
    fn default() -> Self {
        Self {
            monitor_interval_seconds: 60, // 1分钟
            memory_warning_threshold_mb: 80,
            memory_error_threshold_mb: 100,
            cpu_warning_threshold_percent: 80.0,
            enabled: true,
            history_size: 60, // 保留60个数据点
        }
    }
}

/// 资源使用统计
#[derive(Debug, Clone)]
pub struct ResourceStats {
    /// 内存使用量（MB）
    pub memory_usage_mb: usize,
    /// CPU使用率（百分比）
    pub cpu_usage_percent: f64,
    /// 活跃连接数
    pub active_connections: usize,
    /// 队列中的消息数
    pub queued_messages: usize,
    /// 正在执行的任务数
    pub running_tasks: usize,
    /// 统计时间
    pub timestamp: Instant,
}

impl Default for ResourceStats {
    fn default() -> Self {
        Self {
            memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
            active_connections: 0,
            queued_messages: 0,
            running_tasks: 0,
            timestamp: Instant::now(),
        }
    }
}

/// 资源监控器
/// 
/// 监控系统资源使用情况，包括内存、CPU、连接数等
pub struct ResourceMonitor {
    config: ResourceMonitorConfig,
    current_stats: Arc<RwLock<ResourceStats>>,
    history: Arc<RwLock<Vec<ResourceStats>>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    monitor_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ResourceMonitor {
    /// 创建新的资源监控器
    pub fn new(config: ResourceMonitorConfig) -> Self {
        Self {
            config,
            current_stats: Arc::new(RwLock::new(ResourceStats::default())),
            history: Arc::new(RwLock::new(Vec::new())),
            shutdown_tx: None,
            monitor_handle: None,
        }
    }

    /// 启动监控
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.config.enabled {
            info!("Resource monitor is disabled");
            return Ok(());
        }

        info!("Starting resource monitor with config: {:?}", self.config);

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        let current_stats = self.current_stats.clone();
        let history = self.history.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut monitor_interval = interval(Duration::from_secs(config.monitor_interval_seconds));
            
            loop {
                tokio::select! {
                    _ = monitor_interval.tick() => {
                        if let Err(e) = Self::collect_stats(&current_stats, &history, &config).await {
                            warn!("Failed to collect resource stats: {}", e);
                        }
                    }
                    _ = &mut shutdown_rx => {
                        info!("Resource monitor shutdown requested");
                        break;
                    }
                }
            }
            
            info!("Resource monitor stopped");
        });

        self.monitor_handle = Some(handle);
        info!("Resource monitor started successfully");
        Ok(())
    }

    /// 停止监控
    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(handle) = self.monitor_handle.take() {
            if let Err(e) = handle.await {
                warn!("Error waiting for resource monitor to stop: {}", e);
            }
        }

        info!("Resource monitor stopped");
        Ok(())
    }

    /// 获取当前资源统计
    pub async fn get_current_stats(&self) -> ResourceStats {
        self.current_stats.read().await.clone()
    }

    /// 获取历史统计数据
    pub async fn get_history(&self) -> Vec<ResourceStats> {
        self.history.read().await.clone()
    }

    /// 检查资源使用是否正常
    pub async fn check_resource_health(&self) -> ResourceHealthStatus {
        let stats = self.get_current_stats().await;
        
        let memory_status = if stats.memory_usage_mb >= self.config.memory_error_threshold_mb {
            ResourceStatus::Error
        } else if stats.memory_usage_mb >= self.config.memory_warning_threshold_mb {
            ResourceStatus::Warning
        } else {
            ResourceStatus::Ok
        };

        let cpu_status = if stats.cpu_usage_percent >= self.config.cpu_warning_threshold_percent {
            ResourceStatus::Warning
        } else {
            ResourceStatus::Ok
        };

        ResourceHealthStatus {
            memory_status,
            cpu_status,
            overall_status: match (memory_status, cpu_status) {
                (ResourceStatus::Error, _) | (_, ResourceStatus::Error) => ResourceStatus::Error,
                (ResourceStatus::Warning, _) | (_, ResourceStatus::Warning) => ResourceStatus::Warning,
                _ => ResourceStatus::Ok,
            },
            stats,
        }
    }

    /// 收集资源统计信息
    async fn collect_stats(
        current_stats: &Arc<RwLock<ResourceStats>>,
        history: &Arc<RwLock<Vec<ResourceStats>>>,
        config: &ResourceMonitorConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stats = ResourceStats {
            memory_usage_mb: Self::get_memory_usage(),
            cpu_usage_percent: Self::get_cpu_usage().await,
            active_connections: Self::get_active_connections(),
            queued_messages: Self::get_queued_messages(),
            running_tasks: Self::get_running_tasks(),
            timestamp: Instant::now(),
        };

        // 检查阈值并记录警告
        if stats.memory_usage_mb >= config.memory_warning_threshold_mb {
            warn!(
                "High memory usage detected: {}MB (threshold: {}MB)",
                stats.memory_usage_mb, config.memory_warning_threshold_mb
            );
        }

        if stats.cpu_usage_percent >= config.cpu_warning_threshold_percent {
            warn!(
                "High CPU usage detected: {:.1}% (threshold: {:.1}%)",
                stats.cpu_usage_percent, config.cpu_warning_threshold_percent
            );
        }

        // 更新当前统计
        *current_stats.write().await = stats.clone();

        // 更新历史记录
        {
            let mut history_guard = history.write().await;
            history_guard.push(stats.clone());
            
            // 保持历史记录大小限制
            if history_guard.len() > config.history_size {
                history_guard.remove(0);
            }
        }

        debug!("Resource stats collected: {:?}", stats);
        Ok(())
    }

    /// 获取内存使用量（简化实现）
    fn get_memory_usage() -> usize {
        // 简化的内存使用估算
        // 实际实现可能需要使用系统API或第三方库
        #[cfg(target_os = "linux")]
        {
            Self::get_memory_usage_linux().unwrap_or(0)
        }
        #[cfg(not(target_os = "linux"))]
        {
            // 对于非Linux系统，返回估算值
            50 // 假设50MB基础内存使用
        }
    }

    #[cfg(target_os = "linux")]
    fn get_memory_usage_linux() -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        use std::fs;
        
        let status = fs::read_to_string("/proc/self/status")?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: usize = parts[1].parse()?;
                    return Ok(kb / 1024); // 转换为MB
                }
            }
        }
        Ok(0)
    }

    /// 获取CPU使用率（简化实现）
    async fn get_cpu_usage() -> f64 {
        // 简化的CPU使用率估算
        // 实际实现可能需要使用系统API或第三方库
        tokio::task::yield_now().await;
        0.0 // 暂时返回0，实际实现需要计算CPU使用率
    }

    /// 获取活跃连接数（简化实现）
    fn get_active_connections() -> usize {
        // 简化实现，实际需要从连接池或网络层获取
        0
    }

    /// 获取队列中的消息数（简化实现）
    fn get_queued_messages() -> usize {
        // 简化实现，实际需要从消息队列获取
        0
    }

    /// 获取正在执行的任务数（简化实现）
    fn get_running_tasks() -> usize {
        // 简化实现，实际需要从任务执行器获取
        0
    }
}

/// 资源状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResourceStatus {
    Ok,
    Warning,
    Error,
}

/// 资源健康状态
#[derive(Debug, Clone)]
pub struct ResourceHealthStatus {
    pub memory_status: ResourceStatus,
    pub cpu_status: ResourceStatus,
    pub overall_status: ResourceStatus,
    pub stats: ResourceStats,
}

impl ResourceHealthStatus {
    /// 是否健康
    pub fn is_healthy(&self) -> bool {
        matches!(self.overall_status, ResourceStatus::Ok)
    }

    /// 是否有警告
    pub fn has_warnings(&self) -> bool {
        matches!(self.overall_status, ResourceStatus::Warning)
    }

    /// 是否有错误
    pub fn has_errors(&self) -> bool {
        matches!(self.overall_status, ResourceStatus::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_resource_monitor_config_default() {
        let config = ResourceMonitorConfig::default();
        assert_eq!(config.monitor_interval_seconds, 60);
        assert_eq!(config.memory_warning_threshold_mb, 80);
        assert_eq!(config.memory_error_threshold_mb, 100);
        assert_eq!(config.cpu_warning_threshold_percent, 80.0);
        assert!(config.enabled);
        assert_eq!(config.history_size, 60);
    }

    #[tokio::test]
    async fn test_resource_stats_default() {
        let stats = ResourceStats::default();
        assert_eq!(stats.memory_usage_mb, 0);
        assert_eq!(stats.cpu_usage_percent, 0.0);
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.queued_messages, 0);
        assert_eq!(stats.running_tasks, 0);
    }

    #[tokio::test]
    async fn test_resource_monitor_creation() {
        let config = ResourceMonitorConfig::default();
        let monitor = ResourceMonitor::new(config);
        
        let stats = monitor.get_current_stats().await;
        assert_eq!(stats.memory_usage_mb, 0);
        
        let history = monitor.get_history().await;
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_resource_health_status() {
        let stats = ResourceStats::default();
        let health = ResourceHealthStatus {
            memory_status: ResourceStatus::Ok,
            cpu_status: ResourceStatus::Ok,
            overall_status: ResourceStatus::Ok,
            stats,
        };
        
        assert!(health.is_healthy());
        assert!(!health.has_warnings());
        assert!(!health.has_errors());
    }
}