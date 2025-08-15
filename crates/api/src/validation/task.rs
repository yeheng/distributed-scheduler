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
            || c.is_whitespace()
            || c.is_ascii_punctuation()
            || ('\u{4e00}'..='\u{9fff}').contains(&c)
    }) {
        return Err(ValidationError::new("任务名称包含非法字符"));
    }

    // 检查开头和结尾是否为空格
    if name.starts_with(' ') || name.ends_with(' ') {
        return Err(ValidationError::new("任务名称不能以空格开头或结尾"));
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
        Err(e) => Err(ValidationError::new(&*Box::leak(
            format!("无效的CRON表达式: {e}").into_boxed_str(),
        ))),
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
        "SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "UNION", "WHERE", "OR",
        "AND", "EXEC", "EXECUTE", "CAST", "CONVERT",
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
        Err(e) => Err(ValidationError::new(&*Box::leak(
            format!("CRON表达式解析失败: {e}").into_boxed_str(),
        ))),
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
