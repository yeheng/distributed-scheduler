use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn, error};

/// 优雅关闭管理器
pub struct ShutdownManager {
    /// 关闭信号发送器
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,
    /// 是否已经关闭
    is_shutdown: Arc<RwLock<bool>>,
}

impl ShutdownManager {
    /// 创建新的关闭管理器
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(16);

        Self {
            shutdown_tx: Arc::new(RwLock::new(Some(shutdown_tx))),
            is_shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// 订阅关闭信号
    pub async fn subscribe(&self) -> broadcast::Receiver<()> {
        let shutdown_tx = self.shutdown_tx.read().await;
        if let Some(ref tx) = *shutdown_tx {
            tx.subscribe()
        } else {
            // 如果已经关闭，创建一个立即触发的接收器
            let (tx, rx) = broadcast::channel(1);
            let _ = tx.send(());
            rx
        }
    }

    /// 触发关闭
    pub async fn shutdown(&self) {
        let mut is_shutdown = self.is_shutdown.write().await;
        if *is_shutdown {
            debug!("关闭管理器已经触发过关闭");
            return;
        }

        info!("触发系统关闭");
        *is_shutdown = true;

        // 发送关闭信号
        let shutdown_tx = self.shutdown_tx.read().await;
        if let Some(ref tx) = *shutdown_tx {
            let subscriber_count = tx.receiver_count();
            debug!("发送关闭信号给 {} 个订阅者", subscriber_count);

            // 发送关闭信号，忽略错误（可能没有接收者）
            let _ = tx.send(());
        }

        // 清理发送器
        drop(shutdown_tx);
        let mut shutdown_tx = self.shutdown_tx.write().await;
        *shutdown_tx = None;

        info!("关闭信号已发送");
    }

    /// 检查是否已经关闭
    pub async fn _is_shutdown(&self) -> bool {
        *self.is_shutdown.read().await
    }

    /// 等待关闭完成
    pub async fn _wait_for_shutdown(&self) {
        let mut rx = self.subscribe().await;
        let _ = rx.recv().await;
    }
}

impl Default for ShutdownManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ShutdownManager {
    fn clone(&self) -> Self {
        Self {
            shutdown_tx: Arc::clone(&self.shutdown_tx),
            is_shutdown: Arc::clone(&self.is_shutdown),
        }
    }
}

/// 嵌入式优雅关闭管理器
/// 扩展基础的 ShutdownManager，支持嵌入式模式的特殊需求
pub struct EmbeddedShutdownManager {
    /// 基础关闭管理器
    base_manager: ShutdownManager,
    /// 关闭超时时间（秒）
    shutdown_timeout_seconds: u64,
    /// 强制关闭超时时间（秒）
    force_shutdown_timeout_seconds: u64,
}

impl EmbeddedShutdownManager {
    /// 创建新的嵌入式关闭管理器
    pub fn new() -> Self {
        Self {
            base_manager: ShutdownManager::new(),
            shutdown_timeout_seconds: 30,  // 默认30秒等待任务完成
            force_shutdown_timeout_seconds: 60,  // 默认60秒强制关闭
        }
    }

    /// 创建带自定义超时的嵌入式关闭管理器
    pub fn with_timeouts(shutdown_timeout_seconds: u64, force_shutdown_timeout_seconds: u64) -> Self {
        Self {
            base_manager: ShutdownManager::new(),
            shutdown_timeout_seconds,
            force_shutdown_timeout_seconds,
        }
    }

    /// 订阅关闭信号
    pub async fn subscribe(&self) -> broadcast::Receiver<()> {
        self.base_manager.subscribe().await
    }

    /// 检查是否已经关闭
    pub async fn is_shutdown(&self) -> bool {
        self.base_manager._is_shutdown().await
    }

    /// 优雅关闭嵌入式应用
    /// 
    /// 执行以下步骤：
    /// 1. 发送关闭信号给所有组件
    /// 2. 等待正在执行的任务完成（带超时）
    /// 3. 持久化系统状态到数据库
    /// 4. 如果超时，执行强制关闭
    pub async fn graceful_shutdown<F, G, H>(&self, 
        wait_for_tasks: F,
        persist_state: G,
        force_cleanup: H,
    ) -> Result<(), String>
    where
        F: std::future::Future<Output = Result<(), String>> + Send,
        G: std::future::Future<Output = Result<(), String>> + Send,
        H: std::future::Future<Output = Result<(), String>> + Send,
    {
        info!("开始嵌入式应用优雅关闭流程");

        // 1. 发送关闭信号
        self.base_manager.shutdown().await;
        info!("关闭信号已发送给所有组件");

        // 2. 等待正在执行的任务完成（带超时）
        info!("等待正在执行的任务完成（超时: {}秒）", self.shutdown_timeout_seconds);
        
        let wait_result = timeout(
            Duration::from_secs(self.shutdown_timeout_seconds),
            wait_for_tasks
        ).await;

        match wait_result {
            Ok(Ok(())) => {
                info!("所有任务已成功完成");
            }
            Ok(Err(e)) => {
                warn!("等待任务完成时发生错误: {}", e);
            }
            Err(_) => {
                warn!("等待任务完成超时（{}秒），继续关闭流程", self.shutdown_timeout_seconds);
            }
        }

        // 3. 持久化系统状态到数据库
        info!("持久化系统状态到数据库");
        
        let persist_result = timeout(
            Duration::from_secs(10), // 给持久化10秒时间
            persist_state
        ).await;

        match persist_result {
            Ok(Ok(())) => {
                info!("系统状态已成功持久化");
            }
            Ok(Err(e)) => {
                error!("持久化系统状态失败: {}", e);
            }
            Err(_) => {
                error!("持久化系统状态超时");
            }
        }

        // 4. 如果需要，执行强制关闭清理
        info!("执行最终清理");
        
        let cleanup_result = timeout(
            Duration::from_secs(5), // 给清理5秒时间
            force_cleanup
        ).await;

        match cleanup_result {
            Ok(Ok(())) => {
                info!("清理操作完成");
            }
            Ok(Err(e)) => {
                error!("清理操作失败: {}", e);
            }
            Err(_) => {
                error!("清理操作超时");
            }
        }

        info!("嵌入式应用优雅关闭流程完成");
        Ok(())
    }

    /// 强制关闭（当优雅关闭超时时使用）
    pub async fn force_shutdown<F>(&self, force_cleanup: F) -> Result<(), String>
    where
        F: std::future::Future<Output = Result<(), String>> + Send,
    {
        warn!("执行强制关闭流程");

        // 发送关闭信号（如果还没发送）
        self.base_manager.shutdown().await;

        // 执行强制清理（带超时）
        let cleanup_result = timeout(
            Duration::from_secs(self.force_shutdown_timeout_seconds),
            force_cleanup
        ).await;

        match cleanup_result {
            Ok(Ok(())) => {
                info!("强制清理完成");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("强制清理失败: {}", e);
                Err(e)
            }
            Err(_) => {
                error!("强制清理超时（{}秒）", self.force_shutdown_timeout_seconds);
                Err("强制清理超时".to_string())
            }
        }
    }

    /// 等待关闭完成
    pub async fn wait_for_shutdown(&self) {
        self.base_manager._wait_for_shutdown().await;
    }
}

impl Default for EmbeddedShutdownManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EmbeddedShutdownManager {
    fn clone(&self) -> Self {
        Self {
            base_manager: self.base_manager.clone(),
            shutdown_timeout_seconds: self.shutdown_timeout_seconds,
            force_shutdown_timeout_seconds: self.force_shutdown_timeout_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_shutdown_manager_basic() {
        let manager = ShutdownManager::new();

        // 初始状态应该是未关闭
        assert!(!manager._is_shutdown().await);

        // 订阅关闭信号
        let mut rx = manager.subscribe().await;

        // 触发关闭
        manager.shutdown().await;

        // 应该能收到关闭信号
        let result = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_ok());

        // 状态应该是已关闭
        assert!(manager._is_shutdown().await);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let manager = ShutdownManager::new();

        // 创建多个订阅者
        let mut rx1 = manager.subscribe().await;
        let mut rx2 = manager.subscribe().await;
        let mut rx3 = manager.subscribe().await;

        // 触发关闭
        manager.shutdown().await;

        // 所有订阅者都应该收到信号
        let result1 = timeout(Duration::from_millis(100), rx1.recv()).await;
        let result2 = timeout(Duration::from_millis(100), rx2.recv()).await;
        let result3 = timeout(Duration::from_millis(100), rx3.recv()).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());
    }

    #[tokio::test]
    async fn test_subscribe_after_shutdown() {
        let manager = ShutdownManager::new();

        // 先触发关闭
        manager.shutdown().await;

        // 然后订阅（应该立即收到信号）
        let mut rx = manager.subscribe().await;

        let result = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_double_shutdown() {
        let manager = ShutdownManager::new();

        // 第一次关闭
        manager.shutdown().await;
        assert!(manager._is_shutdown().await);

        // 第二次关闭应该是无操作
        manager.shutdown().await;
        assert!(manager._is_shutdown().await);
    }

    #[tokio::test]
    async fn test_wait_for_shutdown() {
        let manager = ShutdownManager::new();

        // 在另一个任务中等待关闭
        let manager_clone = manager.clone();
        let wait_handle = tokio::spawn(async move {
            manager_clone._wait_for_shutdown().await;
        });

        // 稍等一下然后触发关闭
        tokio::time::sleep(Duration::from_millis(10)).await;
        manager.shutdown().await;

        // 等待任务应该完成
        let result = timeout(Duration::from_millis(100), wait_handle).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_embedded_shutdown_manager_creation() {
        let manager = EmbeddedShutdownManager::new();
        assert!(!manager.is_shutdown().await);
    }

    #[tokio::test]
    async fn test_embedded_shutdown_manager_with_custom_timeouts() {
        let manager = EmbeddedShutdownManager::with_timeouts(10, 20);
        assert!(!manager.is_shutdown().await);
        assert_eq!(manager.shutdown_timeout_seconds, 10);
        assert_eq!(manager.force_shutdown_timeout_seconds, 20);
    }

    #[tokio::test]
    async fn test_embedded_graceful_shutdown_success() {
        let manager = EmbeddedShutdownManager::with_timeouts(1, 2);

        // 模拟成功的任务等待
        let wait_for_tasks = async { Ok(()) };
        
        // 模拟成功的状态持久化
        let persist_state = async { Ok(()) };
        
        // 模拟成功的清理
        let force_cleanup = async { Ok(()) };

        let result = manager.graceful_shutdown(wait_for_tasks, persist_state, force_cleanup).await;
        assert!(result.is_ok());
        assert!(manager.is_shutdown().await);
    }

    #[tokio::test]
    async fn test_embedded_graceful_shutdown_with_task_timeout() {
        let manager = EmbeddedShutdownManager::with_timeouts(1, 2);

        // 模拟任务等待超时
        let wait_for_tasks = async {
            sleep(Duration::from_secs(2)).await;
            Ok(())
        };
        
        // 模拟成功的状态持久化
        let persist_state = async { Ok(()) };
        
        // 模拟成功的清理
        let force_cleanup = async { Ok(()) };

        let result = manager.graceful_shutdown(wait_for_tasks, persist_state, force_cleanup).await;
        assert!(result.is_ok()); // 应该成功，即使任务等待超时
        assert!(manager.is_shutdown().await);
    }

    #[tokio::test]
    async fn test_embedded_graceful_shutdown_with_persist_error() {
        let manager = EmbeddedShutdownManager::with_timeouts(1, 2);

        // 模拟成功的任务等待
        let wait_for_tasks = async { Ok(()) };
        
        // 模拟状态持久化失败
        let persist_state = async { Err("Database error".to_string()) };
        
        // 模拟成功的清理
        let force_cleanup = async { Ok(()) };

        let result = manager.graceful_shutdown(wait_for_tasks, persist_state, force_cleanup).await;
        assert!(result.is_ok()); // 应该成功，即使持久化失败
        assert!(manager.is_shutdown().await);
    }

    #[tokio::test]
    async fn test_embedded_force_shutdown() {
        let manager = EmbeddedShutdownManager::with_timeouts(1, 2);

        // 模拟成功的强制清理
        let force_cleanup = async { Ok(()) };

        let result = manager.force_shutdown(force_cleanup).await;
        assert!(result.is_ok());
        assert!(manager.is_shutdown().await);
    }

    #[tokio::test]
    async fn test_embedded_force_shutdown_with_cleanup_error() {
        let manager = EmbeddedShutdownManager::with_timeouts(1, 2);

        // 模拟强制清理失败
        let force_cleanup = async { Err("Cleanup failed".to_string()) };

        let result = manager.force_shutdown(force_cleanup).await;
        assert!(result.is_err());
        assert!(manager.is_shutdown().await);
    }

    #[tokio::test]
    async fn test_embedded_force_shutdown_with_timeout() {
        let manager = EmbeddedShutdownManager::with_timeouts(1, 1);

        // 模拟强制清理超时
        let force_cleanup = async {
            sleep(Duration::from_secs(2)).await;
            Ok(())
        };

        let result = manager.force_shutdown(force_cleanup).await;
        assert!(result.is_err());
        assert!(manager.is_shutdown().await);
    }

    #[tokio::test]
    async fn test_embedded_shutdown_manager_clone() {
        let manager = EmbeddedShutdownManager::new();
        let cloned = manager.clone();

        // 原始管理器触发关闭
        let wait_for_tasks = async { Ok(()) };
        let persist_state = async { Ok(()) };
        let force_cleanup = async { Ok(()) };

        let result = manager.graceful_shutdown(wait_for_tasks, persist_state, force_cleanup).await;
        assert!(result.is_ok());

        // 克隆的管理器应该也显示已关闭
        assert!(cloned.is_shutdown().await);
    }
}
