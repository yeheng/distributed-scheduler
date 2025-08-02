use super::TimeRange;
use crate::SchedulerResult;
use async_trait::async_trait;

/// 配置管理服务接口
///
/// 专注于配置的读取和写入
#[async_trait]
pub trait ConfigurationService: Send + Sync {
    /// 获取配置（返回JSON值）
    async fn get_config_value(&self, key: &str) -> SchedulerResult<Option<serde_json::Value>>;

    /// 设置配置（接受JSON值）
    async fn set_config_value(&self, key: &str, value: &serde_json::Value) -> SchedulerResult<()>;

    /// 删除配置
    async fn delete_config(&self, key: &str) -> SchedulerResult<bool>;
}

/// 配置查询服务接口
///
/// 专注于配置的查询和列举
#[async_trait]
pub trait ConfigurationQueryService: Send + Sync {
    /// 获取所有配置键
    async fn list_config_keys(&self) -> SchedulerResult<Vec<String>>;

    /// 搜索配置键
    async fn search_config_keys(&self, pattern: &str) -> SchedulerResult<Vec<String>>;
}

/// 配置热重载服务接口
///
/// 专注于配置的热重载功能
#[async_trait]
pub trait ConfigurationReloadService: Send + Sync {
    /// 重新加载配置
    async fn reload_config(&self) -> SchedulerResult<()>;

    /// 监听配置变化
    async fn watch_config(&self, key: &str) -> SchedulerResult<Box<dyn ConfigWatcher>>;
}

/// 审计日志服务接口
///
/// 专注于审计日志的记录
#[async_trait]
pub trait AuditLogService: Send + Sync {
    /// 记录审计日志
    async fn log_event(&self, event: &AuditEvent) -> SchedulerResult<()>;

    /// 批量记录审计日志
    async fn log_events(&self, events: &[AuditEvent]) -> SchedulerResult<()>;
}

/// 审计查询服务接口
///
/// 专注于审计日志的查询和分析
#[async_trait]
pub trait AuditQueryService: Send + Sync {
    /// 查询审计日志
    async fn query_events(&self, query: &AuditQuery) -> SchedulerResult<Vec<AuditEvent>>;

    /// 获取审计统计
    async fn get_audit_stats(&self, time_range: TimeRange) -> SchedulerResult<AuditStats>;

    /// 导出审计日志
    async fn export_events(
        &self,
        query: &AuditQuery,
        format: ExportFormat,
    ) -> SchedulerResult<Vec<u8>>;
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
                let typed_value: T = serde_json::from_value(value).map_err(|e| {
                    crate::errors::SchedulerError::Internal(format!("配置反序列化失败: {e}"))
                })?;
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
            .map_err(|e| crate::errors::SchedulerError::Internal(format!("配置序列化失败: {e}")))?;
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
    pub events_by_type: std::collections::HashMap<String, i64>,
    /// 按用户分组的统计
    pub events_by_user: std::collections::HashMap<String, i64>,
}

/// 导出格式
#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Excel,
    Pdf,
}
