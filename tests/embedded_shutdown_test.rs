use anyhow::Result;
use scheduler::shutdown::EmbeddedShutdownManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

#[tokio::test]
async fn test_embedded_shutdown_integration() -> Result<()> {
    // 创建嵌入式关闭管理器
    let shutdown_manager = EmbeddedShutdownManager::with_timeouts(2, 5);
    
    // 模拟运行任务的跟踪器
    let running_tasks = Arc::new(RwLock::new(std::collections::HashMap::new()));
    
    // 模拟添加一个运行中的任务
    {
        let mut tasks = running_tasks.write().await;
        tasks.insert(1, create_mock_task_run(1));
    }
    
    // 模拟任务等待逻辑
    let running_tasks_clone = Arc::clone(&running_tasks);
    let wait_for_tasks = async move {
        // 模拟任务在1秒后完成
        sleep(Duration::from_secs(1)).await;
        {
            let mut tasks = running_tasks_clone.write().await;
            tasks.clear();
        }
        Ok(())
    };
    
    // 模拟状态持久化
    let persist_state = async {
        // 模拟持久化操作
        sleep(Duration::from_millis(100)).await;
        Ok(())
    };
    
    // 模拟资源清理
    let force_cleanup = async {
        // 模拟清理操作
        sleep(Duration::from_millis(50)).await;
        Ok(())
    };
    
    // 执行优雅关闭
    let result = shutdown_manager.graceful_shutdown(wait_for_tasks, persist_state, force_cleanup).await;
    
    assert!(result.is_ok());
    assert!(shutdown_manager.is_shutdown().await);
    
    Ok(())
}

#[tokio::test]
async fn test_embedded_shutdown_with_timeout() -> Result<()> {
    // 创建嵌入式关闭管理器，设置较短的超时时间
    let shutdown_manager = EmbeddedShutdownManager::with_timeouts(1, 2);
    
    // 模拟任务等待超时
    let wait_for_tasks = async {
        // 模拟任务运行时间超过超时时间
        sleep(Duration::from_secs(2)).await;
        Ok(())
    };
    
    // 模拟状态持久化
    let persist_state = async {
        Ok(())
    };
    
    // 模拟资源清理
    let force_cleanup = async {
        Ok(())
    };
    
    // 执行优雅关闭（应该在任务等待超时后继续）
    let result = shutdown_manager.graceful_shutdown(wait_for_tasks, persist_state, force_cleanup).await;
    
    assert!(result.is_ok());
    assert!(shutdown_manager.is_shutdown().await);
    
    Ok(())
}

#[tokio::test]
async fn test_embedded_force_shutdown() -> Result<()> {
    // 创建嵌入式关闭管理器
    let shutdown_manager = EmbeddedShutdownManager::with_timeouts(1, 2);
    
    // 模拟强制清理
    let force_cleanup = async {
        sleep(Duration::from_millis(100)).await;
        Ok(())
    };
    
    // 执行强制关闭
    let result = shutdown_manager.force_shutdown(force_cleanup).await;
    
    assert!(result.is_ok());
    assert!(shutdown_manager.is_shutdown().await);
    
    Ok(())
}

// 辅助函数：创建模拟的任务运行记录
fn create_mock_task_run(id: i64) -> scheduler_domain::entities::TaskRun {
    use scheduler_domain::entities::{TaskRun, TaskRunStatus};
    use chrono::Utc;
    
    TaskRun {
        id,
        task_id: 1,
        status: TaskRunStatus::Running,
        worker_id: Some("test-worker".to_string()),
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        result: None,
        error_message: None,
        created_at: Utc::now(),
    }
}