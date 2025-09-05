use async_trait::async_trait;
use scheduler_errors::SchedulerResult;

/// Worker 与 Dispatcher 通信的抽象接口
/// 封装了 Worker 节点向 Dispatcher 注册、发送心跳和注销的功能
#[async_trait]
pub trait DispatcherApiClient: Send + Sync {
    /// 向 Dispatcher 注册 Worker 节点
    /// 
    /// # Arguments
    /// * `supported_task_types` - 该 Worker 支持的任务类型列表
    /// 
    /// # Returns
    /// * `SchedulerResult<()>` - 注册成功返回 Ok(())，失败返回对应错误
    async fn register(&self, supported_task_types: Vec<String>) -> SchedulerResult<()>;

    /// 向 Dispatcher 发送心跳信息
    /// 
    /// # Arguments  
    /// * `current_task_count` - 当前正在执行的任务数量
    /// 
    /// # Returns
    /// * `SchedulerResult<()>` - 心跳发送成功返回 Ok(())，失败返回对应错误
    async fn send_heartbeat(&self, current_task_count: i32) -> SchedulerResult<()>;

    /// 从 Dispatcher 注销 Worker 节点
    /// 
    /// # Returns
    /// * `SchedulerResult<()>` - 注销成功返回 Ok(())，失败返回对应错误
    async fn unregister(&self) -> SchedulerResult<()>;

    /// 检查是否已配置 Dispatcher 连接
    /// 
    /// # Returns
    /// * `bool` - 如果已配置返回 true，否则返回 false
    fn is_configured(&self) -> bool;

    /// 获取 Dispatcher URL 配置
    /// 
    /// # Returns
    /// * `&Option<String>` - Dispatcher URL 的引用，未配置时为 None
    fn get_dispatcher_url(&self) -> &Option<String>;
}