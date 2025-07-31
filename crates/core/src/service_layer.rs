use async_trait::async_trait;
use std::collections::HashMap;

use crate::{
    SchedulerResult, errors::SchedulerError,
    models::{Task, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus},
};

/// 服务层抽象 - 任务控制服务
#[async_trait]
pub trait TaskControlService: Send + Sync {
    /// 手动触发任务
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;

    /// 暂停任务
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;

    /// 恢复任务
    async fn resume_task(&self, task_id: i64) -> SchedulerResult<()>;

    /// 重启任务运行实例
    async fn restart_task_run(&self, task_run_id: i64) -> SchedulerResult<TaskRun>;

    /// 中止任务运行实例
    async fn abort_task_run(&self, task_run_id: i64) -> SchedulerResult<()>;

    /// 批量取消任务的所有运行实例
    async fn cancel_all_task_runs(&self, task_id: i64) -> SchedulerResult<usize>;

    /// 检查任务是否有运行中的实例
    async fn has_running_instances(&self, task_id: i64) -> SchedulerResult<bool>;

    /// 获取任务的最近执行历史
    async fn get_recent_executions(&self, task_id: i64, limit: usize) -> SchedulerResult<Vec<TaskRun>>;
}

/// 调度器服务抽象
#[async_trait]
pub trait SchedulerService: Send + Sync {
    /// 启动调度器
    async fn start(&self) -> SchedulerResult<()>;

    /// 停止调度器
    async fn stop(&self) -> SchedulerResult<()>;

    /// 调度单个任务
    async fn schedule_task(&self, task: &Task) -> SchedulerResult<()>;

    /// 批量调度任务
    async fn schedule_tasks(&self, tasks: &[Task]) -> SchedulerResult<()>;

    /// 检查调度器状态
    async fn is_running(&self) -> bool;

    /// 获取调度器统计信息
    async fn get_stats(&self) -> SchedulerResult<SchedulerStats>;

    /// 重新加载调度配置
    async fn reload_config(&self) -> SchedulerResult<()>;
}

/// 调度器统计信息
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    /// 总任务数
    pub total_tasks: i64,
    /// 活跃任务数
    pub active_tasks: i64,
    /// 正在运行的任务实例数
    pub running_task_runs: i64,
    /// 待处理的任务实例数
    pub pending_task_runs: i64,
    /// 调度器运行时间（秒）
    pub uptime_seconds: u64,
    /// 最后调度时间
    pub last_schedule_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// Worker管理服务抽象
#[async_trait]
pub trait WorkerManagementService: Send + Sync {
    /// 注册Worker
    async fn register_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()>;

    /// 注销Worker
    async fn unregister_worker(&self, worker_id: &str) -> SchedulerResult<()>;

    /// 更新Worker状态
    async fn update_worker_status(&self, worker_id: &str, status: WorkerStatus) -> SchedulerResult<()>;

    /// 获取活跃的Worker列表
    async fn get_active_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;

    /// 获取Worker详情
    async fn get_worker_details(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;

    /// 检查Worker健康状态
    async fn check_worker_health(&self, worker_id: &str) -> SchedulerResult<bool>;

    /// 获取Worker负载统计
    async fn get_worker_load_stats(&self) -> SchedulerResult<HashMap<String, WorkerLoadStats>>;

    /// 选择最佳Worker执行任务
    async fn select_best_worker(&self, task_type: &str) -> SchedulerResult<Option<String>>;

    /// 处理Worker心跳
    async fn process_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_data: &WorkerHeartbeat,
    ) -> SchedulerResult<()>;
}

/// Worker负载统计
#[derive(Debug, Clone)]
pub struct WorkerLoadStats {
    /// Worker ID
    pub worker_id: String,
    /// 当前任务数
    pub current_task_count: i32,
    /// 最大并发任务数
    pub max_concurrent_tasks: i32,
    /// 系统负载
    pub system_load: Option<f64>,
    /// 内存使用量（MB）
    pub memory_usage_mb: Option<u64>,
    /// 最后心跳时间
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
}

/// Worker心跳数据
#[derive(Debug, Clone)]
pub struct WorkerHeartbeat {
    /// 当前任务数
    pub current_task_count: i32,
    /// 系统负载
    pub system_load: Option<f64>,
    /// 内存使用量（MB）
    pub memory_usage_mb: Option<u64>,
    /// 心跳时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 任务分发服务抽象
#[async_trait]
pub trait TaskDispatchService: Send + Sync {
    /// 分发任务到Worker
    async fn dispatch_task(&self, task_run: &TaskRun, worker_id: &str) -> SchedulerResult<()>;

    /// 批量分发任务
    async fn dispatch_tasks(&self, dispatches: &[(TaskRun, String)]) -> SchedulerResult<()>;

    /// 处理任务状态更新
    async fn handle_status_update(
        &self,
        task_run_id: i64,
        status: TaskRunStatus,
        error_message: Option<String>,
    ) -> SchedulerResult<()>;

    /// 重新分发失败的任务
    async fn redispatch_failed_tasks(&self) -> SchedulerResult<usize>;

    /// 获取分发统计信息
    async fn get_dispatch_stats(&self) -> SchedulerResult<DispatchStats>;
}

/// 分发统计信息
#[derive(Debug, Clone)]
pub struct DispatchStats {
    /// 总分发数
    pub total_dispatched: i64,
    /// 成功分发数
    pub successful_dispatched: i64,
    /// 失败分发数
    pub failed_dispatched: i64,
    /// 重新分发数
    pub redispatched: i64,
    /// 平均分发时间（毫秒）
    pub avg_dispatch_time_ms: f64,
}

/// 监控服务抽象
#[async_trait]
pub trait MonitoringService: Send + Sync {
    /// 记录指标
    async fn record_metric(
        &self,
        name: &str,
        value: f64,
        tags: &HashMap<String, String>,
    ) -> SchedulerResult<()>;

    /// 记录事件
    async fn record_event(&self, event_type: &str, data: &serde_json::Value) -> SchedulerResult<()>;

    /// 获取系统健康状态
    async fn get_system_health(&self) -> SchedulerResult<SystemHealth>;

    /// 获取性能指标
    async fn get_performance_metrics(&self, time_range: TimeRange) -> SchedulerResult<PerformanceMetrics>;

    /// 设置告警规则
    async fn set_alert_rule(&self, rule: &AlertRule) -> SchedulerResult<()>;

    /// 检查告警
    async fn check_alerts(&self) -> SchedulerResult<Vec<Alert>>;
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

/// 时间范围
#[derive(Debug, Clone)]
pub struct TimeRange {
    /// 开始时间
    pub start: chrono::DateTime<chrono::Utc>,
    /// 结束时间
    pub end: chrono::DateTime<chrono::Utc>,
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

/// 配置管理服务抽象
#[async_trait]
pub trait ConfigurationService: Send + Sync {
    /// 获取配置（返回JSON值）
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<serde_json::Value>>;

    /// 设置配置（接受JSON值）
    async fn set_config_value(&self, key: &str, value: &serde_json::Value) -> SchedulerResult<()>;

    /// 删除配置
    async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;

    /// 获取所有配置键
    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;

    /// 重新加载配置
    async fn reload_config(&self) -> SchedulerResult<()>;

    /// 监听配置变化
    async fn watch_config(&self, key: &str) -> SchedulerResult<Box<dyn ConfigWatcher>>;
}

/// 配置服务扩展 - 提供类型安全的配置访问
pub trait ConfigurationServiceExt {
    /// 获取配置（类型安全）
    fn get_config<T>(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = SchedulerResult<Option<T>>> + Send
    where
        T: serde::de::DeserializeOwned;

    /// 设置配置（类型安全）
    fn set_config<T>(
        &self,
        key: &str,
        value: &T,
    ) -> impl std::future::Future<Output = SchedulerResult<()>> + Send
    where
        T: serde::Serialize + std::marker::Sync;
}

/// 为所有ConfigurationService实现扩展方法
impl<C: ConfigurationService> ConfigurationServiceExt for C {
    async fn get_config<T>(&self, key: &str) -> SchedulerResult<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        match self.get_config_value(key).await? {
            Some(value) => {
                let typed_value: T = serde_json::from_value(value)
                    .map_err(|e| SchedulerError::Internal(format!("配置反序列化失败: {e}")))?;
                Ok(Some(typed_value))
            }
            None => Ok(None),
        }
    }

    async fn set_config<T>(&self, key: &str, value: &T) -> SchedulerResult<()>
    where
        T: serde::Serialize + std::marker::Sync,
    {
        let json_value = serde_json::to_value(value)
            .map_err(|e| SchedulerError::Internal(format!("配置序列化失败: {e}")))?;
        self.set_config_value(key, &json_value).await
    }
}

/// 配置监听器
#[async_trait]
pub trait ConfigWatcher: Send + Sync {
    /// 等待配置变化
    async fn wait_for_change(&mut self) -> SchedulerResult<ConfigChange>;

    /// 停止监听
    async fn stop(&mut self) -> SchedulerResult<()>;
}

/// 配置变化事件
#[derive(Debug, Clone)]
pub struct ConfigChange {
    /// 配置键
    pub key: String,
    /// 旧值
    pub old_value: Option<serde_json::Value>,
    /// 新值
    pub new_value: Option<serde_json::Value>,
    /// 变化时间
    pub changed_at: chrono::DateTime<chrono::Utc>,
}

/// 审计日志服务抽象
#[async_trait]
pub trait AuditLogService: Send + Sync {
    /// 记录审计日志
    async fn log_event(&self, event: &AuditEvent) -> SchedulerResult<()>;

    /// 查询审计日志
    async fn query_events(&self, query: &AuditQuery) -> SchedulerResult<Vec<AuditEvent>>;

    /// 获取审计统计
    async fn get_audit_stats(&self, time_range: TimeRange) -> SchedulerResult<AuditStats>;

    /// 导出审计日志
    async fn export_events(&self, query: &AuditQuery, format: ExportFormat) -> SchedulerResult<Vec<u8>>;
}

/// 审计事件
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// 事件ID
    pub id: String,
    /// 事件类型
    pub event_type: String,
    /// 用户ID
    pub user_id: Option<String>,
    /// 资源ID
    pub resource_id: Option<String>,
    /// 操作描述
    pub action: String,
    /// 结果
    pub result: AuditResult,
    /// 事件数据
    pub data: serde_json::Value,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// IP地址
    pub ip_address: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
}

/// 审计结果
#[derive(Debug, Clone)]
pub enum AuditResult {
    Success,
    Failure,
    Error,
}

/// 审计查询
#[derive(Debug, Clone)]
pub struct AuditQuery {
    /// 时间范围
    pub time_range: Option<TimeRange>,
    /// 事件类型过滤
    pub event_types: Vec<String>,
    /// 用户ID过滤
    pub user_ids: Vec<String>,
    /// 资源ID过滤
    pub resource_ids: Vec<String>,
    /// 分页限制
    pub limit: Option<usize>,
    /// 分页偏移
    pub offset: Option<usize>,
}

/// 审计统计
#[derive(Debug, Clone)]
pub struct AuditStats {
    /// 总事件数
    pub total_events: i64,
    /// 成功事件数
    pub success_events: i64,
    /// 失败事件数
    pub failure_events: i64,
    /// 错误事件数
    pub error_events: i64,
    /// 按事件类型分组的统计
    pub events_by_type: HashMap<String, i64>,
    /// 按用户分组的统计
    pub events_by_user: HashMap<String, i64>,
}

/// 导出格式
#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Excel,
    Pdf,
}

/// 服务工厂 - 创建服务实例
#[async_trait]
pub trait ServiceFactory: Send + Sync {
    /// 创建任务控制服务
    async fn create_task_control_service(&self) -> SchedulerResult<Box<dyn TaskControlService>>;

    /// 创建调度器服务
    async fn create_scheduler_service(&self) -> SchedulerResult<Box<dyn SchedulerService>>;

    /// 创建Worker管理服务
    async fn create_worker_management_service(&self) -> SchedulerResult<Box<dyn WorkerManagementService>>;

    /// 创建任务分发服务
    async fn create_task_dispatch_service(&self) -> SchedulerResult<Box<dyn TaskDispatchService>>;

    /// 创建监控服务
    async fn create_monitoring_service(&self) -> SchedulerResult<Box<dyn MonitoringService>>;

    /// 创建配置管理服务
    async fn create_configuration_service(&self) -> SchedulerResult<Box<dyn ConfigurationService>>;

    /// 创建审计日志服务
    async fn create_audit_log_service(&self) -> SchedulerResult<Box<dyn AuditLogService>>;
}
