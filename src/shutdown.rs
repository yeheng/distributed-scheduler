use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info};

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
    pub async fn is_shutdown(&self) -> bool {
        *self.is_shutdown.read().await
    }

    /// 等待关闭完成
    pub async fn wait_for_shutdown(&self) {
        let mut rx = self.subscribe().await;
        let _ = rx.recv().await;
    }
}

impl Default for ShutdownManager {
    fn default() -> Self {
        Self::new()
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
        assert!(!manager.is_shutdown().await);

        // 订阅关闭信号
        let mut rx = manager.subscribe().await;

        // 触发关闭
        manager.shutdown().await;

        // 应该能收到关闭信号
        let result = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_ok());

        // 状态应该是已关闭
        assert!(manager.is_shutdown().await);
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
        assert!(manager.is_shutdown().await);

        // 第二次关闭应该是无操作
        manager.shutdown().await;
        assert!(manager.is_shutdown().await);
    }

    #[tokio::test]
    async fn test_wait_for_shutdown() {
        let manager = ShutdownManager::new();

        // 在另一个任务中等待关闭
        let manager_clone = manager.clone();
        let wait_handle = tokio::spawn(async move {
            manager_clone.wait_for_shutdown().await;
        });

        // 稍等一下然后触发关闭
        tokio::time::sleep(Duration::from_millis(10)).await;
        manager.shutdown().await;

        // 等待任务应该完成
        let result = timeout(Duration::from_millis(100), wait_handle).await;
        assert!(result.is_ok());
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
