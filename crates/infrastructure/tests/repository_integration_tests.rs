use anyhow::Result;
use chrono::Utc;
use scheduler_core::models::{
    Task, TaskFilter, TaskRun, TaskRunStatus, TaskStatus, WorkerInfo, WorkerStatus,
};
use scheduler_core::traits::{TaskRepository, TaskRunRepository, WorkerRepository};
use scheduler_infrastructure::database::postgres_repositories::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};

mod database_test_utils;
use database_test_utils::DatabaseTestContainer;

/// Integration tests for PostgresTaskRepository
#[tokio::test]
async fn test_postgres_task_repository_crud() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let repo = PostgresTaskRepository::new(container.pool.clone());

    // Test create task
    let task = Task {
        id: 0,
        name: "integration_test_task".to_string(),
        task_type: "shell".to_string(),
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'hello'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created_task = repo.create(&task).await?;
    assert!(created_task.id > 0);
    assert_eq!(created_task.name, task.name);
    assert_eq!(created_task.task_type, task.task_type);

    // Test get task
    let retrieved_task = repo.get_by_id(created_task.id).await?;
    assert!(retrieved_task.is_some());
    let retrieved_task = retrieved_task.unwrap();
    assert_eq!(retrieved_task.id, created_task.id);
    assert_eq!(retrieved_task.name, created_task.name);

    // Test update task
    let mut updated_task = retrieved_task.clone();
    updated_task.name = "updated_integration_test_task".to_string();
    updated_task.max_retries = 5;

    repo.update(&updated_task).await?;

    let retrieved_updated_task = repo.get_by_id(updated_task.id).await?;
    assert!(retrieved_updated_task.is_some());
    let retrieved_updated_task = retrieved_updated_task.unwrap();
    assert_eq!(retrieved_updated_task.name, "updated_integration_test_task");
    assert_eq!(retrieved_updated_task.max_retries, 5);

    // Test list tasks
    let filter = TaskFilter {
        status: Some(TaskStatus::Active),
        task_type: Some("shell".to_string()),
        name_pattern: None,
        limit: Some(10),
        offset: Some(0),
    };

    let tasks = repo.list(&filter).await?;
    assert!(!tasks.is_empty());
    assert!(tasks.iter().any(|t| t.id == created_task.id));

    // Test get active tasks
    let active_tasks = repo.get_active_tasks().await?;
    assert!(!active_tasks.is_empty());

    Ok(())
}

/// Integration tests for PostgresTaskRunRepository
#[tokio::test]
async fn test_postgres_task_run_repository_crud() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    // First create a task to reference
    let task_id = container
        .insert_test_task("test_task", "shell", "0 0 * * *")
        .await?;

    let repo = PostgresTaskRunRepository::new(container.pool.clone());

    // Test create task run
    let task_run = TaskRun {
        id: 0,
        task_id,
        status: TaskRunStatus::Pending,
        worker_id: None,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: Utc::now(),
        started_at: None,
        completed_at: None,
        result: None,
        error_message: None,
        created_at: Utc::now(),
    };

    let created_task_run = repo.create(&task_run).await?;
    assert!(created_task_run.id > 0);
    assert_eq!(created_task_run.task_id, task_id);
    assert_eq!(created_task_run.status, TaskRunStatus::Pending);

    // Test update task run status
    repo.update_status(created_task_run.id, TaskRunStatus::Running, None)
        .await?;

    // Verify the status was updated
    let updated_run = repo.get_by_id(created_task_run.id).await?;
    assert!(updated_run.is_some());
    assert_eq!(updated_run.unwrap().status, TaskRunStatus::Running);

    // Test update with result
    repo.update_status(created_task_run.id, TaskRunStatus::Completed, None)
        .await?;

    repo.update_result(
        created_task_run.id,
        Some("Task completed successfully"),
        None,
    )
    .await?;

    let completed_run = repo.get_by_id(created_task_run.id).await?;
    assert!(completed_run.is_some());
    let completed_run = completed_run.unwrap();
    assert_eq!(completed_run.status, TaskRunStatus::Completed);
    assert_eq!(
        completed_run.result,
        Some("Task completed successfully".to_string())
    );

    // Test get pending task runs
    let pending_task_run = TaskRun {
        id: 0,
        task_id,
        status: TaskRunStatus::Pending,
        worker_id: None,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: Utc::now(),
        started_at: None,
        completed_at: None,
        result: None,
        error_message: None,
        created_at: Utc::now(),
    };

    let pending_run = repo.create(&pending_task_run).await?;
    let pending_runs = repo.get_pending_runs(None).await?;
    assert!(!pending_runs.is_empty());
    assert!(pending_runs.iter().any(|tr| tr.id == pending_run.id));

    // Test get running tasks for worker
    repo.update_status(
        pending_run.id,
        TaskRunStatus::Running,
        Some("test_worker_1"),
    )
    .await?;

    let running_tasks = repo.get_by_worker_id("test_worker_1").await?;
    assert!(!running_tasks.is_empty());
    assert!(running_tasks.iter().any(|tr| tr.id == pending_run.id));

    Ok(())
}

/// Integration tests for PostgresWorkerRepository  
#[tokio::test]
async fn test_postgres_worker_repository_crud() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let repo = PostgresWorkerRepository::new(container.pool.clone());

    // Test create/register worker
    let worker = WorkerInfo {
        id: "integration_test_worker".to_string(),
        hostname: "test-host".to_string(),
        ip_address: "192.168.1.100".to_string(),
        supported_task_types: vec!["shell".to_string(), "http".to_string()],
        max_concurrent_tasks: 10,
        current_task_count: 0,
        status: WorkerStatus::Alive,
        last_heartbeat: Utc::now(),
        registered_at: Utc::now(),
    };

    repo.register(&worker).await?;

    // Test get worker
    let retrieved_worker = repo.get_by_id(&worker.id).await?;
    assert!(retrieved_worker.is_some());
    let retrieved_worker = retrieved_worker.unwrap();
    assert_eq!(retrieved_worker.id, worker.id);
    assert_eq!(retrieved_worker.hostname, worker.hostname);
    assert_eq!(
        retrieved_worker.supported_task_types,
        worker.supported_task_types
    );

    // Test update worker heartbeat
    let new_heartbeat = Utc::now();
    repo.update_heartbeat(&worker.id, new_heartbeat, 2).await?;

    let updated_worker = repo.get_by_id(&worker.id).await?;
    assert!(updated_worker.is_some());
    let updated_worker = updated_worker.unwrap();
    // Note: Due to precision differences, we check if the heartbeat is close
    assert!(updated_worker.last_heartbeat >= new_heartbeat - chrono::Duration::seconds(1));

    // Test list active workers
    let active_workers = repo.get_alive_workers().await?;
    assert!(!active_workers.is_empty());
    assert!(active_workers.iter().any(|w| w.id == worker.id));

    // Test mark worker as down
    repo.update_status(&worker.id, WorkerStatus::Down).await?;

    let down_worker = repo.get_by_id(&worker.id).await?;
    assert!(down_worker.is_some());
    assert_eq!(down_worker.unwrap().status, WorkerStatus::Down);

    Ok(())
}

/// Performance test for repository operations
#[tokio::test]
async fn test_repository_performance() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());

    let start = std::time::Instant::now();

    // Create multiple tasks concurrently
    let tasks: Vec<Task> = (0..50)
        .map(|i| Task {
            id: 0,
            name: format!("perf_test_task_{}", i),
            task_type: "shell".to_string(),
            schedule: "0 0 * * *".to_string(),
            parameters: serde_json::json!({"command": format!("echo 'task {}'", i)}),
            timeout_seconds: 300,
            max_retries: 3,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .collect();

    let mut create_futures = Vec::new();
    for task in &tasks {
        create_futures.push(task_repo.create(task));
    }

    let results = futures::future::join_all(create_futures).await;
    let successful_creates = results.into_iter().filter(|r| r.is_ok()).count();

    let create_duration = start.elapsed();

    println!(
        "Created {} tasks in {:?}",
        successful_creates, create_duration
    );
    assert!(successful_creates > 45); // Allow for some failures in concurrent operations

    // Test bulk query performance
    let query_start = std::time::Instant::now();

    let filter = TaskFilter {
        status: Some(TaskStatus::Active),
        task_type: Some("shell".to_string()),
        name_pattern: None,
        limit: Some(1000),
        offset: Some(0),
    };

    let tasks = task_repo.list(&filter).await?;
    let query_duration = query_start.elapsed();

    println!("Queried {} tasks in {:?}", tasks.len(), query_duration);
    assert!(!tasks.is_empty());
    assert!(query_duration.as_millis() < 2000); // Should complete within 2 seconds

    Ok(())
}

/// Test repository error handling
#[tokio::test]
async fn test_repository_error_handling() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());
    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());

    // Test get non-existent task
    let non_existent_task = task_repo.get_by_id(99999).await?;
    assert!(non_existent_task.is_none());

    // Test get non-existent task run
    let non_existent_task_run = task_run_repo.get_by_id(99999).await?;
    assert!(non_existent_task_run.is_none());

    // Test create task run with invalid task_id
    let invalid_task_run = TaskRun {
        id: 0,
        task_id: 99999, // Non-existent task ID
        status: TaskRunStatus::Pending,
        worker_id: None,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: Utc::now(),
        started_at: None,
        completed_at: None,
        result: None,
        error_message: None,
        created_at: Utc::now(),
    };

    let result = task_run_repo.create(&invalid_task_run).await;
    assert!(result.is_err()); // Should fail due to foreign key constraint

    Ok(())
}
