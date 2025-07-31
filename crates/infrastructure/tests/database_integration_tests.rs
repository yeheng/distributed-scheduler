use chrono::Utc;
use scheduler_core::{
    models::{Task, TaskRun, TaskRunStatus, TaskStatus, WorkerInfo, WorkerStatus},
    traits::{TaskRepository, TaskRunRepository, WorkerRepository},
};
use scheduler_infrastructure::database::postgres::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};
use sqlx::PgPool;
use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::time::{sleep, Duration};

/// 测试数据库设置辅助函数
async fn setup_test_database() -> (ContainerAsync<Postgres>, PgPool) {
    let postgres_image = Postgres::default()
        .with_db_name("scheduler_test")
        .with_user("test_user")
        .with_password("test_password")
        .with_tag("16-alpine");

    let container = postgres_image.start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();

    let connection_string = format!(
        "postgresql://test_user:test_password@127.0.0.1:{}/scheduler_test",
        port
    );

    // Wait for database to be ready
    let mut retry_count = 0;
    let pool = loop {
        match PgPool::connect(&connection_string).await {
            Ok(pool) => break pool,
            Err(_) if retry_count < 30 => {
                retry_count += 1;
                sleep(Duration::from_millis(500)).await;
                continue;
            }
            Err(e) => panic!("Failed to connect to test database: {}", e),
        }
    };

    // 运行数据库迁移
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    (container, pool)
}

#[tokio::test]
async fn test_task_repository_integration() {
    let (_container, pool) = setup_test_database().await;
    let repo = PostgresTaskRepository::new(pool);

    // 测试创建任务
    let mut task = Task::new(
        "integration_test_task".to_string(),
        "shell".to_string(),
        "0 0 0 * * *".to_string(),
        serde_json::json!({"command": "echo integration test"}),
    );
    task.max_retries = 3;
    task.timeout_seconds = 300;

    let created_task = repo.create(&task).await.unwrap();
    assert!(created_task.id > 0);
    assert_eq!(created_task.name, task.name);
    assert_eq!(created_task.max_retries, 3);

    // 测试根据ID查询
    let found_task = repo.get_by_id(created_task.id).await.unwrap();
    assert!(found_task.is_some());
    let found_task = found_task.unwrap();
    assert_eq!(found_task.name, task.name);
    assert_eq!(found_task.task_type, task.task_type);
    assert_eq!(found_task.schedule, task.schedule);

    // 测试根据名称查询
    let found_by_name = repo.get_by_name(&task.name).await.unwrap();
    assert!(found_by_name.is_some());
    assert_eq!(found_by_name.unwrap().id, created_task.id);

    // 测试列出所有任务
    let filter = scheduler_core::models::TaskFilter::default();
    let all_tasks = repo.list(&filter).await.unwrap();
    assert!(!all_tasks.is_empty());
    assert!(all_tasks.iter().any(|t| t.id == created_task.id));

    // 测试按状态查询
    let active_filter = scheduler_core::models::TaskFilter {
        status: Some(TaskStatus::Active),
        ..Default::default()
    };
    let active_tasks = repo.list(&active_filter).await.unwrap();
    assert!(active_tasks.iter().any(|t| t.id == created_task.id));

    // 测试更新任务
    let mut updated_task = created_task.clone();
    updated_task.status = TaskStatus::Inactive;
    repo.update(&updated_task).await.unwrap();

    let retrieved_task = repo.get_by_id(created_task.id).await.unwrap().unwrap();
    assert_eq!(retrieved_task.status, TaskStatus::Inactive);

    // 测试删除任务
    repo.delete(created_task.id).await.unwrap();
    let deleted_task = repo.get_by_id(created_task.id).await.unwrap();
    assert!(deleted_task.is_none());
}

#[tokio::test]
async fn test_task_run_repository_integration() {
    let (_container, pool) = setup_test_database().await;
    let task_repo = PostgresTaskRepository::new(pool.clone());
    let run_repo = PostgresTaskRunRepository::new(pool);

    // 创建测试任务
    let task = Task::new(
        "test_task_for_runs".to_string(),
        "shell".to_string(),
        "0 0 0 * * *".to_string(),
        serde_json::json!({"command": "echo test"}),
    );
    let created_task = task_repo.create(&task).await.unwrap();

    // 创建任务执行实例
    let scheduled_time = Utc::now();
    let task_run = TaskRun::new(created_task.id, scheduled_time);
    let created_run = run_repo.create(&task_run).await.unwrap();

    assert!(created_run.id > 0);
    assert_eq!(created_run.task_id, created_task.id);
    assert_eq!(created_run.status, TaskRunStatus::Pending);
    assert_eq!(
        created_run.scheduled_at.naive_utc(),
        scheduled_time.naive_utc()
    );

    // 测试状态更新
    let worker_id = "integration-test-worker";
    run_repo
        .update_status(created_run.id, TaskRunStatus::Running, Some(worker_id))
        .await
        .unwrap();

    let updated_run = run_repo.get_by_id(created_run.id).await.unwrap().unwrap();
    assert_eq!(updated_run.status, TaskRunStatus::Running);
    assert_eq!(updated_run.worker_id, Some(worker_id.to_string()));
    assert!(updated_run.started_at.is_some());

    // 测试结果更新
    let result_output = "Integration test completed successfully";
    run_repo
        .update_result(created_run.id, Some(result_output), None)
        .await
        .unwrap();

    let result_updated_run = run_repo.get_by_id(created_run.id).await.unwrap().unwrap();
    assert_eq!(result_updated_run.result, Some(result_output.to_string()));

    // 测试完成状态更新
    run_repo
        .update_status(created_run.id, TaskRunStatus::Completed, Some(worker_id))
        .await
        .unwrap();

    let completed_run = run_repo.get_by_id(created_run.id).await.unwrap().unwrap();
    assert_eq!(completed_run.status, TaskRunStatus::Completed);
    assert!(completed_run.completed_at.is_some());

    // 测试按任务ID查询执行记录
    let task_runs = run_repo.get_by_task_id(created_task.id).await.unwrap();
    assert_eq!(task_runs.len(), 1);
    assert_eq!(task_runs[0].id, created_run.id);

    // 测试按状态查询执行记录
    let completed_runs = run_repo
        .get_by_status(TaskRunStatus::Completed)
        .await
        .unwrap();
    assert!(completed_runs.iter().any(|r| r.id == created_run.id));

    // 清理
    run_repo.delete(created_run.id).await.unwrap();
    task_repo.delete(created_task.id).await.unwrap();
}

#[tokio::test]
async fn test_worker_repository_integration() {
    let (_container, pool) = setup_test_database().await;
    let repo = PostgresWorkerRepository::new(pool);

    // 创建Worker信息
    let worker = WorkerInfo {
        id: "integration-test-worker-001".to_string(),
        hostname: "integration-test-host".to_string(),
        ip_address: "192.168.100.1".to_string(),
        supported_task_types: vec![
            "shell".to_string(),
            "http".to_string(),
            "python".to_string(),
        ],
        max_concurrent_tasks: 10,
        current_task_count: 0,
        status: WorkerStatus::Alive,
        last_heartbeat: Utc::now(),
        registered_at: Utc::now(),
    };

    // 测试Worker注册
    repo.register(&worker).await.unwrap();

    // 测试查询Worker
    let found_worker = repo.get_by_id(&worker.id).await.unwrap();
    assert!(found_worker.is_some());
    let found_worker = found_worker.unwrap();
    assert_eq!(found_worker.hostname, worker.hostname);
    // IP address might have CIDR notation added by PostgreSQL INET type
    assert!(found_worker.ip_address.starts_with(&worker.ip_address));
    assert_eq!(
        found_worker.supported_task_types,
        worker.supported_task_types
    );
    assert_eq!(
        found_worker.max_concurrent_tasks,
        worker.max_concurrent_tasks
    );
    assert_eq!(found_worker.status, WorkerStatus::Alive);

    // 测试列出所有Worker
    let all_workers = repo.list().await.unwrap();
    assert!(!all_workers.is_empty());
    assert!(all_workers.iter().any(|w| w.id == worker.id));

    // 测试获取活跃Worker
    let alive_workers = repo.get_alive_workers().await.unwrap();
    assert!(alive_workers.iter().any(|w| w.id == worker.id));

    // 测试心跳更新
    let new_heartbeat = Utc::now();
    let new_task_count = 3;
    repo.update_heartbeat(&worker.id, new_heartbeat, new_task_count)
        .await
        .unwrap();

    let heartbeat_updated_worker = repo.get_by_id(&worker.id).await.unwrap().unwrap();
    // current_task_count is calculated from running tasks, not stored directly
    assert_eq!(heartbeat_updated_worker.current_task_count, 3);
    // 注意：由于时间精度问题，这里只检查心跳时间是否更新了
    assert!(heartbeat_updated_worker.last_heartbeat >= worker.last_heartbeat);

    // 测试负载统计 (before setting status to Down)
    let load_stats = repo.get_worker_load_stats().await.unwrap();
    let worker_stats = load_stats.iter().find(|s| s.worker_id == worker.id);
    assert!(worker_stats.is_some());
    let worker_stats = worker_stats.unwrap();
    assert_eq!(worker_stats.max_concurrent_tasks, 10);
    assert_eq!(worker_stats.current_task_count, 0); // No running tasks
    assert_eq!(worker_stats.load_percentage, 0.0); // 0/10 * 100

    // 测试状态更新
    repo.update_status(&worker.id, WorkerStatus::Down)
        .await
        .unwrap();
    let status_updated_worker = repo.get_by_id(&worker.id).await.unwrap().unwrap();
    assert_eq!(status_updated_worker.status, WorkerStatus::Down);

    // 测试Worker注销
    repo.unregister(&worker.id).await.unwrap();
    let unregistered_worker = repo.get_by_id(&worker.id).await.unwrap();
    assert!(unregistered_worker.is_none());
}

#[tokio::test]
async fn test_database_transaction_rollback() {
    let (_container, pool) = setup_test_database().await;
    let task_repo = PostgresTaskRepository::new(pool.clone());

    // 创建一个任务
    let task = Task::new(
        "transaction_test_task".to_string(),
        "shell".to_string(),
        "0 0 0 * * *".to_string(),
        serde_json::json!({"command": "echo test"}),
    );

    let created_task = task_repo.create(&task).await.unwrap();

    // 开始事务并故意让它失败
    let mut tx = pool.begin().await.unwrap();

    // 在事务中删除任务
    sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(created_task.id)
        .execute(&mut *tx)
        .await
        .unwrap();

    // 回滚事务
    tx.rollback().await.unwrap();

    // 验证任务仍然存在
    let task_still_exists = task_repo.get_by_id(created_task.id).await.unwrap();
    assert!(task_still_exists.is_some());

    // 清理
    task_repo.delete(created_task.id).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_database_operations() {
    let (_container, pool) = setup_test_database().await;
    let task_repo = PostgresTaskRepository::new(pool.clone());

    // 并发创建多个任务
    let mut handles = vec![];
    for i in 0..10 {
        let repo = PostgresTaskRepository::new(pool.clone());
        let handle = tokio::spawn(async move {
            let task = Task::new(
                format!("concurrent_task_{}", i),
                "shell".to_string(),
                "0 0 0 * * *".to_string(),
                serde_json::json!({"command": format!("echo {}", i)}),
            );
            repo.create(&task).await
        });
        handles.push(handle);
    }

    // 等待所有任务创建完成
    let mut created_tasks = vec![];
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        created_tasks.push(result.unwrap());
    }

    // 验证所有任务都被创建
    assert_eq!(created_tasks.len(), 10);

    // 验证任务ID都是唯一的
    let mut ids = std::collections::HashSet::new();
    for task in &created_tasks {
        assert!(ids.insert(task.id));
    }

    // 清理所有创建的任务
    for task in created_tasks {
        task_repo.delete(task.id).await.unwrap();
    }
}

#[tokio::test]
async fn test_database_migration_integration() {
    let (_container, pool) = setup_test_database().await;

    // 验证所有必要的表都存在
    let tables_query = r#"
        SELECT table_name 
        FROM information_schema.tables 
        WHERE table_schema = 'public' 
        ORDER BY table_name
    "#;

    let tables: Vec<(String,)> = sqlx::query_as(tables_query).fetch_all(&pool).await.unwrap();

    let table_names: Vec<String> = tables.into_iter().map(|(name,)| name).collect();

    // 验证核心表存在
    assert!(table_names.contains(&"tasks".to_string()));
    assert!(table_names.contains(&"task_runs".to_string()));
    assert!(table_names.contains(&"workers".to_string()));
    assert!(table_names.contains(&"_sqlx_migrations".to_string()));

    // 验证表结构 - 检查tasks表的列
    let tasks_columns_query = r#"
        SELECT column_name, data_type, is_nullable
        FROM information_schema.columns 
        WHERE table_name = 'tasks' 
        ORDER BY ordinal_position
    "#;

    let columns: Vec<(String, String, String)> = sqlx::query_as(tasks_columns_query)
        .fetch_all(&pool)
        .await
        .unwrap();

    // 验证关键列存在
    let column_names: Vec<String> = columns.iter().map(|(name, _, _)| name.clone()).collect();
    assert!(column_names.contains(&"id".to_string()));
    assert!(column_names.contains(&"name".to_string()));
    assert!(column_names.contains(&"task_type".to_string()));
    assert!(column_names.contains(&"schedule".to_string()));
    assert!(column_names.contains(&"parameters".to_string()));
    assert!(column_names.contains(&"status".to_string()));
    assert!(column_names.contains(&"created_at".to_string()));
    assert!(column_names.contains(&"updated_at".to_string()));
}
