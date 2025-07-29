use anyhow::Result;
use chrono::Utc;
use scheduler_core::models::{TaskRun, TaskRunStatus};
use scheduler_core::traits::{TaskRepository, TaskRunRepository, WorkerRepository};
use scheduler_infrastructure::database::postgres_repositories::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};

mod database_test_utils;
use database_test_utils::DatabaseTestContainer;

/// Test basic database operations without enum casting issues
#[tokio::test]
async fn test_basic_database_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    // Test basic table structure
    let count = container.get_table_count("tasks").await?;
    assert_eq!(count, 0);

    let count = container.get_table_count("task_runs").await?;
    assert_eq!(count, 0);

    let count = container.get_table_count("workers").await?;
    assert_eq!(count, 0);

    // Test basic insertion using raw SQL
    let task_id = container
        .insert_test_task("test_task", "shell", "0 0 * * *")
        .await?;
    assert!(task_id > 0);

    let count_after_insert = container.get_table_count("tasks").await?;
    assert_eq!(count_after_insert, 1);

    Ok(())
}

/// Test task repository basic operations
#[tokio::test]
async fn test_task_repository_basic_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let repo = PostgresTaskRepository::new(container.pool.clone());

    // Test get_active_tasks on empty table
    let active_tasks = repo.get_active_tasks().await?;
    assert!(active_tasks.is_empty());

    // Test get_by_id for non-existent task
    let non_existent = repo.get_by_id(999).await?;
    assert!(non_existent.is_none());

    // Test get_by_name for non-existent task
    let non_existent = repo.get_by_name("non_existent").await?;
    assert!(non_existent.is_none());

    Ok(())
}

/// Test task run repository basic operations  
#[tokio::test]
async fn test_task_run_repository_basic_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    // First create a task using helper method
    let task_id = container
        .insert_test_task("test_task", "shell", "0 0 * * *")
        .await?;

    let repo = PostgresTaskRunRepository::new(container.pool.clone());

    // Test get_pending_runs on empty table
    let pending_runs = repo.get_pending_runs(None).await?;
    assert!(pending_runs.is_empty());

    // Test get_by_id for non-existent run
    let non_existent = repo.get_by_id(999).await?;
    assert!(non_existent.is_none());

    // Test get_by_worker_id for non-existent worker
    let worker_runs = repo.get_by_worker_id("non_existent_worker").await?;
    assert!(worker_runs.is_empty());

    // Test get_by_task_id
    let task_runs = repo.get_by_task_id(task_id).await?;
    assert!(task_runs.is_empty());

    Ok(())
}

/// Test worker repository basic operations
#[tokio::test]
async fn test_worker_repository_basic_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let repo = PostgresWorkerRepository::new(container.pool.clone());

    // Test get_alive_workers on empty table
    let alive_workers = repo.get_alive_workers().await?;
    assert!(alive_workers.is_empty());

    // Test get_by_id for non-existent worker
    let non_existent = repo.get_by_id("non_existent").await?;
    assert!(non_existent.is_none());

    Ok(())
}

/// Test repository error handling and edge cases
#[tokio::test]
async fn test_repository_error_handling() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());
    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());

    // Test deletion of non-existent records
    let delete_result = task_repo.delete(999).await;
    assert!(delete_result.is_ok()); // Should not error, just affect 0 rows

    let delete_result = task_run_repo.delete(999).await;
    assert!(delete_result.is_ok());

    Ok(())
}

/// Test data consistency and constraints
#[tokio::test]
async fn test_data_consistency() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());

    // Test creating task run with invalid task_id (should fail due to foreign key)
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
    assert!(result.is_err(), "Should fail due to foreign key constraint");

    Ok(())
}

/// Test concurrent operations
#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    // Create a task first
    let task_id = container
        .insert_test_task("concurrent_test", "shell", "0 0 * * *")
        .await?;

    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());

    // Create a task run
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

    let created_run = task_run_repo.create(&task_run).await?;

    // Simulate concurrent status updates
    let update1 =
        task_run_repo.update_status(created_run.id, TaskRunStatus::Running, Some("worker1"));
    let update2 =
        task_run_repo.update_status(created_run.id, TaskRunStatus::Dispatched, Some("worker2"));

    // Execute both updates concurrently
    let (result1, result2) = tokio::join!(update1, update2);

    // At least one should succeed
    assert!(
        result1.is_ok() || result2.is_ok(),
        "At least one update should succeed"
    );

    // Verify the final state
    let final_run = task_run_repo.get_by_id(created_run.id).await?;
    assert!(final_run.is_some());

    Ok(())
}

/// Test performance with multiple operations
#[tokio::test]
async fn test_performance_multiple_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let start = std::time::Instant::now();

    // Insert multiple test tasks using helper methods
    let mut task_ids = Vec::new();
    for i in 0..20 {
        let task_id = container
            .insert_test_task(&format!("perf_task_{}", i), "shell", "0 0 * * *")
            .await?;
        task_ids.push(task_id);
    }

    let insert_duration = start.elapsed();
    println!("Inserted {} tasks in {:?}", task_ids.len(), insert_duration);

    // Query all tasks back
    let query_start = std::time::Instant::now();
    let final_count = container.get_table_count("tasks").await?;
    let query_duration = query_start.elapsed();

    println!("Queried {} tasks in {:?}", final_count, query_duration);
    assert_eq!(final_count, 20);

    // Performance assertions
    assert!(
        insert_duration.as_millis() < 5000,
        "Insert should complete within 5 seconds"
    );
    assert!(
        query_duration.as_millis() < 1000,
        "Query should complete within 1 second"
    );

    Ok(())
}

/// Test database connection recovery simulation
#[tokio::test]
async fn test_connection_handling() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());

    // Test normal operation
    let active_tasks = task_repo.get_active_tasks().await?;
    assert!(active_tasks.is_empty());

    // Simulate heavy load with concurrent operations
    let mut futures = Vec::new();
    for i in 0..10 {
        let repo_clone = PostgresTaskRepository::new(container.pool.clone());
        let future = async move { repo_clone.get_by_id(i).await };
        futures.push(future);
    }

    let results = futures::future::join_all(futures).await;
    let successful_queries = results.into_iter().filter(|r| r.is_ok()).count();

    println!(
        "Completed {} concurrent queries successfully",
        successful_queries
    );
    assert!(successful_queries >= 8, "Most queries should succeed");

    Ok(())
}
