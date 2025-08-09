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
use std::collections::HashMap;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

struct E2ETestSetup {
    _postgres_container: testcontainers::ContainerAsync<Postgres>,
    pub message_queue: MockMessageQueue,
    pub task_repo: PostgresTaskRepository,
    pub task_run_repo: PostgresTaskRunRepository,
    pub worker_repo: PostgresWorkerRepository,
}

impl E2ETestSetup {
    async fn new() -> Self {
        let postgres_image = Postgres::default()
            .with_db_name("scheduler_e2e_test")
            .with_user("test_user")
            .with_password("test_password")
            .with_tag("16-alpine");

        let postgres_container = postgres_image.start().await.unwrap();
        let db_connection_string = format!(
            "postgresql://test_user:test_password@127.0.0.1:{}/scheduler_e2e_test",
            postgres_container.get_host_port_ipv4(5432).await.unwrap()
        );

        let pool = PgPool::connect(&db_connection_string).await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        let message_queue = MockMessageQueue::new();
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
    async fn create_test_worker(&self, worker_id: &str, task_types: Vec<String>) -> WorkerInfo {
        let worker = WorkerInfo {
            id: worker_id.to_string(),
            hostname: format!("{}-host", worker_id),
            ip_address: "127.0.0.1".to_string(),
            supported_task_types: task_types,
            max_concurrent_tasks: 5,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
        };

        self.worker_repo.register(&worker).await.unwrap();
        worker
    }
    async fn create_test_task(&self, name: &str, task_type: &str, dependencies: Vec<i64>) -> Task {
        let mut task = Task::new(
            name.to_string(),
            task_type.to_string(),
            "0 0 * * *".to_string(),
            serde_json::json!({"command": format!("echo '{}'", name)}),
        );
        task.dependencies = dependencies;
        task.max_retries = 3;
        task.timeout_seconds = 300;

        self.task_repo.create(&task).await.unwrap()
    }
}

#[tokio::test]
async fn test_complete_task_lifecycle() {
    let setup = E2ETestSetup::new().await;
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
    assert!(created_run.started_at.is_none());
    assert!(created_run.completed_at.is_none());
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

    let message = Message::task_execution(task_execution_msg);
    setup
        .message_queue
        .publish_message("tasks", &message)
        .await
        .unwrap();
    let task_messages = setup.message_queue.consume_messages("tasks").await.unwrap();
    assert_eq!(task_messages.len(), 1);
    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();
    let running_task_run = setup
        .task_run_repo
        .get_by_id(created_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(running_task_run.status, TaskRunStatus::Running);
    assert_eq!(running_task_run.worker_id, Some(worker.id.clone()));
    assert!(running_task_run.started_at.is_some());
    let execution_result = "lifecycle_task completed successfully";
    setup
        .task_run_repo
        .update_result(created_run.id, Some(execution_result), None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Completed, Some(&worker.id))
        .await
        .unwrap();
    let completed_task_run = setup
        .task_run_repo
        .get_by_id(created_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed_task_run.status, TaskRunStatus::Completed);
    assert_eq!(
        completed_task_run.result,
        Some(execution_result.to_string())
    );
    assert!(completed_task_run.completed_at.is_some());
    let status_update_msg = StatusUpdateMessage {
        task_run_id: created_run.id,
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
    let status_messages = setup
        .message_queue
        .consume_messages("status_updates")
        .await
        .unwrap();
    assert_eq!(status_messages.len(), 1);

    if let scheduler_core::models::MessageType::StatusUpdate(ref msg) =
        status_messages[0].message_type
    {
        assert_eq!(msg.task_run_id, created_run.id);
        assert_eq!(msg.status, TaskRunStatus::Completed);
        assert_eq!(msg.worker_id, worker.id);
    } else {
        panic!("Expected StatusUpdate message");
    }
    setup.task_run_repo.delete(created_run.id).await.unwrap();
    setup.task_repo.delete(task.id).await.unwrap();
    setup.worker_repo.unregister(&worker.id).await.unwrap();
}

#[tokio::test]
async fn test_task_dependency_chain() {
    let setup = E2ETestSetup::new().await;
    let worker = setup
        .create_test_worker("dependency-worker", vec!["shell".to_string()])
        .await;
    let task_a = setup.create_test_task("task_a", "shell", vec![]).await;
    let task_b = setup
        .create_test_task("task_b", "shell", vec![task_a.id])
        .await;
    let task_c = setup
        .create_test_task("task_c", "shell", vec![task_b.id])
        .await;
    let run_a = setup
        .task_run_repo
        .create(&TaskRun::new(task_a.id, Utc::now()))
        .await
        .unwrap();
    let run_b = setup
        .task_run_repo
        .create(&TaskRun::new(task_b.id, Utc::now()))
        .await
        .unwrap();
    let run_c = setup
        .task_run_repo
        .create(&TaskRun::new(task_c.id, Utc::now()))
        .await
        .unwrap();
    let can_execute_a = setup.task_repo.check_dependencies(task_a.id).await.unwrap();
    assert!(can_execute_a);
    let _can_execute_b = setup.task_repo.check_dependencies(task_b.id).await.unwrap();
    let _can_execute_c = setup.task_repo.check_dependencies(task_c.id).await.unwrap();
    setup
        .task_run_repo
        .update_status(run_a.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_result(run_a.id, Some("task_a completed"), None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(run_a.id, TaskRunStatus::Completed, Some(&worker.id))
        .await
        .unwrap();
    let _can_execute_b_after_a = setup.task_repo.check_dependencies(task_b.id).await.unwrap();
    setup
        .task_run_repo
        .update_status(run_b.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_result(run_b.id, Some("task_b completed"), None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(run_b.id, TaskRunStatus::Completed, Some(&worker.id))
        .await
        .unwrap();
    setup
        .task_run_repo
        .update_status(run_c.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_result(run_c.id, Some("task_c completed"), None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(run_c.id, TaskRunStatus::Completed, Some(&worker.id))
        .await
        .unwrap();
    let final_run_a = setup
        .task_run_repo
        .get_by_id(run_a.id)
        .await
        .unwrap()
        .unwrap();
    let final_run_b = setup
        .task_run_repo
        .get_by_id(run_b.id)
        .await
        .unwrap()
        .unwrap();
    let final_run_c = setup
        .task_run_repo
        .get_by_id(run_c.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(final_run_a.status, TaskRunStatus::Completed);
    assert_eq!(final_run_b.status, TaskRunStatus::Completed);
    assert_eq!(final_run_c.status, TaskRunStatus::Completed);
    assert!(final_run_a.completed_at.unwrap() <= final_run_b.started_at.unwrap());
    assert!(final_run_b.completed_at.unwrap() <= final_run_c.started_at.unwrap());
    setup.task_run_repo.delete(run_a.id).await.unwrap();
    setup.task_run_repo.delete(run_b.id).await.unwrap();
    setup.task_run_repo.delete(run_c.id).await.unwrap();
    setup.task_repo.delete(task_a.id).await.unwrap();
    setup.task_repo.delete(task_b.id).await.unwrap();
    setup.task_repo.delete(task_c.id).await.unwrap();
    setup.worker_repo.unregister(&worker.id).await.unwrap();
}

#[tokio::test]
async fn test_task_retry_mechanism() {
    let setup = E2ETestSetup::new().await;
    let worker = setup
        .create_test_worker("retry-worker", vec!["shell".to_string()])
        .await;
    let mut task = setup.create_test_task("retry_task", "shell", vec![]).await;
    task.max_retries = 2; // 设置最大重试次数为2
    setup.task_repo.update(&task).await.unwrap();
    let task_run = TaskRun::new(task.id, Utc::now());
    let created_run = setup.task_run_repo.create(&task_run).await.unwrap();
    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_result(
            created_run.id,
            None,
            Some("Command failed with exit code 1"),
        )
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Failed, Some(&worker.id))
        .await
        .unwrap();
    let failed_run = setup
        .task_run_repo
        .get_by_id(created_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed_run.status, TaskRunStatus::Failed);
    assert!(failed_run.error_message.is_some());
    let mut retry_run_1 = TaskRun::new(task.id, Utc::now());
    retry_run_1.retry_count = 1;
    let retry_1 = setup.task_run_repo.create(&retry_run_1).await.unwrap();
    setup
        .task_run_repo
        .update_status(retry_1.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_result(retry_1.id, None, Some("Command failed again"))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(retry_1.id, TaskRunStatus::Failed, Some(&worker.id))
        .await
        .unwrap();
    let mut retry_run_2 = TaskRun::new(task.id, Utc::now());
    retry_run_2.retry_count = 2;
    let retry_2 = setup.task_run_repo.create(&retry_run_2).await.unwrap();
    setup
        .task_run_repo
        .update_status(retry_2.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_result(retry_2.id, Some("Finally succeeded on retry"), None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(retry_2.id, TaskRunStatus::Completed, Some(&worker.id))
        .await
        .unwrap();
    let final_retry = setup
        .task_run_repo
        .get_by_id(retry_2.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_retry.status, TaskRunStatus::Completed);
    assert_eq!(final_retry.retry_count, 2);
    assert_eq!(
        final_retry.result,
        Some("Finally succeeded on retry".to_string())
    );
    let all_runs = setup.task_run_repo.get_by_task_id(task.id).await.unwrap();
    assert_eq!(all_runs.len(), 3); // 原始执行 + 2次重试

    let mut retry_counts: Vec<i32> = all_runs.iter().map(|r| r.retry_count).collect();
    retry_counts.sort();
    assert_eq!(retry_counts, vec![0, 1, 2]);
    setup.task_run_repo.delete(created_run.id).await.unwrap();
    setup.task_run_repo.delete(retry_1.id).await.unwrap();
    setup.task_run_repo.delete(retry_2.id).await.unwrap();
    setup.task_repo.delete(task.id).await.unwrap();
    setup.worker_repo.unregister(&worker.id).await.unwrap();
}

#[tokio::test]
async fn test_multiple_worker_load_balancing() {
    let setup = E2ETestSetup::new().await;
    let worker1 = setup
        .create_test_worker("worker-1", vec!["shell".to_string()])
        .await;
    let worker2 = setup
        .create_test_worker("worker-2", vec!["shell".to_string()])
        .await;
    let worker3 = setup
        .create_test_worker("worker-3", vec!["shell".to_string()])
        .await;
    let mut tasks = Vec::new();
    let mut task_runs = Vec::new();

    for i in 0..6 {
        let task = setup
            .create_test_task(&format!("load_balance_task_{}", i), "shell", vec![])
            .await;
        tasks.push(task.clone());

        let task_run = setup
            .task_run_repo
            .create(&TaskRun::new(task.id, Utc::now()))
            .await
            .unwrap();
        task_runs.push(task_run);
    }
    let workers = vec![&worker1, &worker2, &worker3];
    let mut worker_task_counts = HashMap::new();

    for (i, task_run) in task_runs.iter().enumerate() {
        let worker = workers[i % workers.len()]; // 简单的轮询分配
        let count = worker_task_counts.entry(worker.id.clone()).or_insert(0);
        *count += 1;
        setup
            .task_run_repo
            .update_status(task_run.id, TaskRunStatus::Running, Some(&worker.id))
            .await
            .unwrap();
        setup
            .worker_repo
            .update_heartbeat(&worker.id, Utc::now(), *count)
            .await
            .unwrap();
    }
    let load_stats = setup.worker_repo.get_worker_load_stats().await.unwrap();
    assert_eq!(load_stats.len(), 3);

    for stats in &load_stats {
        let expected_count = worker_task_counts.get(&stats.worker_id).unwrap_or(&0);
        assert_eq!(stats.current_task_count, *expected_count);
        let expected_percentage =
            (*expected_count as f64 / stats.max_concurrent_tasks as f64) * 100.0;
        assert_eq!(stats.load_percentage, expected_percentage);
    }
    for (i, task_run) in task_runs.iter().enumerate() {
        let worker = workers[i % workers.len()];

        setup
            .task_run_repo
            .update_result(task_run.id, Some(&format!("Task {} completed", i)), None)
            .await
            .unwrap();

        setup
            .task_run_repo
            .update_status(task_run.id, TaskRunStatus::Completed, Some(&worker.id))
            .await
            .unwrap();
        let count = worker_task_counts.get_mut(&worker.id).unwrap();
        *count -= 1;

        setup
            .worker_repo
            .update_heartbeat(&worker.id, Utc::now(), *count)
            .await
            .unwrap();
    }
    for task_run in &task_runs {
        let completed_run = setup
            .task_run_repo
            .get_by_id(task_run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(completed_run.status, TaskRunStatus::Completed);
    }
    let final_load_stats = setup.worker_repo.get_worker_load_stats().await.unwrap();
    for stats in &final_load_stats {
        assert_eq!(stats.current_task_count, 0);
        assert_eq!(stats.load_percentage, 0.0);
    }
    for task_run in task_runs {
        setup.task_run_repo.delete(task_run.id).await.unwrap();
    }
    for task in tasks {
        setup.task_repo.delete(task.id).await.unwrap();
    }
    setup.worker_repo.unregister(&worker1.id).await.unwrap();
    setup.worker_repo.unregister(&worker2.id).await.unwrap();
    setup.worker_repo.unregister(&worker3.id).await.unwrap();
}

#[tokio::test]
async fn test_task_timeout_handling() {
    let setup = E2ETestSetup::new().await;
    let worker = setup
        .create_test_worker("timeout-worker", vec!["shell".to_string()])
        .await;
    let mut task = setup
        .create_test_task("timeout_task", "shell", vec![])
        .await;
    task.timeout_seconds = 1; // 1秒超时
    setup.task_repo.update(&task).await.unwrap();
    let task_run = TaskRun::new(task.id, Utc::now());
    let created_run = setup.task_run_repo.create(&task_run).await.unwrap();
    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Running, Some(&worker.id))
        .await
        .unwrap();
    let _timeout_runs = setup.task_run_repo.get_timeout_runs(1).await.unwrap();
    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Timeout, Some(&worker.id))
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_result(created_run.id, None, Some("Task execution timed out"))
        .await
        .unwrap();
    let timeout_run = setup
        .task_run_repo
        .get_by_id(created_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(timeout_run.status, TaskRunStatus::Timeout);
    assert!(timeout_run.error_message.is_some());
    assert!(timeout_run.error_message.unwrap().contains("timed out"));
    setup.task_run_repo.delete(created_run.id).await.unwrap();
    setup.task_repo.delete(task.id).await.unwrap();
    setup.worker_repo.unregister(&worker.id).await.unwrap();
}

#[tokio::test]
async fn test_worker_failure_and_task_reassignment() {
    let setup = E2ETestSetup::new().await;
    let worker1 = setup
        .create_test_worker("primary-worker", vec!["shell".to_string()])
        .await;
    let worker2 = setup
        .create_test_worker("backup-worker", vec!["shell".to_string()])
        .await;
    let task = setup
        .create_test_task("failover_task", "shell", vec![])
        .await;
    let task_run = TaskRun::new(task.id, Utc::now());
    let created_run = setup.task_run_repo.create(&task_run).await.unwrap();

    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Running, Some(&worker1.id))
        .await
        .unwrap();
    setup
        .worker_repo
        .update_status(&worker1.id, WorkerStatus::Down)
        .await
        .unwrap();
    let failed_worker = setup
        .worker_repo
        .get_by_id(&worker1.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed_worker.status, WorkerStatus::Down);
    let alive_workers = setup.worker_repo.get_alive_workers().await.unwrap();
    assert_eq!(alive_workers.len(), 1);
    assert_eq!(alive_workers[0].id, worker2.id);
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
    setup
        .task_run_repo
        .update_result(created_run.id, Some("Task completed after failover"), None)
        .await
        .unwrap();

    setup
        .task_run_repo
        .update_status(created_run.id, TaskRunStatus::Completed, Some(&worker2.id))
        .await
        .unwrap();
    let completed_run = setup
        .task_run_repo
        .get_by_id(created_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed_run.status, TaskRunStatus::Completed);
    assert_eq!(completed_run.worker_id, Some(worker2.id.clone()));
    assert_eq!(
        completed_run.result,
        Some("Task completed after failover".to_string())
    );
    setup.task_run_repo.delete(created_run.id).await.unwrap();
    setup.task_repo.delete(task.id).await.unwrap();
    setup.worker_repo.unregister(&worker1.id).await.unwrap();
    setup.worker_repo.unregister(&worker2.id).await.unwrap();
}

#[tokio::test]
async fn test_message_queue_integration() {
    let setup = E2ETestSetup::new().await;
    let worker = setup
        .create_test_worker("mq-worker", vec!["shell".to_string(), "http".to_string()])
        .await;
    let shell_task = setup.create_test_task("shell_task", "shell", vec![]).await;
    let http_task = setup.create_test_task("http_task", "http", vec![]).await;
    let shell_run = setup
        .task_run_repo
        .create(&TaskRun::new(shell_task.id, Utc::now()))
        .await
        .unwrap();
    let http_run = setup
        .task_run_repo
        .create(&TaskRun::new(http_task.id, Utc::now()))
        .await
        .unwrap();
    let shell_msg = TaskExecutionMessage {
        task_run_id: shell_run.id,
        task_id: shell_task.id,
        task_name: shell_task.name.clone(),
        task_type: shell_task.task_type.clone(),
        parameters: shell_task.parameters.clone(),
        timeout_seconds: shell_task.timeout_seconds,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let http_msg = TaskExecutionMessage {
        task_run_id: http_run.id,
        task_id: http_task.id,
        task_name: http_task.name.clone(),
        task_type: http_task.task_type.clone(),
        parameters: http_task.parameters.clone(),
        timeout_seconds: http_task.timeout_seconds,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };

    let shell_message = Message::task_execution(shell_msg);
    let http_message = Message::task_execution(http_msg);
    setup
        .message_queue
        .publish_message("shell_tasks", &shell_message)
        .await
        .unwrap();
    setup
        .message_queue
        .publish_message("http_tasks", &http_message)
        .await
        .unwrap();
    assert_eq!(shell_message.routing_key(), "task.execution.shell");
    assert_eq!(http_message.routing_key(), "task.execution.http");
    let shell_messages = setup
        .message_queue
        .consume_messages("shell_tasks")
        .await
        .unwrap();
    let http_messages = setup
        .message_queue
        .consume_messages("http_tasks")
        .await
        .unwrap();

    assert_eq!(shell_messages.len(), 1);
    assert_eq!(http_messages.len(), 1);
    if let scheduler_core::models::MessageType::TaskExecution(ref msg) =
        shell_messages[0].message_type
    {
        assert_eq!(msg.task_type, "shell");
        assert_eq!(msg.task_run_id, shell_run.id);
    } else {
        panic!("Expected TaskExecution message");
    }

    if let scheduler_core::models::MessageType::TaskExecution(ref msg) =
        http_messages[0].message_type
    {
        assert_eq!(msg.task_type, "http");
        assert_eq!(msg.task_run_id, http_run.id);
    } else {
        panic!("Expected TaskExecution message");
    }
    for (task_run, task_type) in [(shell_run.id, "shell"), (http_run.id, "http")] {
        setup
            .task_run_repo
            .update_status(task_run, TaskRunStatus::Running, Some(&worker.id))
            .await
            .unwrap();

        setup
            .task_run_repo
            .update_result(
                task_run,
                Some(&format!("{} task completed", task_type)),
                None,
            )
            .await
            .unwrap();

        setup
            .task_run_repo
            .update_status(task_run, TaskRunStatus::Completed, Some(&worker.id))
            .await
            .unwrap();
        let status_msg = StatusUpdateMessage {
            task_run_id: task_run,
            status: TaskRunStatus::Completed,
            worker_id: worker.id.clone(),
            result: None,
            error_message: None,
            timestamp: Utc::now(),
        };

        let status_message = Message::status_update(status_msg);
        setup
            .message_queue
            .publish_message("status_updates", &status_message)
            .await
            .unwrap();
    }
    let status_messages = setup
        .message_queue
        .consume_messages("status_updates")
        .await
        .unwrap();
    assert_eq!(status_messages.len(), 2);

    for status_msg in &status_messages {
        if let scheduler_core::models::MessageType::StatusUpdate(ref msg) = status_msg.message_type
        {
            assert_eq!(msg.status, TaskRunStatus::Completed);
            assert_eq!(msg.worker_id, worker.id);
        } else {
            panic!("Expected StatusUpdate message");
        }
    }
    setup.task_run_repo.delete(shell_run.id).await.unwrap();
    setup.task_run_repo.delete(http_run.id).await.unwrap();
    setup.task_repo.delete(shell_task.id).await.unwrap();
    setup.task_repo.delete(http_task.id).await.unwrap();
    setup.worker_repo.unregister(&worker.id).await.unwrap();
}
