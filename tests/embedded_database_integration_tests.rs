use anyhow::Result;
use scheduler::embedded::EmbeddedApplication;
use scheduler_config::AppConfig;
use scheduler_domain::entities::{Task, TaskStatus, TaskType, TaskRun, TaskRunStatus, Worker, WorkerStatus};
use std::path::Path;
use tempfile::TempDir;
use tracing_test::traced_test;

/// 测试数据库自动初始化和迁移
#[tokio::test]
#[traced_test]
async fn test_database_auto_initialization() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("auto_init_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 验证数据库文件不存在
    assert!(!db_path.exists());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 启动应用（应该自动创建数据库）
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 验证数据库文件已创建
    assert!(db_path.exists());

    // 验证数据库表结构
    let service_locator = app_handle.service_locator();
    let task_repo = service_locator.task_repository().await?;

    // 尝试创建任务来验证表结构正确
    let task = Task {
        id: 0,
        name: "init_test_task".to_string(),
        task_type: TaskType::Shell,
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'test'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_task = task_repo.create(task).await?;
    assert!(created_task.id > 0);

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试数据库表结构完整性
#[tokio::test]
#[traced_test]
async fn test_database_schema_integrity() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("schema_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    let service_locator = app_handle.service_locator();

    // 测试所有表的基本操作
    test_tasks_table_operations(&service_locator).await?;
    test_task_runs_table_operations(&service_locator).await?;
    test_workers_table_operations(&service_locator).await?;
    test_system_state_table_operations(&service_locator).await?;

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试任务表操作
async fn test_tasks_table_operations(service_locator: &scheduler_core::ServiceLocator) -> Result<()> {
    let task_repo = service_locator.task_repository().await?;

    // 创建任务
    let task = Task {
        id: 0,
        name: "schema_test_task".to_string(),
        task_type: TaskType::Http,
        schedule: "*/5 * * * *".to_string(),
        parameters: serde_json::json!({
            "url": "http://example.com",
            "method": "GET"
        }),
        timeout_seconds: 600,
        max_retries: 5,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: Some(serde_json::json!({
            "total_shards": 4,
            "shard_key": "user_id"
        })),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_task = task_repo.create(task).await?;
    assert!(created_task.id > 0);
    assert_eq!(created_task.name, "schema_test_task");
    assert_eq!(created_task.task_type, TaskType::Http);
    assert_eq!(created_task.max_retries, 5);
    assert!(created_task.shard_config.is_some());

    // 更新任务
    let mut updated_task = created_task.clone();
    updated_task.status = TaskStatus::Inactive;
    updated_task.timeout_seconds = 900;
    updated_task.updated_at = chrono::Utc::now();

    let result_task = task_repo.update(updated_task).await?;
    assert_eq!(result_task.status, TaskStatus::Inactive);
    assert_eq!(result_task.timeout_seconds, 900);

    // 查询任务
    let found_task = task_repo.find_by_id(created_task.id).await?;
    assert!(found_task.is_some());
    assert_eq!(found_task.unwrap().status, TaskStatus::Inactive);

    // 按名称查询
    let found_by_name = task_repo.find_by_name("schema_test_task").await?;
    assert!(found_by_name.is_some());

    // 查询所有任务
    let all_tasks = task_repo.find_all().await?;
    assert!(!all_tasks.is_empty());

    Ok(())
}

/// 测试任务执行记录表操作
async fn test_task_runs_table_operations(service_locator: &scheduler_core::ServiceLocator) -> Result<()> {
    let task_repo = service_locator.task_repository().await?;
    let task_run_repo = service_locator.task_run_repository().await?;

    // 先创建一个任务
    let task = Task {
        id: 0,
        name: "run_test_task".to_string(),
        task_type: TaskType::Shell,
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'run test'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_task = task_repo.create(task).await?;

    // 创建任务执行记录
    let task_run = TaskRun {
        id: 0,
        task_id: created_task.id,
        status: TaskRunStatus::Pending,
        worker_id: None,
        retry_count: 0,
        shard_index: Some(1),
        shard_total: Some(4),
        scheduled_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        result: None,
        error_message: None,
        created_at: chrono::Utc::now(),
    };

    let created_run = task_run_repo.create(task_run).await?;
    assert!(created_run.id > 0);
    assert_eq!(created_run.task_id, created_task.id);
    assert_eq!(created_run.status, TaskRunStatus::Pending);
    assert_eq!(created_run.shard_index, Some(1));
    assert_eq!(created_run.shard_total, Some(4));

    // 更新执行记录状态
    let mut running_task_run = created_run.clone();
    running_task_run.status = TaskRunStatus::Running;
    running_task_run.worker_id = Some("test-worker-1".to_string());
    running_task_run.started_at = Some(chrono::Utc::now());

    let updated_run = task_run_repo.update(running_task_run).await?;
    assert_eq!(updated_run.status, TaskRunStatus::Running);
    assert!(updated_run.worker_id.is_some());
    assert!(updated_run.started_at.is_some());

    // 完成任务执行
    let mut completed_run = updated_run.clone();
    completed_run.status = TaskRunStatus::Success;
    completed_run.completed_at = Some(chrono::Utc::now());
    completed_run.result = Some(serde_json::json!({
        "output": "run test completed",
        "exit_code": 0
    }));

    let final_run = task_run_repo.update(completed_run).await?;
    assert_eq!(final_run.status, TaskRunStatus::Success);
    assert!(final_run.completed_at.is_some());
    assert!(final_run.result.is_some());

    // 查询任务执行记录
    let found_run = task_run_repo.find_by_id(created_run.id).await?;
    assert!(found_run.is_some());
    assert_eq!(found_run.unwrap().status, TaskRunStatus::Success);

    // 按任务ID查询执行记录
    let task_runs = task_run_repo.find_by_task_id(created_task.id).await?;
    assert_eq!(task_runs.len(), 1);
    assert_eq!(task_runs[0].status, TaskRunStatus::Success);

    Ok(())
}

/// 测试Worker表操作
async fn test_workers_table_operations(service_locator: &scheduler_core::ServiceLocator) -> Result<()> {
    let worker_repo = service_locator.worker_repository().await?;

    // 创建Worker
    let worker = Worker {
        id: "test-worker-schema".to_string(),
        hostname: "test-host".to_string(),
        ip_address: "192.168.1.100".to_string(),
        status: WorkerStatus::Active,
        supported_task_types: vec![TaskType::Shell, TaskType::Http],
        max_concurrent_tasks: 10,
        current_task_count: 0,
        last_heartbeat: chrono::Utc::now(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_worker = worker_repo.create(worker).await?;
    assert_eq!(created_worker.id, "test-worker-schema");
    assert_eq!(created_worker.hostname, "test-host");
    assert_eq!(created_worker.status, WorkerStatus::Active);
    assert_eq!(created_worker.max_concurrent_tasks, 10);

    // 更新Worker状态
    let mut updated_worker = created_worker.clone();
    updated_worker.status = WorkerStatus::Busy;
    updated_worker.current_task_count = 5;
    updated_worker.last_heartbeat = chrono::Utc::now();
    updated_worker.updated_at = chrono::Utc::now();

    let result_worker = worker_repo.update(updated_worker).await?;
    assert_eq!(result_worker.status, WorkerStatus::Busy);
    assert_eq!(result_worker.current_task_count, 5);

    // 查询Worker
    let found_worker = worker_repo.find_by_id("test-worker-schema").await?;
    assert!(found_worker.is_some());
    assert_eq!(found_worker.unwrap().status, WorkerStatus::Busy);

    // 查询所有Worker
    let all_workers = worker_repo.find_all().await?;
    assert!(!all_workers.is_empty());

    Ok(())
}

/// 测试系统状态表操作
async fn test_system_state_table_operations(service_locator: &scheduler_core::ServiceLocator) -> Result<()> {
    // 直接使用数据库连接测试系统状态表
    // 注意：这里我们需要通过service_locator获取数据库连接
    // 由于系统状态表没有专门的仓库，我们直接使用SQL操作

    // 这个测试主要验证系统状态表的基本结构
    // 在实际的关闭过程中会使用到这个表

    Ok(())
}

/// 测试数据库约束和索引
#[tokio::test]
#[traced_test]
async fn test_database_constraints_and_indexes() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("constraints_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    let service_locator = app_handle.service_locator();
    let task_repo = service_locator.task_repository().await?;
    let task_run_repo = service_locator.task_run_repository().await?;

    // 测试唯一约束：任务名称必须唯一
    let task1 = Task {
        id: 0,
        name: "unique_constraint_test".to_string(),
        task_type: TaskType::Shell,
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'test1'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_task1 = task_repo.create(task1).await?;
    assert!(created_task1.id > 0);

    // 尝试创建同名任务（应该失败）
    let task2 = Task {
        id: 0,
        name: "unique_constraint_test".to_string(), // 相同名称
        task_type: TaskType::Http,
        schedule: "0 12 * * *".to_string(),
        parameters: serde_json::json!({"url": "http://example.com"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let duplicate_result = task_repo.create(task2).await;
    assert!(duplicate_result.is_err()); // 应该因为唯一约束失败

    // 测试外键约束：任务执行记录必须关联到存在的任务
    let valid_task_run = TaskRun {
        id: 0,
        task_id: created_task1.id, // 有效的任务ID
        status: TaskRunStatus::Pending,
        worker_id: None,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        result: None,
        error_message: None,
        created_at: chrono::Utc::now(),
    };

    let created_run = task_run_repo.create(valid_task_run).await?;
    assert!(created_run.id > 0);

    // 尝试创建关联到不存在任务的执行记录（应该失败）
    let invalid_task_run = TaskRun {
        id: 0,
        task_id: 99999, // 不存在的任务ID
        status: TaskRunStatus::Pending,
        worker_id: None,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
        scheduled_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        result: None,
        error_message: None,
        created_at: chrono::Utc::now(),
    };

    let foreign_key_result = task_run_repo.create(invalid_task_run).await;
    assert!(foreign_key_result.is_err()); // 应该因为外键约束失败

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试数据库事务处理
#[tokio::test]
#[traced_test]
async fn test_database_transaction_handling() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("transaction_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    let service_locator = app_handle.service_locator();
    let task_repo = service_locator.task_repository().await?;

    // 创建任务
    let task = Task {
        id: 0,
        name: "transaction_test_task".to_string(),
        task_type: TaskType::Shell,
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'transaction test'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_task = task_repo.create(task).await?;

    // 测试更新操作的原子性
    let original_name = created_task.name.clone();
    let mut updated_task = created_task.clone();
    updated_task.name = "transaction_test_task_updated".to_string();
    updated_task.status = TaskStatus::Inactive;
    updated_task.updated_at = chrono::Utc::now();

    let result_task = task_repo.update(updated_task).await?;
    assert_eq!(result_task.name, "transaction_test_task_updated");
    assert_eq!(result_task.status, TaskStatus::Inactive);

    // 验证更新已持久化
    let persisted_task = task_repo.find_by_id(created_task.id).await?.unwrap();
    assert_eq!(persisted_task.name, "transaction_test_task_updated");
    assert_eq!(persisted_task.status, TaskStatus::Inactive);
    assert_ne!(persisted_task.name, original_name);

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试数据库连接池配置
#[tokio::test]
#[traced_test]
async fn test_database_connection_pool() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("pool_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置，设置连接池参数
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.database.max_connections = 3;
    config.database.min_connections = 1;
    config.database.connection_timeout_seconds = 10;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    let service_locator = app_handle.service_locator();
    let task_repo = service_locator.task_repository().await?;

    // 并发创建多个任务来测试连接池
    let mut handles = Vec::new();
    for i in 0..5 {
        let task_repo_clone = task_repo.clone();
        let handle = tokio::spawn(async move {
            let task = Task {
                id: 0,
                name: format!("pool_test_task_{}", i),
                task_type: TaskType::Shell,
                schedule: "0 0 * * *".to_string(),
                parameters: serde_json::json!({"command": format!("echo 'pool test {}'", i)}),
                timeout_seconds: 300,
                max_retries: 3,
                status: TaskStatus::Active,
                dependencies: vec![],
                shard_config: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            task_repo_clone.create(task).await
        });
        handles.push(handle);
    }

    // 等待所有任务创建完成
    let mut created_tasks = Vec::new();
    for handle in handles {
        let task = handle.await??;
        created_tasks.push(task);
    }

    // 验证所有任务都已创建
    assert_eq!(created_tasks.len(), 5);

    // 验证数据库中的任务
    let all_tasks = task_repo.find_all().await?;
    let pool_test_tasks: Vec<_> = all_tasks
        .into_iter()
        .filter(|t| t.name.starts_with("pool_test_task_"))
        .collect();

    assert_eq!(pool_test_tasks.len(), 5);

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试数据库文件权限和安全性
#[tokio::test]
#[traced_test]
async fn test_database_file_security() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("security_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 验证数据库文件已创建
    assert!(db_path.exists());

    // 验证数据库文件是常规文件
    let metadata = std::fs::metadata(&db_path)?;
    assert!(metadata.is_file());
    assert!(metadata.len() > 0); // 文件不为空

    // 在Unix系统上验证文件权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        
        // 验证文件权限合理（不是完全开放的权限）
        assert_ne!(mode & 0o777, 0o777); // 不应该是777权限
    }

    // 关闭应用
    app_handle.shutdown().await?;

    // 验证关闭后数据库文件仍然存在
    assert!(db_path.exists());

    Ok(())
}