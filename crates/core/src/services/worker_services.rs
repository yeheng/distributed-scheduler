use crate::{
    models::{WorkerInfo, WorkerStatus},
    SchedulerResult,
};
use async_trait::async_trait;
use std::collections::HashMap;

/// Worker注册服务接口
///
/// 专注于Worker的注册和注销
#[async_trait]
pub trait WorkerRegistrationService: Send + Sync {
    /// 注册Worker
    async fn register_worker(&self, worker: &WorkerInfo) -> SchedulerResult<()>;

    /// 注销Worker
    async fn unregister_worker(&self, worker_id: &str) -> SchedulerResult<()>;

    /// 更新Worker状态
    async fn update_worker_status(
        &self,
        worker_id: &str,
        status: WorkerStatus,
    ) -> SchedulerResult<()>;
}

/// Worker查询服务接口
///
/// 专注于Worker信息查询
#[async_trait]
pub trait WorkerQueryService: Send + Sync {
    /// 获取活跃的Worker列表
    async fn get_active_workers(&self) -> SchedulerResult<Vec<WorkerInfo>>;

    /// 获取Worker详情
    async fn get_worker_details(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>>;

    /// 获取Worker负载统计
    async fn get_worker_load_stats(&self) -> SchedulerResult<HashMap<String, WorkerLoadStats>>;
}

/// Worker健康检查服务接口
///
/// 专注于Worker健康状态管理
#[async_trait]
pub trait WorkerHealthService: Send + Sync {
    /// 检查Worker健康状态
    async fn check_worker_health(&self, worker_id: &str) -> SchedulerResult<bool>;

    /// 处理Worker心跳
    async fn process_heartbeat(
        &self,
        worker_id: &str,
        heartbeat_data: &WorkerHeartbeat,
    ) -> SchedulerResult<()>;
}

/// Worker选择服务接口
///
/// 专注于Worker选择和负载均衡
#[async_trait]
pub trait WorkerSelectionService: Send + Sync {
    /// 选择最佳Worker执行任务
    async fn select_best_worker(&self, task_type: &str) -> SchedulerResult<Option<String>>;

    /// 选择多个Worker进行负载分发
    async fn select_workers_for_batch(
        &self,
        task_type: &str,
        count: usize,
    ) -> SchedulerResult<Vec<String>>;
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
