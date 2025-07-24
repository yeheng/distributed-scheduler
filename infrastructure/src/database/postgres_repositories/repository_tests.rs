#[cfg(test)]
mod tests {
    use crate::{PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository};

    use chrono::Utc;
    use scheduler_core::{
        models::{Task, TaskRun, TaskRunStatus, TaskStatus, WorkerInfo, WorkerStatus},
        traits::{TaskRepository, TaskRunRepository, WorkerRepository},
    };
    use sqlx::PgPool;

    // 这些测试需要一个运行的PostgreSQL数据库
    // 可以使用testcontainers来创建临时数据库实例

    async fn setup_test_db() -> PgPool {
        // 在实际测试中，这里应该创建一个测试数据库
        // 这里只是示例代码
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://test:test@localhost/scheduler_test".to_string());

        PgPool::connect(&database_url).await.unwrap()
    }

    #[tokio::test]
    #[ignore] // 需要数据库连接
    async fn test_task_repository_crud() {
        let pool = setup_test_db().await;
        let repo = PostgresTaskRepository::new(pool);

        // 创建测试任务
        let mut task = Task::new(
            "test_task".to_string(),
            "shell".to_string(),
            "0 0 * * *".to_string(),
            serde_json::json!({"command": "echo hello"}),
        );

        // 测试创建
        let created_task = repo.create(&task).await.unwrap();
        assert!(created_task.id > 0);
        assert_eq!(created_task.name, task.name);

        // 测试根据ID查询
        let found_task = repo.get_by_id(created_task.id).await.unwrap();
        assert!(found_task.is_some());
        assert_eq!(found_task.unwrap().name, task.name);

        // 测试根据名称查询
        let found_by_name = repo.get_by_name(&task.name).await.unwrap();
        assert!(found_by_name.is_some());
        assert_eq!(found_by_name.unwrap().id, created_task.id);

        // 测试更新
        task.id = created_task.id;
        task.status = TaskStatus::Inactive;
        repo.update(&task).await.unwrap();

        let updated_task = repo.get_by_id(created_task.id).await.unwrap().unwrap();
        assert_eq!(updated_task.status, TaskStatus::Inactive);

        // 测试删除
        repo.delete(created_task.id).await.unwrap();
        let deleted_task = repo.get_by_id(created_task.id).await.unwrap();
        assert!(deleted_task.is_none());
    }

    #[tokio::test]
    #[ignore] // 需要数据库连接
    async fn test_task_run_repository_crud() {
        let pool = setup_test_db().await;
        let task_repo = PostgresTaskRepository::new(pool.clone());
        let run_repo = PostgresTaskRunRepository::new(pool);

        // 先创建一个任务
        let task = Task::new(
            "test_task_for_run".to_string(),
            "shell".to_string(),
            "0 0 * * *".to_string(),
            serde_json::json!({"command": "echo hello"}),
        );
        let created_task = task_repo.create(&task).await.unwrap();

        // 创建任务执行实例
        let task_run = TaskRun::new(created_task.id, Utc::now());
        let created_run = run_repo.create(&task_run).await.unwrap();

        assert!(created_run.id > 0);
        assert_eq!(created_run.task_id, created_task.id);
        assert_eq!(created_run.status, TaskRunStatus::Pending);

        // 测试状态更新
        run_repo
            .update_status(created_run.id, TaskRunStatus::Running, Some("worker-001"))
            .await
            .unwrap();
        let updated_run = run_repo.get_by_id(created_run.id).await.unwrap().unwrap();
        assert_eq!(updated_run.status, TaskRunStatus::Running);
        assert_eq!(updated_run.worker_id, Some("worker-001".to_string()));

        // 测试结果更新
        run_repo
            .update_result(created_run.id, Some("success"), None)
            .await
            .unwrap();
        let result_updated_run = run_repo.get_by_id(created_run.id).await.unwrap().unwrap();
        assert_eq!(result_updated_run.result, Some("success".to_string()));

        // 清理
        run_repo.delete(created_run.id).await.unwrap();
        task_repo.delete(created_task.id).await.unwrap();
    }

    #[tokio::test]
    #[ignore] // 需要数据库连接
    async fn test_worker_repository_crud() {
        let pool = setup_test_db().await;
        let repo = PostgresWorkerRepository::new(pool);

        // 创建Worker信息
        let worker = WorkerInfo {
            id: "test-worker-001".to_string(),
            hostname: "test-host".to_string(),
            ip_address: "192.168.1.100".to_string(),
            supported_task_types: vec!["shell".to_string(), "http".to_string()],
            max_concurrent_tasks: 5,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
        };

        // 测试注册
        repo.register(&worker).await.unwrap();

        // 测试查询
        let found_worker = repo.get_by_id(&worker.id).await.unwrap();
        assert!(found_worker.is_some());
        let found_worker = found_worker.unwrap();
        assert_eq!(found_worker.hostname, worker.hostname);
        assert_eq!(found_worker.status, WorkerStatus::Alive);

        // 测试心跳更新
        let new_heartbeat = Utc::now();
        repo.update_heartbeat(&worker.id, new_heartbeat, 2)
            .await
            .unwrap();

        // 测试状态更新
        repo.update_status(&worker.id, WorkerStatus::Down)
            .await
            .unwrap();
        let updated_worker = repo.get_by_id(&worker.id).await.unwrap().unwrap();
        assert_eq!(updated_worker.status, WorkerStatus::Down);

        // 清理
        repo.unregister(&worker.id).await.unwrap();
        let deleted_worker = repo.get_by_id(&worker.id).await.unwrap();
        assert!(deleted_worker.is_none());
    }

    #[tokio::test]
    #[ignore] // 需要数据库连接
    async fn test_worker_load_stats() {
        let pool = setup_test_db().await;
        let repo = PostgresWorkerRepository::new(pool);

        // 注册测试Worker
        let worker = WorkerInfo {
            id: "load-test-worker".to_string(),
            hostname: "load-test-host".to_string(),
            ip_address: "192.168.1.101".to_string(),
            supported_task_types: vec!["shell".to_string()],
            max_concurrent_tasks: 10,
            current_task_count: 0,
            status: WorkerStatus::Alive,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
        };

        repo.register(&worker).await.unwrap();

        // 获取负载统计
        let stats = repo.get_worker_load_stats().await.unwrap();
        let worker_stats = stats.iter().find(|s| s.worker_id == worker.id);
        assert!(worker_stats.is_some());

        let worker_stats = worker_stats.unwrap();
        assert_eq!(worker_stats.max_concurrent_tasks, 10);
        assert_eq!(worker_stats.current_task_count, 0);
        assert_eq!(worker_stats.load_percentage, 0.0);

        // 清理
        repo.unregister(&worker.id).await.unwrap();
    }
}
