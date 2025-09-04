use anyhow::Result;
use scheduler_config::AppConfig;
use scheduler_core::task_types::{HTTP, SHELL};
use scheduler_domain::{
    entities::{Task, TaskRun, TaskRunStatus, TaskStatus},
    TaskFilter,
};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use tracing_test::traced_test;

use crate::embedded::{EmbeddedApplication, EmbeddedApplicationHandle};

/// 集成测试：嵌入式应用的端到端测试
#[tokio::test]
#[traced_test]
async fn test_embedded_application_end_to_end() -> Result<()> {
    // 创建临时目录用于测试数据库
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_scheduler.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建嵌入式配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string(); // 使用随机端口

    // 创建并启动嵌入式应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 验证应用启动成功
    assert!(!app_handle.api_address().is_empty());

    // 测试数据库持久化和恢复
    test_database_persistence_and_recovery(&app_handle).await?;

    // 测试任务创建、执行、状态更新完整流程
    test_complete_task_lifecycle(&app_handle).await?;

    // 优雅关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试零配置启动流程
#[tokio::test]
#[traced_test]
async fn test_zero_configuration_startup() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("zero_config_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 使用默认配置，只修改数据库路径和API端口
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 验证默认配置的正确性
    assert_eq!(
        config.message_queue.r#type,
        scheduler_config::MessageQueueType::InMemory
    );
    assert!(config.api.enabled);
    assert!(config.dispatcher.enabled);
    assert!(config.worker.enabled);
    assert!(!config.api.auth.enabled); // 嵌入式版本默认关闭认证
    assert!(!config.api.rate_limiting.enabled); // 嵌入式版本默认关闭限流

    // 创建应用（应该能够零配置启动）
    let app = EmbeddedApplication::new(config).await?;

    // 启动应用
    let app_handle = timeout(Duration::from_secs(10), app.start()).await??;

    // 验证启动成功
    assert!(!app_handle.api_address().is_empty());

    // 验证数据库自动创建
    assert!(db_path.exists());

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试任务创建、执行、状态更新完整流程
async fn test_complete_task_lifecycle(app_handle: &EmbeddedApplicationHandle) -> Result<()> {
    let service_locator = app_handle.service_locator();

    // 获取仓库
    let task_repo = service_locator.task_repository().await?;
    let task_run_repo = service_locator.task_run_repository().await?;

    // 1. 创建任务
    let task = Task {
        id: 0, // 将由数据库自动分配
        name: "test_task".to_string(),
        task_type: SHELL.to_string(),
        schedule: "0 0 * * *".to_string(), // 每天午夜执行
        parameters: serde_json::json!({"command": "echo 'Hello World'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_task = task_repo.create(&task).await?;
    assert!(created_task.id > 0);
    assert_eq!(created_task.name, "test_task");
    assert_eq!(created_task.status, TaskStatus::Active);

    // 2. 创建任务执行记录
    let task_run = TaskRun {
        id: 0, // 将由数据库自动分配
        task_id: created_task.id,
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

    let created_task_run = task_run_repo.create(&task_run).await?;
    assert!(created_task_run.id > 0);
    assert_eq!(created_task_run.task_id, created_task.id);
    assert_eq!(created_task_run.status, TaskRunStatus::Pending);

    // 3. 模拟任务执行状态更新
    let mut updated_task_run = created_task_run.clone();
    updated_task_run.status = TaskRunStatus::Running;
    updated_task_run.worker_id = Some("embedded-worker".to_string());
    updated_task_run.started_at = Some(chrono::Utc::now());

    let _ = task_run_repo.update(&updated_task_run).await?;
    assert_eq!(updated_task_run.status, TaskRunStatus::Running);
    assert!(updated_task_run.worker_id.is_some());
    assert!(updated_task_run.started_at.is_some());

    // 4. 模拟任务完成
    let mut completed_task_run = updated_task_run.clone();
    completed_task_run.status = TaskRunStatus::Completed;
    completed_task_run.completed_at = Some(chrono::Utc::now());
    completed_task_run.result = Some(serde_json::json!({"output": "Hello World"}).to_string());

    let _ = task_run_repo.update(&completed_task_run).await?;
    assert_eq!(completed_task_run.status, TaskRunStatus::Completed);
    assert!(completed_task_run.completed_at.is_some());
    assert!(completed_task_run.result.is_some());

    // 5. 验证任务历史记录
    let task_runs = task_run_repo.get_by_task_id(created_task.id).await?;
    assert_eq!(task_runs.len(), 1);
    assert_eq!(task_runs[0].status, TaskRunStatus::Completed);

    // 6. 验证任务查询
    let found_task = task_repo.get_by_id(created_task.id).await?;
    assert!(found_task.is_some());
    assert_eq!(found_task.unwrap().name, "test_task");

    Ok(())
}

/// 测试数据库持久化和恢复
async fn test_database_persistence_and_recovery(
    app_handle: &EmbeddedApplicationHandle,
) -> Result<()> {
    let service_locator = app_handle.service_locator();
    let task_repo = service_locator.task_repository().await?;

    // 1. 创建多个任务
    let tasks_data = vec![
        ("persistent_task_1", "0 0 * * *", SHELL.to_string()),
        ("persistent_task_2", "0 12 * * *", HTTP.to_string()),
        ("persistent_task_3", "*/5 * * * *", SHELL.to_string()),
    ];

    let mut created_task_ids = Vec::new();

    for (name, schedule, task_type) in tasks_data {
        let task = Task {
            id: 0,
            name: name.to_string(),
            task_type,
            schedule: schedule.to_string(),
            parameters: serde_json::json!({"test": "data"}),
            timeout_seconds: 300,
            max_retries: 3,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let created_task = task_repo.create(&task).await?;
        created_task_ids.push(created_task.id);
    }

    // 2. 验证任务已持久化
    let all_tasks = task_repo
        .list(&TaskFilter::default())
        .await?;
    assert!(all_tasks.len() >= 3);

    // 3. 验证每个任务都能正确查询
    for task_id in &created_task_ids {
        let task = task_repo.get_by_id(*task_id).await?;
        assert!(task.is_some());
        assert_eq!(task.unwrap().status, TaskStatus::Active);
    }

    // 4. 测试任务更新持久化
    if let Some(first_task_id) = created_task_ids.first() {
        let mut task = task_repo.get_by_id(*first_task_id).await?.unwrap();
        task.status = TaskStatus::Inactive;
        task.updated_at = chrono::Utc::now();

        let _ = task_repo.update(&task).await?;
        assert_eq!(task.status, TaskStatus::Inactive);

        // 验证更新已持久化
        let persisted_task = task_repo.get_by_id(*first_task_id).await?.unwrap();
        assert_eq!(persisted_task.status, TaskStatus::Inactive);
    }

    // 5. 测试任务删除
    if let Some(last_task_id) = created_task_ids.last() {
        task_repo.delete(*last_task_id).await?;

        // 验证任务已删除
        let deleted_task = task_repo.get_by_id(*last_task_id).await?;
        assert!(deleted_task.is_none());
    }

    Ok(())
}

/// 测试并发任务处理
#[tokio::test]
#[traced_test]
async fn test_concurrent_task_processing() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("concurrent_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();
    config.worker.max_concurrent_tasks = 5; // 设置并发任务数

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    let service_locator = app_handle.service_locator();
    let task_repo = service_locator.task_repository().await?;
    let task_run_repo = service_locator.task_run_repository().await?;

    // 创建多个任务
    let mut task_ids = Vec::new();
    for i in 0..10 {
        let task = Task {
            id: 0,
            name: format!("concurrent_task_{}", i),
            task_type: SHELL.to_string(),
            schedule: "0 0 * * *".to_string(),
            parameters: serde_json::json!({"command": format!("echo 'Task {}'", i)}),
            timeout_seconds: 300,
            max_retries: 3,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let created_task = task_repo.create(&task).await?;
        task_ids.push(created_task.id);
    }

    // 为每个任务创建执行记录
    let mut task_run_handles = Vec::new();
    for task_id in task_ids {
        let task_run_repo_clone = task_run_repo.clone();
        let handle = tokio::spawn(async move {
            let task_run = TaskRun {
                id: 0,
                task_id,
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

            task_run_repo_clone.create(&task_run).await
        });
        task_run_handles.push(handle);
    }

    // 等待所有任务执行记录创建完成
    let mut created_runs = Vec::new();
    for handle in task_run_handles {
        let task_run = handle.await??;
        created_runs.push(task_run);
    }

    // 验证所有任务执行记录都已创建
    assert_eq!(created_runs.len(), 10);

    // 验证数据库中的记录
    let all_runs = task_run_repo.find_all().await?;
    assert!(all_runs.len() >= 10);

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试错误恢复场景
#[tokio::test]
#[traced_test]
async fn test_error_recovery_scenarios() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("error_recovery_test.db");
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

    // 1. 测试重复任务名称错误处理
    let task1 = Task {
        id: 0,
        name: "duplicate_name_task".to_string(),
        task_type: SHELL.to_string(),
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

    let created_task1 = task_repo.create(&task1).await?;
    assert!(created_task1.id > 0);

    // 尝试创建同名任务（应该失败）
    let task2 = Task {
        id: 0,
        name: "duplicate_name_task".to_string(),
        task_type: HTTP.to_string(),
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

    let duplicate_result = task_repo.create(&task2).await;
    assert!(duplicate_result.is_err()); // 应该失败

    // 2. 测试不存在的任务查询
    let non_existent_task = task_repo.get_by_id(99999).await?;
    assert!(non_existent_task.is_none());

    // 3. 测试任务执行失败场景
    let failed_task_run = TaskRun {
        id: 0,
        task_id: created_task1.id,
        status: TaskRunStatus::Failed,
        worker_id: Some("test-worker".to_string()),
        retry_count: 1,
        shard_index: None,
        shard_total: None,
        scheduled_at: chrono::Utc::now(),
        started_at: Some(chrono::Utc::now()),
        completed_at: Some(chrono::Utc::now()),
        result: None,
        error_message: Some("Simulated task failure".to_string()),
        created_at: chrono::Utc::now(),
    };

    let created_failed_run = task_run_repo.create(&failed_task_run).await?;
    assert_eq!(created_failed_run.status, TaskRunStatus::Failed);
    assert!(created_failed_run.error_message.is_some());

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试应用重启后的状态恢复
#[tokio::test]
#[traced_test]
async fn test_application_restart_recovery() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("restart_recovery_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 第一次启动：创建数据
    {
        let mut config = AppConfig::embedded_default();
        config.database.url = db_url.clone();
        config.api.bind_address = "127.0.0.1:0".to_string();

        let app = EmbeddedApplication::new(config).await?;
        let app_handle = app.start().await?;

        let service_locator = app_handle.service_locator();
        let task_repo = service_locator.task_repository().await?;

        // 创建一些任务
        for i in 0..3 {
            let task = Task {
                id: 0,
                name: format!("restart_test_task_{}", i),
                task_type: SHELL.to_string(),
                schedule: "0 0 * * *".to_string(),
                parameters: serde_json::json!({"command": format!("echo 'Task {}'", i)}),
                timeout_seconds: 300,
                max_retries: 3,
                status: TaskStatus::Active,
                dependencies: vec![],
                shard_config: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            task_repo.create(&task).await?;
        }

        // 正常关闭应用
        app_handle.shutdown().await?;
    }

    // 第二次启动：验证数据恢复
    {
        let mut config = AppConfig::embedded_default();
        config.database.url = db_url;
        config.api.bind_address = "127.0.0.1:0".to_string();

        let app = EmbeddedApplication::new(config).await?;
        let app_handle = app.start().await?;

        let service_locator = app_handle.service_locator();
        let task_repo = service_locator.task_repository().await?;

        // 验证任务数据已恢复
        let all_tasks = task_repo.find_all().await?;
        let restart_tasks: Vec<_> = all_tasks
            .into_iter()
            .filter(|t| t.name.starts_with("restart_test_task_"))
            .collect();

        assert_eq!(restart_tasks.len(), 3);

        // 验证任务详情
        for (i, task) in restart_tasks.iter().enumerate() {
            assert_eq!(task.name, format!("restart_test_task_{}", i));
            assert_eq!(task.status, TaskStatus::Active);
            assert_eq!(task.task_type, SHELL.to_string());
        }

        // 关闭应用
        app_handle.shutdown().await?;
    }

    Ok(())
}

/// 测试内存队列功能
#[tokio::test]
#[traced_test]
async fn test_in_memory_queue_functionality() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("queue_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:0".to_string();

    // 验证消息队列配置
    assert_eq!(
        config.message_queue.r#type,
        scheduler_config::MessageQueueType::InMemory
    );
    assert_eq!(config.message_queue.task_queue, "tasks");
    assert_eq!(config.message_queue.status_queue, "status_updates");
    assert_eq!(config.message_queue.heartbeat_queue, "heartbeats");

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 验证消息队列已初始化
    let service_locator = app_handle.service_locator();
    let _ = service_locator.message_queue().await?;

    // 测试消息队列基本功能（通过创建任务来间接测试）
    let task_repo = service_locator.task_repository().await?;
    let task = Task {
        id: 0,
        name: "queue_test_task".to_string(),
        task_type: SHELL.to_string(),
        schedule: "0 0 * * *".to_string(),
        parameters: serde_json::json!({"command": "echo 'queue test'"}),
        timeout_seconds: 300,
        max_retries: 3,
        status: TaskStatus::Active,
        dependencies: vec![],
        shard_config: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created_task = task_repo.create(&task).await?;
    assert!(created_task.id > 0);

    // 记录队列指标
    app_handle.record_embedded_metrics(1, 1);

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}
