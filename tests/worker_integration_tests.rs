use anyhow::Result;
use scheduler_config::models::ExecutorConfig;
use scheduler_worker::components::TaskExecutionManager;
use scheduler_worker::ExecutorFactory;
use serde_json::json;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing_test::traced_test;

/// 简化的Worker集成测试：专注于已实现的TaskExecutionManager功能
#[tokio::test]
#[traced_test]
async fn test_task_execution_manager_integration() -> Result<()> {
    println!("开始TaskExecutionManager集成测试");

    // 创建ExecutorFactory和Registry
    let executor_config = ExecutorConfig::default();
    let executor_factory = Arc::new(ExecutorFactory::new(executor_config));
    executor_factory.initialize().await?;
    
    let executor_registry: Arc<dyn scheduler_application::ExecutorRegistry> = executor_factory;

    // 创建TaskExecutionManager
    let manager = TaskExecutionManager::new(
        "test-worker".to_string(),
        executor_registry,
        2, // 最大并发数
    );

    // 验证基本功能
    assert_eq!(manager.get_current_task_count().await, 0);
    
    let supported_types = manager.get_supported_task_types().await;
    println!("支持的任务类型: {:?}", supported_types);
    
    // 验证Shell任务类型支持
    assert!(manager.can_accept_task("shell").await);
    
    println!("✅ TaskExecutionManager基础功能验证通过");

    // 创建一个测试任务消息
    let task_message = scheduler_domain::entities::TaskExecutionMessage {
        task_run_id: 1,
        task_id: 1,
        task_type: "shell".to_string(),
        parameters: json!({
            "command": "echo",
            "args": ["Integration test success"]
        }),
        timeout_seconds: 30,
        retry_count: 0,
        shard_index: Some(0),
        shard_total: Some(1),
        task_name: "integration_test".to_string(),
    };

    // 设置状态回调来捕获执行结果
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let status_callback = move |task_run_id, status, result, error_message| {
        let tx = tx.clone();
        async move {
            tx.send((task_run_id, status, result, error_message))
                .await
                .unwrap();
            Ok(())
        }
    };

    // 执行任务
    manager
        .handle_task_execution(task_message, status_callback)
        .await?;

    // 验证状态更新
    let (task_run_id, status, _result, _error) = rx.recv().await.unwrap();
    assert_eq!(task_run_id, 1);
    assert_eq!(status, scheduler_domain::entities::TaskRunStatus::Running);
    println!("✅ 任务开始运行");

    let (task_run_id, status, result, error) = rx.recv().await.unwrap();
    assert_eq!(task_run_id, 1);
    assert_eq!(status, scheduler_domain::entities::TaskRunStatus::Completed);
    assert!(result.is_some());
    assert!(error.is_none());
    println!("✅ 任务执行完成: {:?}", result);

    // 验证任务计数恢复
    assert_eq!(manager.get_current_task_count().await, 0);

    println!("✅ TaskExecutionManager集成测试通过");
    Ok(())
}

/// 测试任务取消功能的集成
#[tokio::test]
#[traced_test]
async fn test_task_cancellation_integration() -> Result<()> {
    println!("开始任务取消集成测试");

    let executor_config = ExecutorConfig::default();
    let executor_factory = Arc::new(ExecutorFactory::new(executor_config));
    executor_factory.initialize().await?;
    
    let executor_registry: Arc<dyn scheduler_application::ExecutorRegistry> = executor_factory;

    let manager = Arc::new(TaskExecutionManager::new(
        "test-worker".to_string(),
        executor_registry,
        1,
    ));

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    // 创建长时间运行的任务
    let task_message = scheduler_domain::entities::TaskExecutionMessage {
        task_run_id: 2,
        task_id: 2,
        task_type: "shell".to_string(),
        parameters: json!({
            "command": "sleep",
            "args": ["10"] // 10秒任务
        }),
        timeout_seconds: 60,
        retry_count: 0,
        shard_index: Some(0),
        shard_total: Some(1),
        task_name: "long_running_test".to_string(),
    };

    let status_callback = move |task_run_id, status, result, error_message| {
        let tx = tx.clone();
        async move {
            tx.send((task_run_id, status, result, error_message))
                .await
                .unwrap();
            Ok(())
        }
    };

    // 启动任务
    manager
        .handle_task_execution(task_message, status_callback)
        .await?;

    // 等待任务开始运行
    let (_task_run_id, status, _result, _error) = rx.recv().await.unwrap();
    assert_eq!(status, scheduler_domain::entities::TaskRunStatus::Running);
    println!("✅ 长时间任务开始运行");

    // 验证任务计数
    assert_eq!(manager.get_current_task_count().await, 1);

    // 取消任务
    let cancel_result = manager.cancel_task(2).await;
    assert!(cancel_result.is_ok());
    println!("✅ 任务取消调用成功");

    // 验证任务从运行列表中移除
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(manager.get_current_task_count().await, 0);
    println!("✅ 任务已从运行列表移除");

    println!("✅ 任务取消集成测试通过");
    Ok(())
}

/// 测试并发执行限制的集成
#[tokio::test]
#[traced_test]
async fn test_concurrency_limits_integration() -> Result<()> {
    println!("开始并发限制集成测试");

    let executor_config = ExecutorConfig::default();
    let executor_factory = Arc::new(ExecutorFactory::new(executor_config));
    executor_factory.initialize().await?;
    
    let executor_registry: Arc<dyn scheduler_application::ExecutorRegistry> = executor_factory;

    let manager = Arc::new(TaskExecutionManager::new(
        "test-worker".to_string(),
        executor_registry,
        2, // 最大并发数为2
    ));

    let (tx1, mut _rx1) = tokio::sync::mpsc::channel(10);
    let (tx2, mut _rx2) = tokio::sync::mpsc::channel(10);
    let (tx3, mut _rx3) = tokio::sync::mpsc::channel(10);

    // 创建3个任务，但只有2个应该能同时运行
    let tasks = vec![
        ("task1", tx1, 1),
        ("task2", tx2, 2),
        ("task3", tx3, 3),
    ];

    let mut handles = Vec::new();

    // 快速启动3个任务
    for (name, tx, id) in tasks {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            let task_message = scheduler_domain::entities::TaskExecutionMessage {
                task_run_id: id,
                task_id: id,
                task_type: "shell".to_string(),
                parameters: json!({
                    "command": "sleep",
                    "args": ["2"] // 2秒任务
                }),
                timeout_seconds: 30,
                retry_count: 0,
                shard_index: Some(0),
                shard_total: Some(1),
                task_name: name.to_string(),
            };

            let status_callback = move |task_run_id, status, result, error_message| {
                let tx = tx.clone();
                async move {
                    tx.send((task_run_id, status, result, error_message))
                        .await
                        .unwrap();
                    Ok(())
                }
            };

            manager_clone
                .handle_task_execution(task_message, status_callback)
                .await
                .unwrap();
        });
        handles.push(handle);
    }

    // 等待所有任务启动
    for handle in handles {
        handle.await.unwrap();
    }

    // 检查并发限制
    sleep(Duration::from_millis(100)).await;
    let concurrent_count = manager.get_current_task_count().await;
    println!("当前并发任务数: {}", concurrent_count);
    assert!(concurrent_count <= 2, "并发任务数不应超过限制");

    println!("✅ 并发限制集成测试通过");
    Ok(())
}