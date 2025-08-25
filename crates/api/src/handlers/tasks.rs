use axum::{
    extract::{Path, Query, State},
    Json,
};
use scheduler_domain::entities::{Task, TaskFilter, TaskRun, TaskRunStatus, TaskStatus};
use scheduler_domain::repositories::TaskExecutionStats;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    auth::{AuthenticatedUser, Permission},
    error::{ApiError, ApiResult},
    response::{created, success, PaginatedResponse},
    routes::AppState,
    types::{NumericUpdateValue, UpdateValue},
    update_request,
    validation::task::{validate_create_task_request, validate_update_task_request},
};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTaskRequest {
    #[validate(length(min = 1, max = 255, message = "任务名称长度必须在1-255个字符之间"))]
    pub name: String,

    #[validate(length(min = 1, max = 100, message = "任务类型长度必须在1-100个字符之间"))]
    pub task_type: String,

    #[validate(length(min = 1, max = 255, message = "调度表达式长度必须在1-255个字符之间"))]
    pub schedule: String,

    pub parameters: serde_json::Value,

    #[validate(range(min = 1, max = 86400, message = "超时时间必须在1-86400秒之间"))]
    pub timeout_seconds: Option<i32>,

    #[validate(range(min = 0, max = 10, message = "重试次数必须在0-10次之间"))]
    pub max_retries: Option<i32>,

    pub dependencies: Option<Vec<i64>>,
}

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: i64,
    pub name: String,
    pub task_type: String,
    pub schedule: String,
    pub parameters: serde_json::Value,
    pub timeout_seconds: i32,
    pub max_retries: i32,
    pub status: TaskStatus,
    pub dependencies: Vec<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub recent_runs: Option<Vec<TaskRunResponse>>,
    pub execution_stats: Option<TaskExecutionStats>,
}

#[derive(Debug, Serialize)]
pub struct TaskRunResponse {
    pub id: i64,
    pub task_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: Option<String>,
    pub retry_count: i32,
    pub scheduled_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub result: Option<String>,
    pub error_message: Option<String>,
    pub execution_duration_ms: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        Self {
            id: task.id,
            name: task.name,
            task_type: task.task_type,
            schedule: task.schedule,
            parameters: task.parameters,
            timeout_seconds: task.timeout_seconds,
            max_retries: task.max_retries,
            status: task.status,
            dependencies: task.dependencies,
            created_at: task.created_at,
            updated_at: task.updated_at,
            recent_runs: None,
            execution_stats: None,
        }
    }
}

impl From<TaskRun> for TaskRunResponse {
    fn from(task_run: TaskRun) -> Self {
        let execution_duration_ms = task_run.execution_duration_ms();
        Self {
            id: task_run.id,
            task_id: task_run.task_id,
            status: task_run.status,
            worker_id: task_run.worker_id,
            retry_count: task_run.retry_count,
            scheduled_at: task_run.scheduled_at,
            started_at: task_run.started_at,
            completed_at: task_run.completed_at,
            result: task_run.result,
            error_message: task_run.error_message,
            execution_duration_ms,
            created_at: task_run.created_at,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTaskRequest {
    #[validate(length(min = 1, max = 255, message = "任务名称长度必须在1-255个字符之间"))]
    pub name: Option<String>,

    #[validate(length(min = 1, max = 255, message = "调度表达式长度必须在1-255个字符之间"))]
    pub schedule: Option<String>,

    pub parameters: Option<serde_json::Value>,

    #[validate(range(min = 1, max = 86400, message = "超时时间必须在1-86400秒之间"))]
    pub timeout_seconds: Option<i32>,

    #[validate(range(min = 0, max = 10, message = "重试次数必须在0-10次之间"))]
    pub max_retries: Option<i32>,

    pub dependencies: Option<Vec<i64>>,

    pub status: Option<scheduler_domain::entities::TaskStatus>,
}

update_request! {
    #[derive(Debug, Deserialize)]
    pub struct PatchTaskRequest {
        pub name: UpdateValue<String>,
        pub schedule: UpdateValue<String>,
        pub parameters: UpdateValue<serde_json::Value>,
        pub timeout_seconds: NumericUpdateValue<i32>,
        pub max_retries: NumericUpdateValue<i32>,
        pub dependencies: UpdateValue<Vec<i64>>,
        pub status: UpdateValue<scheduler_domain::entities::TaskStatus>,
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct TaskQueryParams {
    #[validate(length(max = 100, message = "任务类型长度不能超过100个字符"))]
    pub task_type: Option<String>,

    #[validate(length(max = 255, message = "任务名称长度不能超过255个字符"))]
    pub name: Option<String>,

    #[validate(range(min = 1, max = 1000, message = "页码必须在1-1000之间"))]
    pub page: Option<i64>,

    #[validate(range(min = 1, max = 100, message = "每页大小必须在1-100之间"))]
    pub page_size: Option<i64>,

    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TaskRunQueryParams {
    #[validate(range(min = 1, max = 1000, message = "页码必须在1-1000之间"))]
    pub page: Option<i64>,

    #[validate(range(min = 1, max = 100, message = "每页大小必须在1-100之间"))]
    pub page_size: Option<i64>,

    pub status: Option<String>,
}

pub async fn create_task(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Json(request): Json<CreateTaskRequest>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task creation
    current_user.require_permission(Permission::TaskWrite)?;
    tracing::debug!("Creating task with request: {:?}", request);

    // 输入验证
    if let Err(validation_errors) = request.validate() {
        return Err(ApiError::Validation(validation_errors));
    }

    // 自定义验证
    crate::validation::task::validate_task_name(&request.name)?;
    crate::validation::task::validate_task_type(&request.task_type)?;
    crate::validation::task::validate_cron_expression(&request.schedule)?;
    crate::validation::task::validate_task_parameters(&request.parameters)?;
    crate::validation::task::validate_dependencies(&request.dependencies)?;

    // 业务逻辑验证
    validate_create_task_request(&request, &state).await?;

    // 检查任务名称唯一性
    if (state.task_repo.get_by_name(&request.name).await?).is_some() {
        return Err(ApiError::Conflict(format!(
            "任务名称 '{}' 已存在",
            request.name
        )));
    }

    let mut task = Task::new(
        request.name,
        request.task_type,
        request.schedule,
        request.parameters,
    );

    if let Some(timeout) = request.timeout_seconds {
        task.timeout_seconds = timeout;
    }

    if let Some(retries) = request.max_retries {
        task.max_retries = retries;
    }

    if let Some(deps) = request.dependencies {
        task.dependencies = deps;
    }

    let created_task = match state.task_repo.create(&task).await {
        Ok(task) => task,
        Err(e) => {
            tracing::error!("Failed to create task: {:?}", e);
            return Err(e.into());
        }
    };

    let response = TaskResponse::from(created_task);

    tracing::debug!("Successfully created task: {:?}", response);
    Ok(created(response))
}

pub async fn list_tasks(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Query(params): Query<TaskQueryParams>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task reading
    current_user.require_permission(Permission::TaskRead)?;
    // 输入验证
    if let Err(validation_errors) = params.validate() {
        return Err(ApiError::Validation(validation_errors));
    }

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let mut filter = TaskFilter {
        limit: Some(page_size),
        offset: Some(offset),
        ..Default::default()
    };

    if let Some(status_str) = &params.status {
        crate::validation::task::validate_task_status(status_str)?;
        match status_str.to_uppercase().as_str() {
            "ACTIVE" => filter.status = Some(TaskStatus::Active),
            "INACTIVE" => filter.status = Some(TaskStatus::Inactive),
            _ => {} // 已经在validate_task_status中验证过了
        }
    }

    // Avoid cloning by using references to build filter
    if let Some(task_type) = &params.task_type {
        filter.task_type = Some(task_type.as_str().to_string());
    }

    if let Some(name) = &params.name {
        filter.name_pattern = Some(name.as_str().to_string());
    }

    let tasks = state.task_repo.list(&filter).await?;
    
    // Create total_filter efficiently by moving values instead of cloning
    let total_filter = TaskFilter {
        status: filter.status,
        task_type: filter.task_type,
        name_pattern: filter.name_pattern,
        ..Default::default()
    };
    let total_tasks = state.task_repo.list(&total_filter).await?;
    let total = total_tasks.len() as i64;

    let task_responses: Vec<TaskResponse> = tasks.into_iter().map(TaskResponse::from).collect();

    let paginated_response = PaginatedResponse::new(task_responses, total, page, page_size);

    Ok(success(paginated_response))
}

pub async fn get_task(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task reading
    current_user.require_permission(Permission::TaskRead)?;
    tracing::debug!("Getting task with id: {}", id);

    // 验证ID格式
    if id <= 0 {
        return Err(ApiError::BadRequest("无效的任务ID".to_string()));
    }

    let task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    tracing::debug!("Found task: {:?}", task.name);

    let recent_runs = state
        .task_run_repo
        .get_recent_runs(id, 10)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to get recent runs for task {}: {:?}", id, e);
            Vec::new()
        });

    let recent_run_responses: Vec<TaskRunResponse> =
        recent_runs.into_iter().map(TaskRunResponse::from).collect();

    let execution_stats = state
        .task_run_repo
        .get_execution_stats(id, 30)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to get execution stats for task {}: {:?}", id, e);
            scheduler_domain::repositories::TaskExecutionStats {
                task_id: id,
                total_runs: 0,
                successful_runs: 0,
                failed_runs: 0,
                timeout_runs: 0,
                average_execution_time_ms: None,
                success_rate: 0.0,
                last_execution: None,
            }
        });

    let mut response = TaskResponse::from(task);
    response.recent_runs = Some(recent_run_responses);
    response.execution_stats = Some(execution_stats);

    Ok(success(response))
}

pub async fn update_task(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(request): Json<UpdateTaskRequest>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task updating
    current_user.require_permission(Permission::TaskWrite)?;
    // 验证ID格式
    if id <= 0 {
        return Err(ApiError::BadRequest("无效的任务ID".to_string()));
    }

    // 输入验证
    if let Err(validation_errors) = request.validate() {
        return Err(ApiError::Validation(validation_errors));
    }

    // 自定义验证
    if let Some(ref name) = request.name {
        crate::validation::task::validate_task_name(name)?;
    }
    if let Some(ref schedule) = request.schedule {
        crate::validation::task::validate_cron_expression(schedule)?;
    }
    if let Some(ref parameters) = request.parameters {
        crate::validation::task::validate_task_parameters(parameters)?;
    }
    crate::validation::task::validate_dependencies(&request.dependencies)?;

    // 业务逻辑验证
    validate_update_task_request(&request, id, &state).await?;

    let mut task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    if let Some(name) = request.name {
        if let Some(existing_task) = state.task_repo.get_by_name(&name).await? {
            if existing_task.id != id {
                return Err(ApiError::Conflict(format!("任务名称 '{name}' 已存在")));
            }
        }
        task.name = name;
    }

    if let Some(schedule) = request.schedule {
        task.schedule = schedule;
    }

    if let Some(parameters) = request.parameters {
        task.parameters = parameters;
    }

    if let Some(timeout) = request.timeout_seconds {
        task.timeout_seconds = timeout;
    }

    if let Some(retries) = request.max_retries {
        task.max_retries = retries;
    }

    if let Some(deps) = request.dependencies {
        task.dependencies = deps;
    }

    if let Some(status) = request.status {
        task.status = status;
    }

    task.updated_at = chrono::Utc::now();
    state.task_repo.update(&task).await?;

    let response = TaskResponse::from(task);
    Ok(success(response))
}

pub async fn patch_task(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(request): Json<PatchTaskRequest>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task updating
    current_user.require_permission(Permission::TaskWrite)?;

    // Validate ID format
    if id <= 0 {
        return Err(ApiError::BadRequest("无效的任务ID".to_string()));
    }

    // Check if the request has any actual changes
    if !request.has_changes() {
        return Err(ApiError::BadRequest(
            "PATCH请求必须包含至少一个要修改的字段".to_string(),
        ));
    }

    // Manual validation for PATCH request
    validate_patch_request_fields(&request)?;

    // Custom validation for provided fields
    if let Some(Some(name)) = request.name.as_option() {
        crate::validation::task::validate_task_name(name)?;
    }
    if let Some(Some(schedule)) = request.schedule.as_option() {
        crate::validation::task::validate_cron_expression(schedule)?;
    }
    if let Some(Some(parameters)) = request.parameters.as_option() {
        crate::validation::task::validate_task_parameters(parameters)?;
    }
    if let Some(Some(deps)) = request.dependencies.as_option() {
        crate::validation::task::validate_dependencies(&Some(deps.to_vec()))?;
    }

    // Business logic validation
    validate_patch_task_request(&request, id, &state).await?;

    let mut task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Apply PATCH updates
    apply_patch_updates(&mut task, &request, &state).await?;

    task.updated_at = chrono::Utc::now();
    state.task_repo.update(&task).await?;

    let response = TaskResponse::from(task);
    Ok(success(response))
}

/// Apply PATCH updates to a task
async fn apply_patch_updates(
    task: &mut Task,
    request: &PatchTaskRequest,
    state: &AppState,
) -> ApiResult<()> {
    // Update name if provided
    if let Some(Some(name)) = request.name.as_option() {
        // Check name uniqueness
        if let Some(existing_task) = state.task_repo.get_by_name(name).await? {
            if existing_task.id != task.id {
                return Err(ApiError::Conflict(format!("任务名称 '{name}' 已存在")));
            }
        }
        task.name = name.to_string();
    }

    // Update schedule if provided
    if let Some(Some(schedule)) = request.schedule.as_option() {
        task.schedule = schedule.to_string();
    }

    // Update parameters if provided
    if let Some(parameters_opt) = request.parameters.as_option() {
        task.parameters = parameters_opt.cloned().unwrap_or(serde_json::Value::Null);
    }

    // Update timeout_seconds if provided
    if let Some(timeout_opt) = request.timeout_seconds.as_option() {
        task.timeout_seconds = *timeout_opt.unwrap_or(&task.timeout_seconds);
    }

    // Update max_retries if provided
    if let Some(retries_opt) = request.max_retries.as_option() {
        task.max_retries = *retries_opt.unwrap_or(&task.max_retries);
    }

    // Update dependencies if provided
    if let Some(Some(deps)) = request.dependencies.as_option() {
        task.dependencies = deps.to_vec();
    }

    // Update status if provided
    if let Some(Some(status)) = request.status.as_option() {
        task.status = *status;
    }

    Ok(())
}

/// Validate PATCH task request fields
fn validate_patch_request_fields(request: &PatchTaskRequest) -> ApiResult<()> {
    // Validate name field if provided
    if let Some(Some(name)) = request.name.as_option() {
        if name.is_empty() || name.len() > 255 {
            return Err(ApiError::BadRequest(
                "任务名称长度必须在1-255个字符之间".to_string(),
            ));
        }
    }

    // Validate schedule field if provided
    if let Some(Some(schedule)) = request.schedule.as_option() {
        if schedule.is_empty() || schedule.len() > 255 {
            return Err(ApiError::BadRequest(
                "调度表达式长度必须在1-255个字符之间".to_string(),
            ));
        }
    }

    // Validate timeout_seconds field if provided
    if let Some(Some(timeout)) = request.timeout_seconds.as_option() {
        if *timeout < 1 || *timeout > 86400 {
            return Err(ApiError::BadRequest(
                "超时时间必须在1-86400秒之间".to_string(),
            ));
        }
    }

    // Validate max_retries field if provided
    if let Some(Some(retries)) = request.max_retries.as_option() {
        if *retries < 0 || *retries > 10 {
            return Err(ApiError::BadRequest("重试次数必须在0-10次之间".to_string()));
        }
    }

    Ok(())
}

/// Validate PATCH task request business logic
async fn validate_patch_task_request(
    request: &PatchTaskRequest,
    task_id: i64,
    state: &AppState,
) -> ApiResult<()> {
    // Similar validation logic as update_task_request but adapted for PATCH

    // Check dependencies if they're being updated
    if let Some(Some(deps)) = request.dependencies.as_option() {
        // Check if dependency tasks exist
        for &dep_id in deps {
            if dep_id == task_id {
                return Err(ApiError::BadRequest("任务不能依赖自己".to_string()));
            }

            if state.task_repo.get_by_id(dep_id).await?.is_none() {
                return Err(ApiError::BadRequest(format!(
                    "依赖的任务ID {dep_id} 不存在"
                )));
            }
        }
    }

    // Check if task exists (already done in the main function)

    Ok(())
}

pub async fn delete_task(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task deletion
    current_user.require_permission(Permission::TaskDelete)?;
    // 验证ID格式
    if id <= 0 {
        return Err(ApiError::BadRequest("无效的任务ID".to_string()));
    }

    let task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // 检查是否有其他任务依赖此任务
    let all_tasks = state.task_repo.list(&TaskFilter::default()).await?;
    for other_task in all_tasks {
        if other_task.dependencies.contains(&id) {
            return Err(ApiError::Conflict(format!(
                "无法删除任务，任务 '{}' 依赖于此任务",
                other_task.name
            )));
        }
    }

    // 检查是否有正在运行的任务实例
    let running_runs = state.task_run_repo.get_by_task_id(id).await?;
    let has_running = running_runs.iter().any(|run| run.is_running());
    if has_running {
        return Err(ApiError::Conflict(
            "无法删除任务，存在正在运行的任务实例".to_string(),
        ));
    }

    // 软删除：设置为非活跃状态
    let mut task_to_update = task;
    task_to_update.status = TaskStatus::Inactive;
    task_to_update.updated_at = chrono::Utc::now();
    state.task_repo.update(&task_to_update).await?;

    Ok(success(serde_json::json!({
        "message": "任务已成功删除（设置为非活跃状态）",
        "task_id": id
    })))
}

pub async fn trigger_task(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> ApiResult<impl axum::response::IntoResponse> {
    use scheduler_observability::CrossComponentTracer;

    let span = CrossComponentTracer::instrument_service_call(
        "api",
        "trigger_task",
        vec![
            ("task.id".to_string(), id.to_string()),
            ("user.id".to_string(), current_user.user_id.to_string()),
        ],
    );
    let _guard = span.enter();

    // Check permissions for task triggering
    current_user.require_permission(Permission::TaskWrite)?;
    // 验证ID格式
    if id <= 0 {
        return Err(ApiError::BadRequest("无效的任务ID".to_string()));
    }

    let task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    if !task.is_active() {
        return Err(ApiError::BadRequest("只能触发活跃状态的任务".to_string()));
    }

    if !state.task_repo.check_dependencies(id).await? {
        return Err(ApiError::BadRequest(
            "任务依赖未满足，无法触发执行".to_string(),
        ));
    }

    let task_run = state.task_controller.trigger_task(id).await?;
    let response = TaskRunResponse::from(task_run);

    Ok(success(response))
}

pub async fn get_task_runs(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Path(task_id): Path<i64>,
    Query(params): Query<TaskRunQueryParams>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task run reading
    current_user.require_permission(Permission::TaskRead)?;
    // 验证任务ID
    if task_id <= 0 {
        return Err(ApiError::BadRequest("无效的任务ID".to_string()));
    }

    // 验证任务存在
    state
        .task_repo
        .get_by_id(task_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // 验证查询参数
    if let Err(validation_errors) = params.validate() {
        return Err(ApiError::Validation(validation_errors));
    }

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);

    let mut all_runs = state.task_run_repo.get_by_task_id(task_id).await?;

    if let Some(status_str) = &params.status {
        crate::validation::task::validate_task_run_status(status_str)?;
        let filter_status = match status_str.to_uppercase().as_str() {
            "PENDING" => TaskRunStatus::Pending,
            "DISPATCHED" => TaskRunStatus::Dispatched,
            "RUNNING" => TaskRunStatus::Running,
            "COMPLETED" => TaskRunStatus::Completed,
            "FAILED" => TaskRunStatus::Failed,
            "TIMEOUT" => TaskRunStatus::Timeout,
            _ => TaskRunStatus::Pending, // 默认值，实际上不会执行到这里
        };
        all_runs.retain(|run| run.status == filter_status);
    }

    let total = all_runs.len() as i64;
    let start = ((page - 1) * page_size) as usize;
    let end = (start + page_size as usize).min(all_runs.len());
    let paginated_runs = if start < all_runs.len() {
        all_runs[start..end].to_vec()
    } else {
        vec![]
    };

    let run_responses: Vec<TaskRunResponse> = paginated_runs
        .into_iter()
        .map(TaskRunResponse::from)
        .collect();

    let paginated_response = PaginatedResponse::new(run_responses, total, page, page_size);

    Ok(success(paginated_response))
}

pub async fn get_task_run(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task run reading
    current_user.require_permission(Permission::TaskRead)?;
    // 验证ID格式
    if id <= 0 {
        return Err(ApiError::BadRequest("无效的任务运行实例ID".to_string()));
    }

    let task_run = state
        .task_run_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let response = TaskRunResponse::from(task_run);
    Ok(success(response))
}

pub async fn get_task_execution_stats(
    State(state): State<AppState>,
    current_user: AuthenticatedUser,
    Path(id): Path<i64>,
    Query(params): Query<TaskStatsQueryParams>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // Check permissions for task stats reading
    current_user.require_permission(Permission::TaskRead)?;
    // 验证任务ID
    if id <= 0 {
        return Err(ApiError::BadRequest("无效的任务ID".to_string()));
    }

    // 验证任务存在
    let _task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // 验证查询参数
    if let Err(validation_errors) = params.validate() {
        return Err(ApiError::Validation(validation_errors));
    }

    let days = params.days.unwrap_or(30).clamp(1, 365);
    let stats = state.task_run_repo.get_execution_stats(id, days).await?;

    Ok(success(stats))
}

#[derive(Debug, Deserialize, Validate)]
pub struct TaskStatsQueryParams {
    #[validate(range(min = 1, max = 365, message = "统计天数必须在1-365天之间"))]
    pub days: Option<i32>,
}
