use anyhow::Result;
use chrono::Utc;
use scheduler_domain::entities::{
    Task, TaskFilter, TaskRun, TaskRunStatus, TaskStatus, WorkerInfo, WorkerStatus,
};
use scheduler_domain::repositories::*;
use scheduler_infrastructure::database::postgres::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};

mod database_test_utils;
use database_test_utils::DatabaseTestContainer;

#[tokio::test]
async fn test_postgres_task_repository_crud() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let repo = PostgresTaskRepository::new(container.pool.clone());
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
    let retrieved_task = repo.get_by_id(created_task.id).await?;
    assert!(retrieved_task.is_some());
    let retrieved_task = retrieved_task.unwrap();
    assert_eq!(retrieved_task.id, created_task.id);
    assert_eq!(retrieved_task.name, created_task.name);
    let mut updated_task = retrieved_task.clone();
    updated_task.name = "updated_integration_test_task".to_string();
    updated_task.max_retries = 5;

    repo.update(&updated_task).await?;

    let retrieved_updated_task = repo.get_by_id(updated_task.id).await?;
    assert!(retrieved_updated_task.is_some());
    let retrieved_updated_task = retrieved_updated_task.unwrap();
    assert_eq!(retrieved_updated_task.name, "updated_integration_test_task");
    assert_eq!(retrieved_updated_task.max_retries, 5);
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
    let active_tasks = repo.get_active_tasks().await?;
    assert!(!active_tasks.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_postgres_task_run_repository_crud() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;
    let task_id = container
        .insert_test_task("test_task", "shell", "0 0 * * *")
        .await?;

    let repo = PostgresTaskRunRepository::new(container.pool.clone());
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
    repo.update_status(created_task_run.id, TaskRunStatus::Running, None)
        .await?;
    let updated_run = repo.get_by_id(created_task_run.id).await?;
    assert!(updated_run.is_some());
    assert_eq!(updated_run.unwrap().status, TaskRunStatus::Running);
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

#[tokio::test]
async fn test_postgres_worker_repository_crud() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let repo = PostgresWorkerRepository::new(container.pool.clone());
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
    let retrieved_worker = repo.get_by_id(&worker.id).await?;
    assert!(retrieved_worker.is_some());
    let retrieved_worker = retrieved_worker.unwrap();
    assert_eq!(retrieved_worker.id, worker.id);
    assert_eq!(retrieved_worker.hostname, worker.hostname);
    assert_eq!(
        retrieved_worker.supported_task_types,
        worker.supported_task_types
    );
    let new_heartbeat = Utc::now();
    repo.update_heartbeat(&worker.id, new_heartbeat, 2).await?;

    let updated_worker = repo.get_by_id(&worker.id).await?;
    assert!(updated_worker.is_some());
    let updated_worker = updated_worker.unwrap();
    assert!(updated_worker.last_heartbeat >= new_heartbeat - chrono::Duration::seconds(1));
    let active_workers = repo.get_alive_workers().await?;
    assert!(!active_workers.is_empty());
    assert!(active_workers.iter().any(|w| w.id == worker.id));
    repo.update_status(&worker.id, WorkerStatus::Down).await?;

    let down_worker = repo.get_by_id(&worker.id).await?;
    assert!(down_worker.is_some());
    assert_eq!(down_worker.unwrap().status, WorkerStatus::Down);

    Ok(())
}

#[tokio::test]
async fn test_repository_performance() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());

    let start = std::time::Instant::now();
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

#[tokio::test]
async fn test_repository_error_handling() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());
    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());
    let worker_repo = PostgresWorkerRepository::new(container.pool.clone());
    let non_existent_task = task_repo.get_by_id(99999).await?;
    assert!(non_existent_task.is_none());
    let non_existent_task_run = task_run_repo.get_by_id(99999).await?;
    assert!(non_existent_task_run.is_none());
    let non_existent_worker = worker_repo.get_by_id("non_existent_worker").await?;
    assert!(non_existent_worker.is_none());
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
    let non_existent_task_update = Task {
        id: 99999,
        name: "non_existent".to_string(),
        task_type: "shell".to_string(),
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let update_result = task_repo.update(&non_existent_task_update).await;
    assert!(update_result.is_err()); // Should fail because task doesn't exist
    let delete_result = task_repo.delete(99999).await;
    assert!(delete_result.is_err()); // Should fail because task doesn't exist
    let status_update_result = task_run_repo
        .update_status(99999, TaskRunStatus::Running, Some("worker1"))
        .await;
    assert!(status_update_result.is_err()); // Should fail because task run doesn't exist
    let worker_status_result = worker_repo
        .update_status("non_existent_worker", WorkerStatus::Down)
        .await;
    assert!(worker_status_result.is_err()); // Should fail because worker doesn't exist

    Ok(())
}

#[tokio::test]
async fn test_task_dependencies() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());
    let parent_task = Task {
        id: 0,
        name: "parent_task".to_string(),
        task_type: "shell".to_string(),
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'parent'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created_parent = task_repo.create(&parent_task).await?;
    let child_task = Task {
        id: 0,
        name: "child_task".to_string(),
        task_type: "shell".to_string(),
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'child'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![created_parent.id],
        shard_config: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created_child = task_repo.create(&child_task).await?;
    let dependencies = task_repo.get_dependencies(created_child.id).await?;
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].id, created_parent.id);
    let deps_satisfied = task_repo.check_dependencies(created_child.id).await?;
    assert!(!deps_satisfied); // No successful runs of parent task yet

    Ok(())
}

#[tokio::test]
async fn test_worker_load_stats() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let worker_repo = PostgresWorkerRepository::new(container.pool.clone());
    let worker = WorkerInfo {
        id: "load_test_worker".to_string(),
        hostname: "load-test-host".to_string(),
        ip_address: "192.168.1.101".to_string(),
        supported_task_types: vec!["shell".to_string()],
        max_concurrent_tasks: 10,
        current_task_count: 0,
        status: WorkerStatus::Alive,
        last_heartbeat: Utc::now(),
        registered_at: Utc::now(),
    };

    worker_repo.register(&worker).await?;
    let stats = worker_repo.get_worker_load_stats().await?;
    let worker_stats = stats.iter().find(|s| s.worker_id == worker.id);
    assert!(worker_stats.is_some());

    let worker_stats = worker_stats.unwrap();
    assert_eq!(worker_stats.max_concurrent_tasks, 10);
    assert_eq!(worker_stats.current_task_count, 0);
    assert_eq!(worker_stats.load_percentage, 0.0);
    worker_repo.unregister(&worker.id).await?;

    Ok(())
}

#[tokio::test]
async fn test_task_filtering_and_pagination() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());
    let tasks_data = vec![
        ("shell_task_1", "shell", TaskStatus::Active),
        ("shell_task_2", "shell", TaskStatus::Inactive),
        ("http_task_1", "http", TaskStatus::Active),
        ("http_task_2", "http", TaskStatus::Active),
    ];

    for (name, task_type, status) in tasks_data {
        let task = Task {
            id: 0,
            name: name.to_string(),
            task_type: task_type.to_string(),
            schedule: "0 0 * * *".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 300,
            max_retries: 3,
            status,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        task_repo.create(&task).await?;
    }
    let active_filter = TaskFilter {
        status: Some(TaskStatus::Active),
        task_type: None,
        name_pattern: None,
        limit: None,
        offset: None,
    };
    let active_tasks = task_repo.list(&active_filter).await?;
    assert!(active_tasks.len() >= 3); // At least 3 active tasks
    let shell_filter = TaskFilter {
        status: None,
        task_type: Some("shell".to_string()),
        name_pattern: None,
        limit: None,
        offset: None,
    };
    let shell_tasks = task_repo.list(&shell_filter).await?;
    assert!(shell_tasks.len() >= 2); // At least 2 shell tasks
    let name_filter = TaskFilter {
        status: None,
        task_type: None,
        name_pattern: Some("shell".to_string()),
        limit: None,
        offset: None,
    };
    let name_filtered_tasks = task_repo.list(&name_filter).await?;
    assert!(name_filtered_tasks.len() >= 2); // At least 2 tasks with "shell" in name
    let paginated_filter = TaskFilter {
        status: Some(TaskStatus::Active),
        task_type: None,
        name_pattern: None,
        limit: Some(2),
        offset: Some(0),
    };
    let paginated_tasks = task_repo.list(&paginated_filter).await?;
    assert!(paginated_tasks.len() <= 2); // Should return at most 2 tasks

    Ok(())
}

#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());
    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());
    let task = Task {
        id: 0,
        name: "concurrent_test_task".to_string(),
        task_type: "shell".to_string(),
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created_task = task_repo.create(&task).await?;
    let task_runs: Vec<TaskRun> = (0..10)
        .map(|i| TaskRun {
            id: 0,
            task_id: created_task.id,
            status: TaskRunStatus::Pending,
            worker_id: Some(format!("worker_{}", i)),
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: Utc::now(),
        })
        .collect();

    let mut futures = Vec::new();
    for task_run in &task_runs {
        futures.push(task_run_repo.create(task_run));
    }

    let results = futures::future::join_all(futures).await;
    let successful_creates = results.into_iter().filter(|r| r.is_ok()).count();
    assert!(successful_creates >= 8);

    Ok(())
}
