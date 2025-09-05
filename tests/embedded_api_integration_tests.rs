use anyhow::Result;
use reqwest::Client;
use scheduler::embedded::EmbeddedApplication;
use scheduler_config::AppConfig;
use serde_json::{json, Value};
use tempfile::TempDir;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing_test::traced_test;

/// 测试嵌入式API的集成功能
#[tokio::test]
#[traced_test]
async fn test_embedded_api_integration() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("api_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置，使用固定端口进行测试
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:18080".to_string(); // 使用固定端口

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 等待API服务器启动
    sleep(Duration::from_millis(500)).await;

    let base_url = format!("http://{}", app_handle.api_address());
    let client = Client::new();

    // 测试健康检查端点
    test_health_endpoint(&client, &base_url).await?;

    // 测试根路径端点
    test_root_endpoint(&client, &base_url).await?;

    // 测试任务管理API
    test_task_management_api(&client, &base_url).await?;

    // 测试系统指标端点
    test_metrics_endpoint(&client, &base_url).await?;

    // 测试错误处理
    test_api_error_handling(&client, &base_url).await?;

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试健康检查端点
async fn test_health_endpoint(client: &Client, base_url: &str) -> Result<()> {
    let response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/health", base_url)).send()
    ).await??;

    assert_eq!(response.status(), 200);

    let health_data: Value = response.json().await?;
    assert!(health_data.get("status").is_some());
    assert_eq!(health_data["status"], "healthy");

    Ok(())
}

/// 测试根路径端点
async fn test_root_endpoint(client: &Client, base_url: &str) -> Result<()> {
    let response = timeout(
        Duration::from_secs(5),
        client.get(base_url).send()
    ).await??;

    assert_eq!(response.status(), 200);

    let root_data: Value = response.json().await?;
    assert!(root_data.get("name").is_some());
    assert!(root_data.get("version").is_some());
    assert!(root_data.get("status").is_some());

    Ok(())
}

/// 测试任务管理API
async fn test_task_management_api(client: &Client, base_url: &str) -> Result<()> {
    // 1. 创建任务
    let create_task_payload = json!({
        "name": "api_test_task",
        "task_type": "Shell",
        "schedule": "0 0 0 * * *",
        "parameters": {
            "command": "echo 'API test'"
        },
        "timeout_seconds": 300,
        "max_retries": 3
    });

    let create_response = timeout(
        Duration::from_secs(5),
        client.post(&format!("{}/api/tasks", base_url))
            .json(&create_task_payload)
            .send()
    ).await??;

    // Debug: 打印创建任务响应状态和内容
    println!("Create task response status: {}", create_response.status());
    let status_code = create_response.status().as_u16();
    if create_response.status() != 201 {
        let error_text = create_response.text().await?;
        println!("Create task response error: {}", error_text);
        panic!("Create task endpoint returned status {}", status_code);
    }

    let created_task: Value = create_response.json().await?;
    println!("Created task response: {}", serde_json::to_string_pretty(&created_task)?);
    
    let task_data = &created_task["data"];
    assert!(task_data.get("id").is_some());
    assert_eq!(task_data["name"], "api_test_task");
    assert_eq!(task_data["task_type"], "Shell");

    let task_id = task_data["id"].as_i64().unwrap();

    // 2. 获取任务列表
    let list_response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/api/tasks", base_url)).send()
    ).await??;

    assert_eq!(list_response.status(), 200);

    let tasks_list: Value = list_response.json().await?;
    assert!(tasks_list.is_array());
    let tasks = tasks_list.as_array().unwrap();
    assert!(!tasks.is_empty());

    // 验证创建的任务在列表中
    let found_task = tasks.iter().find(|t| t["id"] == task_id);
    assert!(found_task.is_some());

    // 3. 获取单个任务
    let get_response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/api/tasks/{}", base_url, task_id)).send()
    ).await??;

    assert_eq!(get_response.status(), 200);

    let task_detail: Value = get_response.json().await?;
    assert_eq!(task_detail["id"], task_id);
    assert_eq!(task_detail["name"], "api_test_task");

    // 4. 更新任务
    let update_payload = json!({
        "name": "api_test_task_updated",
        "task_type": "Shell",
        "schedule": "0 12 * * *",
        "parameters": {
            "command": "echo 'API test updated'"
        },
        "timeout_seconds": 600,
        "max_retries": 5,
        "status": "Inactive"
    });

    let update_response = timeout(
        Duration::from_secs(5),
        client.put(&format!("{}/api/tasks/{}", base_url, task_id))
            .json(&update_payload)
            .send()
    ).await??;

    assert_eq!(update_response.status(), 200);

    let updated_task: Value = update_response.json().await?;
    assert_eq!(updated_task["name"], "api_test_task_updated");
    assert_eq!(updated_task["schedule"], "0 12 * * *");
    assert_eq!(updated_task["timeout_seconds"], 600);

    // 5. 触发任务执行
    let trigger_response = timeout(
        Duration::from_secs(5),
        client.post(&format!("{}/api/tasks/{}/trigger", base_url, task_id)).send()
    ).await??;

    assert_eq!(trigger_response.status(), 200);

    let trigger_result: Value = trigger_response.json().await?;
    assert!(trigger_result.get("task_run_id").is_some());

    // 6. 获取任务执行历史
    let runs_response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/api/tasks/{}/runs", base_url, task_id)).send()
    ).await??;

    assert_eq!(runs_response.status(), 200);

    let task_runs: Value = runs_response.json().await?;
    assert!(task_runs.is_array());
    let runs = task_runs.as_array().unwrap();
    assert!(!runs.is_empty());

    // 7. 删除任务
    let delete_response = timeout(
        Duration::from_secs(5),
        client.delete(&format!("{}/api/tasks/{}", base_url, task_id)).send()
    ).await??;

    assert_eq!(delete_response.status(), 204);

    // 验证任务已删除
    let get_deleted_response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/api/tasks/{}", base_url, task_id)).send()
    ).await??;

    assert_eq!(get_deleted_response.status(), 404);

    Ok(())
}

/// 测试系统指标端点
async fn test_metrics_endpoint(client: &Client, base_url: &str) -> Result<()> {
    let response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/metrics", base_url)).send()
    ).await??;

    assert_eq!(response.status(), 200);

    let metrics_text = response.text().await?;
    
    // 验证Prometheus格式的指标
    assert!(metrics_text.contains("# HELP"));
    assert!(metrics_text.contains("# TYPE"));
    
    // 验证一些基本指标存在
    assert!(metrics_text.contains("scheduler_"));

    Ok(())
}

/// 测试API错误处理
async fn test_api_error_handling(client: &Client, base_url: &str) -> Result<()> {
    // 1. 测试获取不存在的任务
    let not_found_response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/api/tasks/99999", base_url)).send()
    ).await??;

    assert_eq!(not_found_response.status(), 404);

    let error_data: Value = not_found_response.json().await?;
    assert!(error_data.get("error").is_some());

    // 2. 测试无效的任务创建请求
    let invalid_payload = json!({
        "name": "", // 空名称应该失败
        "task_type": "InvalidType",
        "schedule": "invalid cron"
    });

    let invalid_response = timeout(
        Duration::from_secs(5),
        client.post(&format!("{}/api/tasks", base_url))
            .json(&invalid_payload)
            .send()
    ).await??;

    assert_eq!(invalid_response.status(), 400);

    let validation_error: Value = invalid_response.json().await?;
    assert!(validation_error.get("error").is_some());

    // 3. 测试重复任务名称
    let task_payload = json!({
        "name": "duplicate_test_task",
        "task_type": "Shell",
        "schedule": "0 0 0 * * *",
        "parameters": {
            "command": "echo 'test'"
        }
    });

    // 创建第一个任务
    let first_response = timeout(
        Duration::from_secs(5),
        client.post(&format!("{}/api/tasks", base_url))
            .json(&task_payload)
            .send()
    ).await??;

    assert_eq!(first_response.status(), 201);

    // 尝试创建同名任务
    let duplicate_response = timeout(
        Duration::from_secs(5),
        client.post(&format!("{}/api/tasks", base_url))
            .json(&task_payload)
            .send()
    ).await??;

    assert_eq!(duplicate_response.status(), 409); // Conflict

    let conflict_error: Value = duplicate_response.json().await?;
    assert!(conflict_error.get("error").is_some());

    Ok(())
}

/// 测试Worker API端点
#[tokio::test]
#[traced_test]
async fn test_worker_api_endpoints() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("worker_api_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:18081".to_string();

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 等待API服务器启动
    sleep(Duration::from_millis(500)).await;

    let base_url = format!("http://{}", app_handle.api_address());
    let client = Client::new();

    // 测试Worker列表端点
    let workers_response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/api/workers", base_url)).send()
    ).await??;

    // Debug: 打印响应状态和内容
    println!("Workers response status: {}", workers_response.status());
    let status_code = workers_response.status().as_u16();
    if workers_response.status() != 200 {
        let error_text = workers_response.text().await?;
        println!("Workers response error: {}", error_text);
        panic!("Workers endpoint returned status {}", status_code);
    }

    let workers: Value = workers_response.json().await?;
    assert!(workers.is_array());

    // 测试系统状态端点
    let system_response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/api/system/health", base_url)).send()
    ).await??;

    assert_eq!(system_response.status(), 200);

    let system_status: Value = system_response.json().await?;
    assert!(system_status.get("status").is_some());
    assert!(system_status.get("uptime").is_some());

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试API认证功能（当启用时）
#[tokio::test]
#[traced_test]
async fn test_api_authentication_disabled() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("auth_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置，验证认证默认关闭
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:18082".to_string();

    // 验证认证默认关闭
    assert!(!config.api.auth.enabled);

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 等待API服务器启动
    sleep(Duration::from_millis(500)).await;

    let base_url = format!("http://{}", app_handle.api_address());
    let client = Client::new();

    // 测试无需认证即可访问API
    let response = timeout(
        Duration::from_secs(5),
        client.get(&format!("{}/api/tasks", base_url)).send()
    ).await??;

    assert_eq!(response.status(), 200); // 应该成功，因为认证已关闭

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}

/// 测试CORS配置
#[tokio::test]
#[traced_test]
async fn test_cors_configuration() -> Result<()> {
    // 创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("cors_test.db");
    let db_url = format!("sqlite:{}", db_path.display());

    // 创建配置
    let mut config = AppConfig::embedded_default();
    config.database.url = db_url;
    config.api.bind_address = "127.0.0.1:18083".to_string();

    // 验证CORS默认启用
    assert!(config.api.cors_enabled);
    assert_eq!(config.api.cors_origins, vec!["*"]);

    // 启动应用
    let app = EmbeddedApplication::new(config).await?;
    let app_handle = app.start().await?;

    // 等待API服务器启动
    sleep(Duration::from_millis(500)).await;

    let base_url = format!("http://{}", app_handle.api_address());
    let client = Client::new();

    // 测试CORS预检请求
    let preflight_response = timeout(
        Duration::from_secs(5),
        client.request(reqwest::Method::OPTIONS, &format!("{}/api/tasks", base_url))
            .header("Origin", "http://localhost:3000")
            .header("Access-Control-Request-Method", "POST")
            .header("Access-Control-Request-Headers", "Content-Type")
            .send()
    ).await??;

    // 验证CORS头部
    assert!(preflight_response.headers().contains_key("access-control-allow-origin"));

    // 关闭应用
    app_handle.shutdown().await?;

    Ok(())
}