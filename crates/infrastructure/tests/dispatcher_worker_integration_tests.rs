use chrono::Utc;
use scheduler_core::traits::MockMessageQueue;
use scheduler_core::{
    models::{
        Message, StatusUpdateMessage, Task, TaskExecutionMessage, TaskRun, TaskRunStatus,
        WorkerInfo, WorkerStatus,
    },
    traits::{MessageQueue, TaskRepository, TaskRunRepository, WorkerRepository},
};
use scheduler_infrastructure::database::postgres::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};
use sqlx::PgPool;
use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;

/// 测试环境设置
struct IntegrationTestSetup {
    _postgres_container: ContainerAsync<Postgres>,
    pub message_queue: MockMessageQueue,
    pub task_repo: PostgresTaskRepository,
    pub task_run_repo: PostgresTaskRunRepository,
    pub worker_repo: PostgresWorkerRepository,
}

impl IntegrationTestSetup {
    async fn new() -> Self {
        // 启动PostgreSQL容器
        let postgres_image = Postgres::default()
            .with_db_name("scheduler_integration_test")
            .with_user("test_user")
            .with_password("test_password")
            .with_tag("16-alpine");

        let postgres_container = postgres_image.start().await.unwrap();
        let db_connection_string = format!(
            "postgresql://test_user:test_password@127.0.0.1:{}/scheduler_integration_test",
            postgres_container.get_host_port_ipv4(5432).await.unwrap()
        );

        let pool = PgPool::connect(&db_connection_string).await.unwrap();

        // 运行数据库迁移
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

        // 创建Mock消息队列
        let message_queue = MockMessageQueue::new();

        // 创建仓储实例
        let task_repo = PostgresTaskRepository::new(pool.clone());
        let task_run_repo = PostgresTaskRunRepository::new(pool.clone());
        let worker_repo = PostgresWorkerRepository::new(pool.clone());

        Self {
            _postgres_container: postgres_container,
            message_queue,
            task_repo,
            task_run_repo,
            worker_repo,
        }
    }
}

#[tokio::test]
async fn test_task_dispatch_and_execution_flow() {
    let setup = IntegrationTestSetup::new().await;

    // 1. 创建测试任务
    let task = Task::new(
        "integration_test_task".to_string(),
        "shell".to_string(),
        "0 0 * * *".to_string(),
        serde_json::json!({"command": "echo 'Hello Integration Test'"}),
    );
    let created_task = setup.task_repo.create(&task).await.unwrap();

    // 2. 注册测试Worker
    let worker = WorkerInfo {
        id: "integration-test-worker".to_string(),
        hostname: "test-host".to_string(),
        ip_address: "127.0.0.1".to_string(),
        supported_task_types: vec!["shell".to_string()],
        max_concurrent_tasks: 5,
        current_task_count: 0,
        status: WorkerStatus::Alive,
        last_heartbeat: Utc::now(),
        registered_at: Utc::now(),
    };
    setup.worker_repo.register(&worker).await.unwrap();

    // 3. 创建任务执行实例
    let task_run = TaskRun::new(created_task.id, Utc::now());
    let created_run = setup.task_run_repo.create(&task_run).await.unwrap();

    // 4. 模拟Dispatcher发送任务到消息队列
    let task_execution_msg = TaskExecutionMessage {
        task_run_id: created_run.id,
        task_id: created_task.id,
        task_name: created_task.name.clone(),
        task_type: created_task.task_type.clone(),
        parameters: created_task.parameters.clone(),
        timeout_seconds: created_task.timeout_seconds,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let message = Message::task_execution(task_execution_msg);
    setup
        .message_queue
        .publish_message("tasks", &message)
        .await
        .unwrap();

    // 5. 模拟Worker接收任务并更新状态
    let received_messages = setup.message_queue.consume_messages("tasks").await.unwrap();
    assert!(!received_messages.is_empty());

    // 解析消息
    let received_message = &received_messages[0];
    let task_run_id = if let scheduler_core::models::MessageType::TaskExecution(ref msg) =
        received_message.message_type
    {
        msg.task_run_id
    } else {
        panic!("Expected TaskExecution message");
    };

    // 6. Worker更新任务状态为Running
    setup
        .task_run_repo
        .update_status(task_run_id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();

    // 验证状态更新
    let running_task_run = setup
        .task_run_repo
        .get_by_id(task_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(running_task_run.status, TaskRunStatus::Running);
    assert_eq!(running_task_run.worker_id, Some(worker.id.clone()));
    assert!(running_task_run.started_at.is_some());

    // 7. 模拟任务执行完成
    let execution_result = "Hello Integration Test";
    setup
        .task_run_repo
        .update_result(task_run_id, Some(execution_result), None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(task_run_id, TaskRunStatus::Completed, Some(&worker.id))
        .await
        .unwrap();

    // 8. 验证最终状态
    let completed_task_run = setup
        .task_run_repo
        .get_by_id(task_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed_task_run.status, TaskRunStatus::Completed);
    assert_eq!(
        completed_task_run.result,
        Some(execution_result.to_string())
    );
    assert!(completed_task_run.completed_at.is_some());

    // 9. 发送状态更新消息
    let status_update_msg = StatusUpdateMessage {
        task_run_id,
        status: TaskRunStatus::Completed,
        worker_id: worker.id.clone(),
        result: None,
        error_message: None,
        timestamp: Utc::now(),
    };

    let status_message = Message::status_update(status_update_msg);
    setup
        .message_queue
        .publish_message("status_updates", &status_message)
        .await
        .unwrap();

    // 验证状态更新消息
    let status_messages = setup
        .message_queue
        .consume_messages("status_updates")
        .await
        .unwrap();
    assert!(!status_messages.is_empty());

    let received_status = &status_messages[0];
    if let scheduler_core::models::MessageType::StatusUpdate(ref msg) = received_status.message_type
    {
        assert_eq!(msg.task_run_id, task_run_id);
        assert_eq!(msg.status, TaskRunStatus::Completed);
    } else {
        panic!("Expected StatusUpdate message");
    }

    // 清理
    setup.task_run_repo.delete(task_run_id).await.unwrap();
    setup.task_repo.delete(created_task.id).await.unwrap();
    setup.worker_repo.unregister(&worker.id).await.unwrap();
}
#[tokio::test]
async fn test_worker_heartbeat_and_failover() {
    let setup = IntegrationTestSetup::new().await;

    // 1. 注册两个Worker
    let worker1 = WorkerInfo {
        id: "worker-001".to_string(),
        hostname: "host-001".to_string(),
        ip_address: "127.0.0.1".to_string(),
        supported_task_types: vec!["shell".to_string()],
        max_concurrent_tasks: 5,
        current_task_count: 0,
        status: WorkerStatus::Alive,
        last_heartbeat: Utc::now(),
        registered_at: Utc::now(),
    };

    let worker2 = WorkerInfo {
        id: "worker-002".to_string(),
        hostname: "host-002".to_string(),
        ip_address: "127.0.0.2".to_string(),
        supported_task_types: vec!["shell".to_string()],
        max_concurrent_tasks: 3,
        current_task_count: 0,
        status: WorkerStatus::Alive,
        last_heartbeat: Utc::now(),
        registered_at: Utc::now(),
    };

    setup.worker_repo.register(&worker1).await.unwrap();
    setup.worker_repo.register(&worker2).await.unwrap();

    // 2. 验证两个Worker都已注册
    let alive_workers = setup.worker_repo.get_alive_workers().await.unwrap();
    assert_eq!(alive_workers.len(), 2);

    // 3. 创建任务并分配给worker1
    let task = Task::new(
        "failover_test_task".to_string(),
        "shell".to_string(),
        "0 0 * * *".to_string(),
        serde_json::json!({"command": "echo 'failover test'"}),
    );
    let created_task = setup.task_repo.create(&task).await.unwrap();

    let task_run = TaskRun::new(created_task.id, Utc::now());
    let created_run = setup.task_run_repo.create(&task_run).await.unwrap();

    // 4. 模拟worker1开始执行任务
    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Running, Some(&worker1.id))
        .await
        .unwrap();

    // 5. 更新worker1心跳
    setup
        .worker_repo
        .update_heartbeat(&worker1.id, Utc::now(), 1)
        .await
        .unwrap();

    // 6. 模拟worker1故障 - 设置为Down状态
    setup
        .worker_repo
        .update_status(&worker1.id, WorkerStatus::Down)
        .await
        .unwrap();

    // 7. 验证worker1状态已更新
    let failed_worker = setup
        .worker_repo
        .get_by_id(&worker1.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed_worker.status, WorkerStatus::Down);

    // 8. 获取活跃Worker列表（应该只有worker2）
    let alive_workers_after_failure = setup.worker_repo.get_alive_workers().await.unwrap();
    assert_eq!(alive_workers_after_failure.len(), 1);
    assert_eq!(alive_workers_after_failure[0].id, worker2.id);

    // 9. 模拟故障转移 - 将任务重新分配给worker2
    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Pending, None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Running, Some(&worker2.id))
        .await
        .unwrap();

    // 10. 验证任务已转移到worker2
    let recovered_task_run = setup
        .task_run_repo
        .get_by_id(created_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered_task_run.worker_id, Some(worker2.id.clone()));
    assert_eq!(recovered_task_run.status, TaskRunStatus::Running);

    // 11. 模拟worker2完成任务
    setup
        .task_run_repo
        .update_result(created_run.id, Some("failover test completed"), None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Completed, Some(&worker2.id))
        .await
        .unwrap();

    // 12. 验证任务成功完成
    let final_task_run = setup
        .task_run_repo
        .get_by_id(created_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_task_run.status, TaskRunStatus::Completed);
    assert_eq!(
        final_task_run.result,
        Some("failover test completed".to_string())
    );

    // 清理
    setup.task_run_repo.delete(created_run.id).await.unwrap();
    setup.task_repo.delete(created_task.id).await.unwrap();
    setup.worker_repo.unregister(&worker1.id).await.unwrap();
    setup.worker_repo.unregister(&worker2.id).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_task_execution() {
    let setup = IntegrationTestSetup::new().await;

    // 1. 注册一个支持并发的Worker
    let worker = WorkerInfo {
        id: "concurrent-worker".to_string(),
        hostname: "concurrent-host".to_string(),
        ip_address: "127.0.0.1".to_string(),
        supported_task_types: vec!["shell".to_string()],
        max_concurrent_tasks: 3,
        current_task_count: 0,
        status: WorkerStatus::Alive,
        last_heartbeat: Utc::now(),
        registered_at: Utc::now(),
    };
    setup.worker_repo.register(&worker).await.unwrap();

    // 2. 创建多个任务
    let mut tasks = Vec::new();
    let mut task_runs = Vec::new();

    for i in 0..3 {
        let task = Task::new(
            format!("concurrent_task_{}", i),
            "shell".to_string(),
            "0 0 * * *".to_string(),
            serde_json::json!({"command": format!("echo 'task {}'", i)}),
        );
        let created_task = setup.task_repo.create(&task).await.unwrap();
        tasks.push(created_task.clone());

        let task_run = TaskRun::new(created_task.id, Utc::now());
        let created_run = setup.task_run_repo.create(&task_run).await.unwrap();
        task_runs.push(created_run);
    }

    // 3. 模拟所有任务同时开始执行
    for (i, task_run) in task_runs.iter().enumerate() {
        setup
            .task_run_repo
            .update_status(task_run.id, TaskRunStatus::Running, Some(&worker.id))
            .await
            .unwrap();

        // 发送任务到消息队列
        let task_execution_msg = TaskExecutionMessage {
            task_run_id: task_run.id,
            task_id: task_run.task_id,
            task_name: format!("concurrent_task_{}", i),
            task_type: "shell".to_string(),
            parameters: serde_json::json!({"command": format!("echo 'task {}'", i)}),
            timeout_seconds: 300,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
        };

        let message = Message::task_execution(task_execution_msg);
        setup
            .message_queue
            .publish_message("tasks", &message)
            .await
            .unwrap();
    }

    // 4. 更新Worker的当前任务数量
    setup
        .worker_repo
        .update_heartbeat(&worker.id, Utc::now(), 3)
        .await
        .unwrap();

    // 5. 验证Worker负载统计
    let load_stats = setup.worker_repo.get_worker_load_stats().await.unwrap();
    let worker_stats = load_stats
        .iter()
        .find(|s| s.worker_id == worker.id)
        .unwrap();
    assert_eq!(worker_stats.current_task_count, 3);
    assert_eq!(worker_stats.max_concurrent_tasks, 3);
    assert_eq!(worker_stats.load_percentage, 100.0);

    // 6. 验证消息队列中有任务
    let task_messages = setup.message_queue.consume_messages("tasks").await.unwrap();
    assert_eq!(task_messages.len(), 3);

    // 7. 模拟任务逐个完成
    for (i, task_run) in task_runs.iter().enumerate() {
        // 完成任务
        setup
            .task_run_repo
            .update_result(task_run.id, Some(&format!("task {} completed", i)), None)
            .await
            .unwrap();

        setup
            .task_run_repo
            .update_status(task_run.id, TaskRunStatus::Completed, Some(&worker.id))
            .await
            .unwrap();

        // 发送状态更新
        let status_update_msg = StatusUpdateMessage {
            task_run_id: task_run.id,
            status: TaskRunStatus::Completed,
            worker_id: worker.id.clone(),
            result: None,
            error_message: None,
            timestamp: Utc::now(),
        };

        let status_message = Message::status_update(status_update_msg);
        setup
            .message_queue
            .publish_message("status_updates", &status_message)
            .await
            .unwrap();
    }

    // 8. 验证所有任务都已完成
    for task_run in &task_runs {
        let completed_run = setup
            .task_run_repo
            .get_by_id(task_run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(completed_run.status, TaskRunStatus::Completed);
        assert!(completed_run.result.is_some());
    }

    // 9. 验证状态更新消息
    let status_messages = setup
        .message_queue
        .consume_messages("status_updates")
        .await
        .unwrap();
    assert_eq!(status_messages.len(), 3);

    for status_msg in &status_messages {
        if let scheduler_core::models::MessageType::StatusUpdate(ref msg) = status_msg.message_type
        {
            assert_eq!(msg.status, TaskRunStatus::Completed);
        } else {
            panic!("Expected StatusUpdate message");
        }
    }

    // 清理
    for task_run in task_runs {
        setup.task_run_repo.delete(task_run.id).await.unwrap();
    }
    for task in tasks {
        setup.task_repo.delete(task.id).await.unwrap();
    }
    setup.worker_repo.unregister(&worker.id).await.unwrap();
}
