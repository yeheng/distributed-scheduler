use chrono::{Duration, Utc};
use futures;
use scheduler_domain::entities::*;
use scheduler_domain::repositories::*;
use scheduler_domain::traits::MessageQueue;
use scheduler_domain::traits::MockMessageQueue;
use scheduler_infrastructure::database::postgres::*;
use scheduler_observability::MetricsCollector;
use scheduler_testing_utils::{TaskBuilder, WorkerInfoBuilder};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;
use tokio::time::sleep;

pub struct ContainerPool {
    containers: HashMap<String, testcontainers::ContainerAsync<Postgres>>,
    pools: HashMap<String, PgPool>,
}

impl ContainerPool {
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
            pools: HashMap::new(),
        }
    }

    pub async fn get_or_create_pool(
        &mut self,
        pool_id: &str,
    ) -> Result<PgPool, Box<dyn std::error::Error>> {
        if let Some(pool) = self.pools.get(pool_id) {
            return Ok(pool.clone());
        }

        let postgres_image = Postgres::default()
            .with_db_name(&format!("scheduler_e2e_test_{}", pool_id))
            .with_user("test_user")
            .with_password("test_password")
            .with_tag("16-alpine");

        let container = postgres_image.start().await?;
        let connection_string = format!(
            "postgresql://test_user:test_password@127.0.0.1:{}/scheduler_e2e_test_{}",
            container.get_host_port_ipv4(5432).await?,
            pool_id
        );

        let pool = PgPool::connect(&connection_string).await?;
        sqlx::migrate!("../../migrations").run(&pool).await?;

        self.containers.insert(pool_id.to_string(), container);
        self.pools.insert(pool_id.to_string(), pool.clone());

        Ok(pool)
    }

    pub async fn cleanup(&mut self) {
        for (_, container) in self.containers.drain() {
            let _ = container.stop().await;
        }
        self.pools.clear();
    }
}

pub struct E2ETestSetup {
    pool_id: String,
    pub pool: PgPool,
    pub message_queue: Arc<MockMessageQueue>,
    pub task_repo: Arc<PostgresTaskRepository>,
    pub task_run_repo: Arc<PostgresTaskRunRepository>,
    pub worker_repo: Arc<PostgresWorkerRepository>,
    pub metrics: Arc<MetricsCollector>,
    performance_metrics: HashMap<String, std::time::Duration>,
}
static mut CONTAINER_POOL: Option<ContainerPool> = None;
static INIT: std::sync::Once = std::sync::Once::new();

impl E2ETestSetup {
    pub async fn new() -> Self {
        Self::with_pool_id("default").await
    }

    pub async fn with_pool_id(pool_id: &str) -> Self {
        INIT.call_once(|| unsafe {
            CONTAINER_POOL = Some(ContainerPool::new());
        });

        let pool = unsafe {
            if let Some(ref mut pool) = CONTAINER_POOL {
                pool.get_or_create_pool(pool_id).await.unwrap()
            } else {
                panic!("Container pool not initialized");
            }
        };
        let message_queue = Arc::new(MockMessageQueue::new());
        let task_repo = Arc::new(PostgresTaskRepository::new(pool.clone()));
        let task_run_repo = Arc::new(PostgresTaskRunRepository::new(pool.clone()));
        let worker_repo = Arc::new(PostgresWorkerRepository::new(pool.clone()));
        let metrics = Arc::new(MetricsCollector::new().unwrap());

        Self {
            pool_id: pool_id.to_string(),
            pool,
            message_queue,
            task_repo,
            task_run_repo,
            worker_repo,
            metrics,
            performance_metrics: HashMap::new(),
        }
    }
    pub fn record_performance(&mut self, operation: &str, duration: std::time::Duration) {
        self.performance_metrics
            .insert(operation.to_string(), duration);
    }
    pub fn get_performance_report(&self) -> String {
        let mut report = String::new();
        report.push_str("Performance Metrics:\n");
        for (operation, duration) in &self.performance_metrics {
            report.push_str(&format!("  {}: {:?}\n", operation, duration));
        }
        report
    }
    pub async fn create_test_worker(&self, worker_id: &str, task_types: Vec<String>) -> WorkerInfo {
        let worker = WorkerInfoBuilder::new()
            .with_id(worker_id)
            .with_supported_task_types(task_types.iter().map(|s| s.as_str()).collect())
            .build();

        self.worker_repo.register(&worker).await.unwrap();
        worker
    }
    pub async fn create_test_task(
        &self,
        name: &str,
        task_type: &str,
        dependencies: Vec<i64>,
    ) -> Task {
        let task = TaskBuilder::new()
            .with_name(name)
            .with_task_type(task_type)
            .with_dependencies(dependencies)
            .with_parameters(serde_json::json!({
                "command": format!("echo 'Executing {}'", name),
                "timeout": 30
            }))
            .build();

        self.task_repo.create(&task).await.unwrap()
    }
    pub async fn create_test_task_with_config(&self, builder: TaskBuilder) -> Task {
        let task = builder.build();
        self.task_repo.create(&task).await.unwrap()
    }
    async fn wait_for_task_run_status(
        &self,
        task_run_id: i64,
        expected_status: TaskRunStatus,
        timeout_secs: u64,
    ) -> bool {
        let start_time = Utc::now();
        let timeout_duration = Duration::seconds(timeout_secs as i64);

        while Utc::now() - start_time < timeout_duration {
            if let Ok(Some(task_run)) = self.task_run_repo.get_by_id(task_run_id).await {
                if task_run.status == expected_status {
                    return true;
                }
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        false
    }
    async fn simulate_task_execution(
        &self,
        task_run_id: i64,
        worker_id: &str,
        result: Option<String>,
        error: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        self.task_run_repo
            .update_status(task_run_id, TaskRunStatus::Running, Some(worker_id))
            .await?;
        sleep(tokio::time::Duration::from_millis(50)).await;
        if let Some(error_msg) = error {
            self.task_run_repo
                .update_result(task_run_id, None, Some(&error_msg))
                .await?;
            self.task_run_repo
                .update_status(task_run_id, TaskRunStatus::Failed, Some(worker_id))
                .await?;
        } else {
            let result_msg = result.unwrap_or_else(|| "Task completed successfully".to_string());
            self.task_run_repo
                .update_result(task_run_id, Some(&result_msg), None)
                .await?;
            self.task_run_repo
                .update_status(task_run_id, TaskRunStatus::Completed, Some(worker_id))
                .await?;
        }
        let _duration = start_time.elapsed();

        Ok(())
    }
    async fn cleanup_task_and_runs(&mut self, task_id: i64) {
        let start_time = Instant::now();
        if let Ok(task_runs) = self.task_run_repo.get_by_task_id(task_id).await {
            for task_run in task_runs {
                let _ = self.task_run_repo.delete(task_run.id).await;
            }
        }
        let _ = self.task_repo.delete(task_id).await;
        let duration = start_time.elapsed();
        self.record_performance("cleanup_task_and_runs", duration);
    }
    async fn cleanup_worker(&mut self, worker_id: &str) {
        let start_time = Instant::now();
        let _ = self.worker_repo.unregister(worker_id).await;
        let duration = start_time.elapsed();
        self.record_performance("cleanup_worker", duration);
    }
    pub async fn create_test_workers_parallel(
        &mut self,
        worker_configs: Vec<(String, Vec<String>)>,
    ) -> Vec<WorkerInfo> {
        let start_time = Instant::now();

        let worker_repo = &self.worker_repo;
        let workers = futures::future::try_join_all(worker_configs.into_iter().map(
            |(id, task_types)| async move {
                let worker = WorkerInfoBuilder::new()
                    .with_id(&id)
                    .with_supported_task_types(task_types.iter().map(|s| s.as_str()).collect())
                    .build();
                worker_repo.register(&worker).await?;
                Ok::<WorkerInfo, Box<dyn std::error::Error>>(worker)
            },
        ))
        .await
        .unwrap();

        let duration = start_time.elapsed();
        self.record_performance("create_workers_parallel", duration);

        workers
    }
    pub async fn create_test_tasks_parallel(
        &mut self,
        task_configs: Vec<(String, &str, Vec<i64>)>,
    ) -> Vec<Task> {
        let start_time = Instant::now();

        let task_repo = &self.task_repo;
        let tasks = futures::future::try_join_all(task_configs.into_iter().map(
            |(name, task_type, dependencies)| async move {
                let task = TaskBuilder::new()
                    .with_name(&name)
                    .with_task_type(task_type)
                    .with_dependencies(dependencies)
                    .with_parameters(serde_json::json!({
                        "command": format!("echo 'Executing {}'", name),
                        "timeout": 30
                    }))
                    .build();
                let created_task = task_repo.create(&task).await?;
                assert!(
                    created_task.id > 0,
                    "Task should have a valid database-generated ID"
                );
                Ok::<Task, Box<dyn std::error::Error>>(created_task)
            },
        ))
        .await
        .unwrap();

        let duration = start_time.elapsed();
        self.record_performance("create_tasks_parallel", duration);

        tasks
    }
    pub async fn verify_database_health(&mut self) -> bool {
        let start_time = Instant::now();

        let result = sqlx::query("SELECT 1").fetch_one(&self.pool).await.is_ok();

        let duration = start_time.elapsed();
        self.record_performance("database_health_check", duration);

        result
    }
    pub fn get_test_stats(&self) -> String {
        format!(
            "Test Setup Stats:\n  Pool ID: {}\n  Performance Metrics: {}\n  Database Health: {}",
            self.pool_id,
            self.performance_metrics.len(),
            if self
                .performance_metrics
                .contains_key("database_health_check")
            {
                "Verified"
            } else {
                "Not checked"
            }
        )
    }
}
pub async fn cleanup_container_pool() {
    unsafe {
        if let Some(ref mut pool) = CONTAINER_POOL {
            pool.cleanup().await;
        }
    }
}

#[tokio::test]
async fn test_complete_task_lifecycle_e2e() {
    let mut setup = E2ETestSetup::new().await;
    assert!(
        setup.verify_database_health().await,
        "Database should be healthy"
    );
    let worker = setup
        .create_test_worker("e2e-worker", vec!["shell".to_string()])
        .await;
    let task = setup
        .create_test_task("lifecycle_task", "shell", vec![])
        .await;
    let task_run = TaskRun::new(task.id, Utc::now());
    let created_run = setup.task_run_repo.create(&task_run).await.unwrap();
    assert_eq!(created_run.status, TaskRunStatus::Pending);
    assert!(created_run.worker_id.is_none());
    let task_execution_msg = TaskExecutionMessage {
        task_run_id: created_run.id,
        task_id: task.id,
        task_name: task.name.clone(),
        task_type: task.task_type.clone(),
        parameters: task.parameters.clone(),
        timeout_seconds: task.timeout_seconds,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    setup
        .message_queue
        .publish_message(
            "tasks",
            &scheduler_domain::entities::Message::task_execution(task_execution_msg),
        )
        .await
        .unwrap();
    let task_messages = setup.message_queue.consume_messages("tasks").await.unwrap();
    assert_eq!(task_messages.len(), 1);
    setup
        .simulate_task_execution(
            created_run.id,
            &worker.id,
            Some("Task completed successfully".to_string()),
            None,
        )
        .await
        .unwrap();
    assert!(
        setup
            .wait_for_task_run_status(created_run.id, TaskRunStatus::Completed, 5)
            .await,
        "Task should be completed"
    );

    let completed_run = setup
        .task_run_repo
        .get_by_id(created_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed_run.status, TaskRunStatus::Completed);
    assert!(completed_run.result.is_some());
    assert!(completed_run.completed_at.is_some());
    assert_eq!(completed_run.worker_id, Some(worker.id.clone()));
    println!("{}", setup.get_performance_report());
    setup.cleanup_task_and_runs(task.id).await;
    setup.cleanup_worker(&worker.id).await;
}

#[tokio::test]
async fn test_task_dependency_chain_e2e() {
    let mut setup = E2ETestSetup::with_pool_id("dependency_test").await;
    let worker = setup
        .create_test_worker("dependency-worker", vec!["shell".to_string()])
        .await;
    let task_configs = vec![
        ("task_a".to_string(), "shell", vec![]),
        ("task_b".to_string(), "shell", vec![]), // 依赖将在后面设置
        ("task_c".to_string(), "shell", vec![]), // 依赖将在后面设置
    ];

    let mut tasks = setup.create_test_tasks_parallel(task_configs).await;
    tasks[1].dependencies = vec![tasks[0].id];
    tasks[2].dependencies = vec![tasks[1].id];
    for task in &tasks {
        setup.task_repo.update(task).await.unwrap();
    }
    let created_runs: Vec<TaskRun> =
        futures::future::try_join_all(tasks.iter().map(|task| async {
            let run = TaskRun::new(task.id, Utc::now());
            setup.task_run_repo.create(&run).await
        }))
        .await
        .unwrap();
    let can_execute_a = setup
        .task_repo
        .check_dependencies(tasks[0].id)
        .await
        .unwrap();
    assert!(
        can_execute_a,
        "Task A should be executable (no dependencies)"
    );
    setup
        .simulate_task_execution(
            created_runs[0].id,
            &worker.id,
            Some("task_a completed".to_string()),
            None,
        )
        .await
        .unwrap();
    assert!(
        setup
            .wait_for_task_run_status(created_runs[0].id, TaskRunStatus::Completed, 5)
            .await,
        "Task A should complete first"
    );
    setup
        .simulate_task_execution(
            created_runs[1].id,
            &worker.id,
            Some("task_b completed".to_string()),
            None,
        )
        .await
        .unwrap();
    assert!(
        setup
            .wait_for_task_run_status(created_runs[1].id, TaskRunStatus::Completed, 5)
            .await,
        "Task B should complete after Task A"
    );
    setup
        .simulate_task_execution(
            created_runs[2].id,
            &worker.id,
            Some("task_c completed".to_string()),
            None,
        )
        .await
        .unwrap();
    for (i, run) in created_runs.iter().enumerate() {
        assert!(
            setup
                .wait_for_task_run_status(run.id, TaskRunStatus::Completed, 5)
                .await,
            "Task {} should be completed",
            i + 1
        );
    }
    let final_runs: Vec<Option<TaskRun>> = futures::future::try_join_all(
        created_runs
            .iter()
            .map(|run| setup.task_run_repo.get_by_id(run.id)),
    )
    .await
    .unwrap();

    let final_runs: Vec<TaskRun> = final_runs.into_iter().filter_map(|r| r).collect();

    for i in 0..final_runs.len() - 1 {
        assert!(
            final_runs[i].completed_at.unwrap() <= final_runs[i + 1].completed_at.unwrap(),
            "Task {} should complete before Task {}",
            i + 1,
            i + 2
        );
    }
    println!("{}", setup.get_performance_report());
    for task in tasks {
        setup.cleanup_task_and_runs(task.id).await;
    }
    setup.cleanup_worker(&worker.id).await;
}

#[tokio::test]
async fn test_task_retry_mechanism_e2e() {
    let mut setup = E2ETestSetup::with_pool_id("retry_test").await;
    let worker = setup
        .create_test_worker("retry-worker", vec!["shell".to_string()])
        .await;
    let task = TaskBuilder::new()
        .with_name("retry_task")
        .with_task_type("shell")
        .with_max_retries(2)
        .with_timeout(10)
        .build();

    let task = setup.task_repo.create(&task).await.unwrap();
    let task_run_1 = setup
        .task_run_repo
        .create(&TaskRun::new(task.id, Utc::now()))
        .await
        .unwrap();

    setup
        .simulate_task_execution(
            task_run_1.id,
            &worker.id,
            None,
            Some("Command failed with exit code 1".to_string()),
        )
        .await
        .unwrap();

    assert!(
        setup
            .wait_for_task_run_status(task_run_1.id, TaskRunStatus::Failed, 5)
            .await,
        "First attempt should fail"
    );
    let mut retry_run_1 = TaskRun::new(task.id, Utc::now());
    retry_run_1.retry_count = 1;
    let retry_1 = setup.task_run_repo.create(&retry_run_1).await.unwrap();

    setup
        .simulate_task_execution(
            retry_1.id,
            &worker.id,
            None,
            Some("Command failed again".to_string()),
        )
        .await
        .unwrap();

    assert!(
        setup
            .wait_for_task_run_status(retry_1.id, TaskRunStatus::Failed, 5)
            .await,
        "First retry should also fail"
    );
    let mut retry_run_2 = TaskRun::new(task.id, Utc::now());
    retry_run_2.retry_count = 2;
    let retry_2 = setup.task_run_repo.create(&retry_run_2).await.unwrap();

    setup
        .simulate_task_execution(
            retry_2.id,
            &worker.id,
            Some("Finally succeeded on retry".to_string()),
            None,
        )
        .await
        .unwrap();

    assert!(
        setup
            .wait_for_task_run_status(retry_2.id, TaskRunStatus::Completed, 5)
            .await,
        "Second retry should succeed"
    );
    let final_retry = setup
        .task_run_repo
        .get_by_id(retry_2.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_retry.status, TaskRunStatus::Completed);
    assert_eq!(final_retry.retry_count, 2);
    assert!(final_retry.result.unwrap().contains("succeeded"));
    let all_runs = setup.task_run_repo.get_by_task_id(task.id).await.unwrap();
    assert_eq!(all_runs.len(), 3); // 原始 + 2次重试
    println!("{}", setup.get_performance_report());
    setup.cleanup_task_and_runs(task.id).await;
    setup.cleanup_worker(&worker.id).await;
}

#[tokio::test]
async fn test_multiple_workers_load_balancing_e2e() {
    let mut setup = E2ETestSetup::with_pool_id("load_balance_test").await;
    let worker_configs = vec![
        ("load-worker-1".to_string(), vec!["shell".to_string()]),
        ("load-worker-2".to_string(), vec!["shell".to_string()]),
        ("load-worker-3".to_string(), vec!["shell".to_string()]),
    ];

    let workers = setup.create_test_workers_parallel(worker_configs).await;
    let task_configs: Vec<(String, &str, Vec<i64>)> = (0..6)
        .map(|i| (format!("load_test_task_{}", i), "shell", vec![]))
        .collect();

    let tasks = setup.create_test_tasks_parallel(task_configs).await;
    let created_runs: Vec<TaskRun> =
        futures::future::try_join_all(tasks.iter().map(|task| async {
            let run = TaskRun::new(task.id, Utc::now());
            setup.task_run_repo.create(&run).await
        }))
        .await
        .unwrap();
    let _execution_results =
        futures::future::try_join_all(created_runs.iter().enumerate().map(|(i, task_run)| {
            let worker = &workers[i % workers.len()]; // 简单轮询负载均衡
            let result_msg = format!("Task {} completed by {}", i, worker.id);
            setup.simulate_task_execution(task_run.id, &worker.id, Some(result_msg), None)
        }))
        .await
        .unwrap();
    for (i, task_run) in created_runs.iter().enumerate() {
        assert!(
            setup
                .wait_for_task_run_status(task_run.id, TaskRunStatus::Completed, 5)
                .await,
            "Task {} should be completed",
            i
        );

        let completed_run = setup
            .task_run_repo
            .get_by_id(task_run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(completed_run.status, TaskRunStatus::Completed);
    }
    let load_stats = setup.worker_repo.get_worker_load_stats().await.unwrap();
    assert_eq!(load_stats.len(), 3, "Should have load stats for 3 workers");
    println!("{}", setup.get_performance_report());
    for task in tasks {
        setup.cleanup_task_and_runs(task.id).await;
    }
    for worker in workers {
        setup.cleanup_worker(&worker.id).await;
    }
}

#[tokio::test]
async fn test_worker_failure_and_task_reassignment_e2e() {
    let mut setup = E2ETestSetup::with_pool_id("failover_test").await;
    let worker_configs = vec![
        ("primary-worker".to_string(), vec!["shell".to_string()]),
        ("backup-worker".to_string(), vec!["shell".to_string()]),
    ];

    let workers = setup.create_test_workers_parallel(worker_configs).await;
    let task = setup
        .create_test_task("failover_task", "shell", vec![])
        .await;
    let task_run = setup
        .task_run_repo
        .create(&TaskRun::new(task.id, Utc::now()))
        .await
        .unwrap();
    setup
        .task_run_repo
        .update_status(task_run.id, TaskRunStatus::Running, Some(&workers[0].id))
        .await
        .unwrap();
    setup
        .worker_repo
        .update_status(&workers[0].id, WorkerStatus::Down)
        .await
        .unwrap();
    let alive_workers = setup.worker_repo.get_alive_workers().await.unwrap();
    assert_eq!(alive_workers.len(), 1);
    assert_eq!(alive_workers[0].id, workers[1].id);
    setup
        .task_run_repo
        .update_status(task_run.id, TaskRunStatus::Pending, None)
        .await
        .unwrap();

    setup
        .simulate_task_execution(
            task_run.id,
            &workers[1].id,
            Some("Task completed after failover".to_string()),
            None,
        )
        .await
        .unwrap();
    assert!(
        setup
            .wait_for_task_run_status(task_run.id, TaskRunStatus::Completed, 5)
            .await,
        "Task should be completed after failover"
    );

    let completed_run = setup
        .task_run_repo
        .get_by_id(task_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed_run.status, TaskRunStatus::Completed);
    assert_eq!(completed_run.worker_id, Some(workers[1].id.clone()));
    assert!(completed_run.result.unwrap().contains("failover"));
    println!("{}", setup.get_performance_report());
    setup.cleanup_task_and_runs(task.id).await;
    for worker in workers {
        setup.cleanup_worker(&worker.id).await;
    }
}

#[tokio::test]
async fn test_task_timeout_handling_e2e() {
    let mut setup = E2ETestSetup::with_pool_id("timeout_test").await;
    let worker = setup
        .create_test_worker("timeout-worker", vec!["shell".to_string()])
        .await;
    let task = TaskBuilder::new()
        .with_name("timeout_task")
        .with_task_type("shell")
        .with_timeout(1) // 1秒超时
        .build();

    let task = setup.task_repo.create(&task).await.unwrap();
    let task_run = setup
        .task_run_repo
        .create(&TaskRun::new(task.id, Utc::now()))
        .await
        .unwrap();
    setup
        .task_run_repo
        .update_status(task_run.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();
    setup
        .task_run_repo
        .update_status(task_run.id, TaskRunStatus::Timeout, Some(&worker.id))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_result(task_run.id, None, Some("Task execution timed out"))
        .await
        .unwrap();
    assert!(
        setup
            .wait_for_task_run_status(task_run.id, TaskRunStatus::Timeout, 5)
            .await,
        "Task should timeout"
    );

    let timeout_run = setup
        .task_run_repo
        .get_by_id(task_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(timeout_run.status, TaskRunStatus::Timeout);
    assert!(timeout_run.error_message.is_some());
    assert!(timeout_run.error_message.unwrap().contains("timed out"));
    println!("{}", setup.get_performance_report());
    setup.cleanup_task_and_runs(task.id).await;
    setup.cleanup_worker(&worker.id).await;
}

#[tokio::test]
async fn test_performance_benchmark_e2e() {
    let mut setup = E2ETestSetup::with_pool_id("benchmark_test").await;
    let worker_configs: Vec<(String, Vec<String>)> = (0..5)
        .map(|i| (format!("benchmark-worker-{}", i), vec!["shell".to_string()]))
        .collect();

    let workers = setup.create_test_workers_parallel(worker_configs).await;
    let task_configs: Vec<(String, &str, Vec<i64>)> = (0..20)
        .map(|i| (format!("benchmark_task_{}", i), "shell", vec![]))
        .collect();

    let tasks = setup.create_test_tasks_parallel(task_configs).await;
    let start_time = Instant::now();
    let created_runs: Vec<TaskRun> =
        futures::future::try_join_all(tasks.iter().map(|task| async {
            let run = TaskRun::new(task.id, Utc::now());
            setup.task_run_repo.create(&run).await
        }))
        .await
        .unwrap();

    let creation_duration = start_time.elapsed();
    setup.record_performance("bulk_task_run_creation", creation_duration);
    let execution_start = Instant::now();
    let _execution_results =
        futures::future::try_join_all(created_runs.iter().enumerate().map(|(i, task_run)| {
            let worker = &workers[i % workers.len()];
            let result_msg = format!("Benchmark task {} completed", i);
            setup.simulate_task_execution(task_run.id, &worker.id, Some(result_msg), None)
        }))
        .await
        .unwrap();

    let execution_duration = execution_start.elapsed();
    setup.record_performance("parallel_task_execution", execution_duration);
    let verification_start = Instant::now();
    for (i, task_run) in created_runs.iter().enumerate() {
        assert!(
            setup
                .wait_for_task_run_status(task_run.id, TaskRunStatus::Completed, 10)
                .await,
            "Benchmark task {} should be completed",
            i
        );
    }
    let verification_duration = verification_start.elapsed();
    setup.record_performance("task_status_verification", verification_duration);
    assert!(
        creation_duration.as_millis() < 5000,
        "Bulk task run creation should complete within 5 seconds, took: {:?}",
        creation_duration
    );

    assert!(
        execution_duration.as_millis() < 3000,
        "Parallel task execution should complete within 3 seconds, took: {:?}",
        execution_duration
    );
    println!("=== Performance Benchmark Results ===");
    println!("{}", setup.get_performance_report());
    println!("Total tasks executed: {}", tasks.len());
    println!("Total workers used: {}", workers.len());
    println!(
        "Average time per task: {:?}",
        execution_duration / tasks.len() as u32
    );
    for task in tasks {
        setup.cleanup_task_and_runs(task.id).await;
    }
    for worker in workers {
        setup.cleanup_worker(&worker.id).await;
    }
}

#[tokio::test]
async fn test_concurrent_execution_e2e() {
    let mut setup = E2ETestSetup::with_pool_id("concurrent_test").await;
    let worker = setup
        .create_test_worker("concurrent-worker", vec!["shell".to_string()])
        .await;
    let task_configs: Vec<(String, &str, Vec<i64>)> = (0..10)
        .map(|i| (format!("concurrent_task_{}", i), "shell", vec![]))
        .collect();

    let tasks = setup.create_test_tasks_parallel(task_configs).await;
    let created_runs: Vec<TaskRun> =
        futures::future::try_join_all(tasks.iter().map(|task| async {
            let run = TaskRun::new(task.id, Utc::now());
            setup.task_run_repo.create(&run).await
        }))
        .await
        .unwrap();
    let concurrent_start = Instant::now();
    let _execution_results =
        futures::future::try_join_all(created_runs.iter().enumerate().map(|(i, task_run)| {
            let result_msg = format!("Concurrent task {} completed", i);
            setup.simulate_task_execution(task_run.id, &worker.id, Some(result_msg), None)
        }))
        .await
        .unwrap();

    let concurrent_duration = concurrent_start.elapsed();
    setup.record_performance("concurrent_task_execution", concurrent_duration);
    for (i, task_run) in created_runs.iter().enumerate() {
        assert!(
            setup
                .wait_for_task_run_status(task_run.id, TaskRunStatus::Completed, 5)
                .await,
            "Concurrent task {} should be completed",
            i
        );
    }
    assert!(
        concurrent_duration.as_millis() < 2000,
        "Concurrent execution should complete within 2 seconds, took: {:?}",
        concurrent_duration
    );
    println!("=== Concurrent Execution Results ===");
    println!("{}", setup.get_performance_report());
    println!("Concurrent tasks: {}", tasks.len());
    println!("Concurrent execution time: {:?}", concurrent_duration);
    for task in tasks {
        setup.cleanup_task_and_runs(task.id).await;
    }
    setup.cleanup_worker(&worker.id).await;
}
