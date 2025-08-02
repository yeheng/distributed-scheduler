use async_trait::async_trait;
use std::collections::HashMap;
use crate::SchedulerResult;
use super::TimeRange;

/// 指标收集服务接口
///
/// 专注于性能指标的收集
#[async_trait]
pub trait MetricsCollectionService: Send + Sync {
    /// 记录指标
    async fn record_metric(
        &self,
        name: &str,
        value: f64,
        tags: &HashMap<String, String>,
    ) -> SchedulerResult<()>;

    /// 批量记录指标
    async fn record_metrics(&self, metrics: &[MetricRecord]) ->SchedulerResult<()>;
}

/// 事件记录服务接口
///
/// 专注于系统事件的记录
#[async_trait]
pub trait EventRecordingService: Send + Sync {
    /// 记录事件
    async fn record_event(&self, event_type: &str, data: &serde_json::Value) -> SchedulerResult<()>;

    /// 批量记录事件
    async fn record_events(&self, events: &[EventRecord]) -> SchedulerResult<()>;
}

/// 健康检查服务接口
///
/// 专注于系统健康状态监控
#[async_trait]
pub trait HealthCheckService: Send + Sync {
    /// 获取系统健康状态
    async fn get_system_health(&self) -> SchedulerResult<SystemHealth>;

    /// 检查组件健康状态
    async fn check_component_health(&self, component: &str) -> SchedulerResult<ComponentHealth>;
}

/// 性能监控服务接口
///
/// 专注于性能指标查询和分析
#[async_trait]
pub trait PerformanceMonitoringService: Send + Sync {
    /// 获取性能指标
    async fn get_performance_metrics(
        &self,
        time_range: TimeRange,
    ) -> SchedulerResult<PerformanceMetrics>;

    /// 获取实时性能统计
    async fn get_realtime_stats(&self) -> SchedulerResult<RealtimeStats>;
}

/// 告警管理服务接口
///
/// 专注于告警规则和告警处理
#[async_trait]
pub trait AlertManagementService: Send + Sync {
    /// 设置告警规则
    async fn set_alert_rule(&self, rule: &AlertRule) -> SchedulerResult<()>;

    /// 检查告警
    async fn check_alerts(&self) -> SchedulerResult<Vec<Alert>>;

    /// 解决告警
    async fn resolve_alert(&self, alert_id: &str) -> SchedulerResult<()>;
}

/// 指标记录
#[derive(Debug, Clone)]
pub struct MetricRecord {
    pub name: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 事件记录
#[derive(Debug, Clone)]
pub struct EventRecord {
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 系统健康状态
#[derive(Debug, Clone)]
pub struct SystemHealth {
    /// 整体健康状态
    pub overall_status: HealthStatus,
    /// 组件健康状态
    pub components: HashMap<String, ComponentHealth>,
    /// 检查时间
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

/// 健康状态枚举
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

/// 组件健康状态
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    /// 状态
    pub status: HealthStatus,
    /// 描述信息
    pub message: Option<String>,
    /// 最后检查时间
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// 性能指标
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// 任务吞吐量（每秒）
    pub task_throughput: f64,
    /// 平均任务执行时间（毫秒）
    pub avg_execution_time_ms: f64,
    /// 任务成功率
    pub success_rate: f64,
    /// 系统资源使用情况
    pub resource_usage: ResourceUsage,
}

/// 实时统计信息
#[derive(Debug, Clone)]
pub struct RealtimeStats {
    /// 当前活跃任务数
    pub active_tasks: i64,
    /// 每分钟处理的任务数
    pub tasks_per_minute: f64,
    /// 当前系统负载
    pub current_load: f64,
}

/// 资源使用情况
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    /// CPU使用率
    pub cpu_usage_percent: f64,
    /// 内存使用量（MB）
    pub memory_usage_mb: u64,
    /// 磁盘使用量（MB）
    pub disk_usage_mb: u64,
    /// 网络I/O（MB）
    pub network_io_mb: u64,
}

/// 告警规则
#[derive(Debug, Clone)]
pub struct AlertRule {
    /// 规则ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 指标名称
    pub metric_name: String,
    /// 条件
    pub condition: AlertCondition,
    /// 阈值
    pub threshold: f64,
    /// 持续时间（秒）
    pub duration_seconds: u64,
    /// 是否启用
    pub enabled: bool,
}

/// 告警条件
#[derive(Debug, Clone)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
}

/// 告警
#[derive(Debug, Clone)]
pub struct Alert {
    /// 告警ID
    pub id: String,
    /// 规则ID
    pub rule_id: String,
    /// 告警级别
    pub level: AlertLevel,
    /// 告警消息
    pub message: String,
    /// 触发时间
    pub triggered_at: chrono::DateTime<chrono::Utc>,
    /// 是否已解决
    pub resolved: bool,
}

/// 告警级别
#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}