use anyhow::Result;
use chrono::Utc;
use scheduler_foundation::SchedulerError;
use scheduler_domain::entities::*;
use scheduler_domain::repositories::*;
use scheduler_infrastructure::database::postgres::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};

mod database_test_utils;
use database_test_utils::DatabaseTestContainer;

#[tokio::test]
async fn test_basic_database_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;
    let count = container.get_table_count("tasks").await?;
    assert_eq!(count, 0);

    let count = container.get_table_count("task_runs").await?;
    assert_eq!(count, 0);

    let count = container.get_table_count("workers").await?;
    assert_eq!(count, 0);
    let task_id = container
        .insert_test_task("test_task", "shell", "0 0 * * *")
        .await?;
    assert!(task_id > 0);

    let count_after_insert = container.get_table_count("tasks").await?;
    assert_eq!(count_after_insert, 1);

    Ok(())
}

#[tokio::test]
async fn test_task_repository_basic_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let repo = PostgresTaskRepository::new(container.pool.clone());
    let active_tasks = repo.get_active_tasks().await?;
    assert!(active_tasks.is_empty());
    let non_existent = repo.get_by_id(999).await?;
    assert!(non_existent.is_none());
    let non_existent = repo.get_by_name("non_existent").await?;
    assert!(non_existent.is_none());

    Ok(())
}

#[tokio::test]
async fn test_task_run_repository_basic_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;
    let task_id = container
        .insert_test_task("test_task", "shell", "0 0 * * *")
        .await?;

    let repo = PostgresTaskRunRepository::new(container.pool.clone());
    let pending_runs = repo.get_pending_runs(None).await?;
    assert!(pending_runs.is_empty());
    let non_existent = repo.get_by_id(999).await?;
    assert!(non_existent.is_none());
    let worker_runs = repo.get_by_worker_id("non_existent_worker").await?;
    assert!(worker_runs.is_empty());
    let task_runs = repo.get_by_task_id(task_id).await?;
    assert!(task_runs.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_worker_repository_basic_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let repo = PostgresWorkerRepository::new(container.pool.clone());
    let alive_workers = repo.get_alive_workers().await?;
    assert!(alive_workers.is_empty());
    let non_existent = repo.get_by_id("non_existent").await?;
    assert!(non_existent.is_none());

    Ok(())
}

#[tokio::test]
async fn test_repository_error_handling() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());
    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());
    let delete_result = task_repo.delete(999).await;
    assert!(delete_result.is_err()); // Should error for non-existent task
    assert!(matches!(
        delete_result,
        Err(SchedulerError::TaskNotFound { id: 999 })
    ));

    let delete_result = task_run_repo.delete(999).await;
    assert!(delete_result.is_err()); // Should error for non-existent task run
    assert!(matches!(
        delete_result,
        Err(SchedulerError::TaskRunNotFound { id: 999 })
    ));

    Ok(())
}

#[tokio::test]
async fn test_data_consistency() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());
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

#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;
    let task_id = container
        .insert_test_task("concurrent_test", "shell", "0 0 * * *")
        .await?;

    let task_run_repo = PostgresTaskRunRepository::new(container.pool.clone());
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
    let update1 =
        task_run_repo.update_status(created_run.id, TaskRunStatus::Running, Some("worker1"));
    let update2 =
        task_run_repo.update_status(created_run.id, TaskRunStatus::Dispatched, Some("worker2"));
    let (result1, result2) = tokio::join!(update1, update2);
    assert!(
        result1.is_ok() || result2.is_ok(),
        "At least one update should succeed"
    );
    let final_run = task_run_repo.get_by_id(created_run.id).await?;
    assert!(final_run.is_some());

    Ok(())
}

#[tokio::test]
async fn test_performance_multiple_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let start = std::time::Instant::now();
    let mut task_ids = Vec::new();
    for i in 0..20 {
        let task_id = container
            .insert_test_task(&format!("perf_task_{}", i), "shell", "0 0 * * *")
            .await?;
        task_ids.push(task_id);
    }

    let insert_duration = start.elapsed();
    println!("Inserted {} tasks in {:?}", task_ids.len(), insert_duration);
    let query_start = std::time::Instant::now();
    let final_count = container.get_table_count("tasks").await?;
    let query_duration = query_start.elapsed();

    println!("Queried {} tasks in {:?}", final_count, query_duration);
    assert_eq!(final_count, 20);
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

#[tokio::test]
async fn test_connection_handling() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;
    container.run_migrations().await?;

    let task_repo = PostgresTaskRepository::new(container.pool.clone());
    let active_tasks = task_repo.get_active_tasks().await?;
    assert!(active_tasks.is_empty());
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
