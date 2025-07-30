use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use scheduler_core::{
    models::TaskStatusUpdate, ApplicationContext, DefaultExecutorRegistry, MockMessageQueue,
    ServiceLocator, TaskRunStatus,
};
use tokio::time::sleep;

use crate::{WorkerService, WorkerServiceTrait as _};

/// 创建测试用的服务定位器
async fn create_test_service_locator() -> Arc<ServiceLocator> {
    // 简化版本，只注册消息队列
    let mut context = ApplicationContext::default();
    let message_queue = Arc::new(MockMessageQueue::new());
    context
        .container_mut()
        .register_message_queue(message_queue)
        .await
        .unwrap();
    Arc::new(ServiceLocator::new(Arc::new(context)))
}

/// 创建空的执行器注册表
fn create_empty_executor_registry() -> Arc<DefaultExecutorRegistry> {
    Arc::new(DefaultExecutorRegistry::new())
}

#[tokio::test]
async fn test_worker_service_creation() {
    let service_locator = create_test_service_locator().await;
    let executor_registry = create_empty_executor_registry();
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        service_locator,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .with_executor_registry(executor_registry)
    .max_concurrent_tasks(3)
    .heartbeat_interval_seconds(10)
    .poll_interval_ms(500)
    .build()
    .await
    .unwrap();

    assert_eq!(worker_service.get_supported_task_types().await.len(), 0);
    assert_eq!(worker_service.get_current_task_count().await, 0);
}

#[tokio::test]
async fn test_worker_service_start_stop() {
    let service_locator = create_test_service_locator().await;
    let executor_registry = create_empty_executor_registry();
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        service_locator,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .with_executor_registry(executor_registry)
    .build()
    .await
    .unwrap();

    // 测试启动
    assert!(worker_service.start().await.is_ok());

    // 等待一小段时间让服务启动
    sleep(Duration::from_millis(100)).await;

    // 测试停止
    assert!(worker_service.stop().await.is_ok());
}

#[tokio::test]
async fn test_worker_service_status_update() {
    let service_locator = create_test_service_locator().await;
    let executor_registry = create_empty_executor_registry();
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        service_locator,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .with_executor_registry(executor_registry)
    .build()
    .await
    .unwrap();

    let status_update = TaskStatusUpdate {
        task_run_id: 789,
        status: TaskRunStatus::Completed,
        worker_id: "test-worker".to_string(),
        result: Some("任务完成".to_string()),
        error_message: None,
        timestamp: Utc::now(),
    };

    assert!(worker_service
        .send_status_update(status_update)
        .await
        .is_ok());
}

#[tokio::test]
async fn test_worker_service_with_dispatcher_config() {
    let service_locator = create_test_service_locator().await;
    let executor_registry = create_empty_executor_registry();
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        service_locator,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .with_executor_registry(executor_registry)
    .dispatcher_url("http://localhost:8080".to_string())
    .hostname("test-host".to_string())
    .ip_address("192.168.1.100".to_string())
    .build()
    .await
    .unwrap();

    // Note: 没有getter方法，只能测试构建是否成功
    // 测试配置是否正确设置只能通过正常构建来验证
    assert_eq!(worker_service.get_supported_task_types().await.len(), 0);
}

#[tokio::test]
async fn test_worker_service_registration_without_dispatcher() {
    let service_locator = create_test_service_locator().await;
    let executor_registry = create_empty_executor_registry();
    let worker_service = WorkerService::builder(
        "test-worker".to_string(),
        service_locator,
        "task_queue".to_string(),
        "status_queue".to_string(),
    )
    .with_executor_registry(executor_registry)
    .build()
    .await
    .unwrap();

    // 没有配置Dispatcher URL时，注册应该成功但不执行实际注册
    assert!(worker_service.register_with_dispatcher().await.is_ok());
    assert!(worker_service.send_heartbeat_to_dispatcher().await.is_ok());
    assert!(worker_service.unregister_from_dispatcher().await.is_ok());
}
