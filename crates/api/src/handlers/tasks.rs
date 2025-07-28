use axum::{
    extract::{Path, Query, State},
    Json,
};
use scheduler_core::{
    models::{Task, TaskFilter, TaskRun, TaskRunStatus, TaskStatus},
    traits::TaskExecutionStats,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{
    error::{ApiError, ApiResult},
    response::{created, success, PaginatedResponse},
    routes::AppState,
};

/// 任务创建请求
#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub task_type: String,
    pub schedule: String,
    pub parameters: serde_json::Value,
    pub timeout_seconds: Option<i32>,
    pub max_retries: Option<i32>,
    pub dependencies: Option<Vec<i64>>,
}

/// 任务响应
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

/// 任务执行记录响应
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

/// 任务更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub timeout_seconds: Option<i32>,
    pub max_retries: Option<i32>,
    pub dependencies: Option<Vec<i64>>,
    pub status: Option<scheduler_core::models::task::TaskStatus>,
}

/// 任务查询参数
#[derive(Debug, Deserialize)]
pub struct TaskQueryParams {
    pub status: Option<String>,
    pub task_type: Option<String>,
    pub name: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// 任务运行查询参数
#[derive(Debug, Deserialize)]
pub struct TaskRunQueryParams {
    pub status: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// 创建任务
pub async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 验证输入参数
    if request.name.trim().is_empty() {
        return Err(ApiError::BadRequest("任务名称不能为空".to_string()));
    }

    if request.task_type.trim().is_empty() {
        return Err(ApiError::BadRequest("任务类型不能为空".to_string()));
    }

    if request.schedule.trim().is_empty() {
        return Err(ApiError::BadRequest("调度表达式不能为空".to_string()));
    }

    // 验证CRON表达式
    if let Err(e) = cron::Schedule::from_str(&request.schedule) {
        return Err(ApiError::BadRequest(format!("无效的CRON表达式: {e}")));
    }

    // 检查任务名称是否已存在
    if (state.task_repo.get_by_name(&request.name).await?).is_some() {
        return Err(ApiError::Conflict(format!(
            "任务名称 '{}' 已存在",
            request.name
        )));
    }

    // 创建任务对象
    let mut task = Task::new(
        request.name,
        request.task_type,
        request.schedule,
        request.parameters,
    );

    if let Some(timeout) = request.timeout_seconds {
        if timeout <= 0 {
            return Err(ApiError::BadRequest("超时时间必须大于0".to_string()));
        }
        task.timeout_seconds = timeout;
    }

    if let Some(retries) = request.max_retries {
        if retries < 0 {
            return Err(ApiError::BadRequest("重试次数不能为负数".to_string()));
        }
        task.max_retries = retries;
    }

    if let Some(deps) = request.dependencies {
        // 验证依赖任务是否存在
        for dep_id in &deps {
            if state.task_repo.get_by_id(*dep_id).await?.is_none() {
                return Err(ApiError::BadRequest(format!("依赖任务 {dep_id} 不存在")));
            }
        }
        task.dependencies = deps;
    }

    // 保存任务到数据库
    let created_task = state.task_repo.create(&task).await?;
    let response = TaskResponse::from(created_task);

    Ok(created(response))
}

/// 获取任务列表
pub async fn list_tasks(
    State(state): State<AppState>,
    Query(params): Query<TaskQueryParams>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 设置默认分页参数
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    // 构建过滤条件
    let mut filter = TaskFilter {
        limit: Some(page_size),
        offset: Some(offset),
        ..Default::default()
    };

    if let Some(status_str) = &params.status {
        match status_str.to_uppercase().as_str() {
            "ACTIVE" => filter.status = Some(TaskStatus::Active),
            "INACTIVE" => filter.status = Some(TaskStatus::Inactive),
            _ => {
                return Err(ApiError::BadRequest(format!(
                    "无效的任务状态: {status_str}"
                )));
            }
        }
    }

    if let Some(task_type) = &params.task_type {
        filter.task_type = Some(task_type.clone());
    }

    if let Some(name) = &params.name {
        filter.name_pattern = Some(name.clone());
    }

    // 查询任务列表
    let tasks = state.task_repo.list(&filter).await?;

    // 获取总数（为了分页信息）
    let total_filter = TaskFilter {
        status: filter.status,
        task_type: filter.task_type.clone(),
        name_pattern: filter.name_pattern.clone(),
        ..Default::default()
    };
    let total_tasks = state.task_repo.list(&total_filter).await?;
    let total = total_tasks.len() as i64;

    // 转换为响应格式
    let task_responses: Vec<TaskResponse> = tasks.into_iter().map(TaskResponse::from).collect();

    let paginated_response = PaginatedResponse::new(task_responses, total, page, page_size);

    Ok(success(paginated_response))
}

/// 获取单个任务
pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 获取任务基本信息
    let task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // 获取最近的执行记录
    let recent_runs = state.task_run_repo.get_recent_runs(id, 10).await?;
    let recent_run_responses: Vec<TaskRunResponse> =
        recent_runs.into_iter().map(TaskRunResponse::from).collect();

    // 获取执行统计信息
    let execution_stats = state.task_run_repo.get_execution_stats(id, 30).await?;

    // 构建响应
    let mut response = TaskResponse::from(task);
    response.recent_runs = Some(recent_run_responses);
    response.execution_stats = Some(execution_stats);

    Ok(success(response))
}

/// 更新任务
pub async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateTaskRequest>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 获取现有任务
    let mut task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // 更新字段
    if let Some(name) = request.name {
        if name.trim().is_empty() {
            return Err(ApiError::BadRequest("任务名称不能为空".to_string()));
        }
        // 检查名称是否与其他任务冲突
        if let Some(existing_task) = state.task_repo.get_by_name(&name).await? {
            if existing_task.id != id {
                return Err(ApiError::Conflict(format!("任务名称 '{name}' 已存在")));
            }
        }
        task.name = name;
    }

    if let Some(schedule) = request.schedule {
        if schedule.trim().is_empty() {
            return Err(ApiError::BadRequest("调度表达式不能为空".to_string()));
        }
        // 验证CRON表达式
        if let Err(e) = cron::Schedule::from_str(&schedule) {
            return Err(ApiError::BadRequest(format!("无效的CRON表达式: {e}")));
        }
        task.schedule = schedule;
    }

    if let Some(parameters) = request.parameters {
        task.parameters = parameters;
    }

    if let Some(timeout) = request.timeout_seconds {
        if timeout <= 0 {
            return Err(ApiError::BadRequest("超时时间必须大于0".to_string()));
        }
        task.timeout_seconds = timeout;
    }

    if let Some(retries) = request.max_retries {
        if retries < 0 {
            return Err(ApiError::BadRequest("重试次数不能为负数".to_string()));
        }
        task.max_retries = retries;
    }

    if let Some(deps) = request.dependencies {
        // 验证依赖任务是否存在
        for dep_id in &deps {
            if *dep_id == id {
                return Err(ApiError::BadRequest("任务不能依赖自己".to_string()));
            }
            if state.task_repo.get_by_id(*dep_id).await?.is_none() {
                return Err(ApiError::BadRequest(format!("依赖任务 {dep_id} 不存在")));
            }
        }
        task.dependencies = deps;
    }

    if let Some(status) = request.status {
        task.status = status;
    }

    // 更新时间戳
    task.updated_at = chrono::Utc::now();

    // 保存更新
    state.task_repo.update(&task).await?;

    let response = TaskResponse::from(task);
    Ok(success(response))
}

/// 删除任务
pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 检查任务是否存在
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

    // 执行软删除（设置为INACTIVE状态）而不是硬删除
    let mut task_to_update = task;
    task_to_update.status = TaskStatus::Inactive;
    task_to_update.updated_at = chrono::Utc::now();
    state.task_repo.update(&task_to_update).await?;

    Ok(success(serde_json::json!({
        "message": "任务已成功删除（设置为非活跃状态）",
        "task_id": id
    })))
}

/// 触发任务执行
pub async fn trigger_task(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 检查任务是否存在且为活跃状态
    let task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    if !task.is_active() {
        return Err(ApiError::BadRequest("只能触发活跃状态的任务".to_string()));
    }

    // 检查依赖是否满足
    if !state.task_repo.check_dependencies(id).await? {
        return Err(ApiError::BadRequest(
            "任务依赖未满足，无法触发执行".to_string(),
        ));
    }

    // 使用任务控制服务触发任务
    let task_run = state.task_controller.trigger_task(id).await?;
    let response = TaskRunResponse::from(task_run);

    Ok(success(response))
}

/// 获取任务执行历史
pub async fn get_task_runs(
    State(state): State<AppState>,
    Path(task_id): Path<i64>,
    Query(params): Query<TaskRunQueryParams>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 检查任务是否存在
    state
        .task_repo
        .get_by_id(task_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // 设置默认分页参数
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);

    // 获取任务执行记录
    let mut all_runs = state.task_run_repo.get_by_task_id(task_id).await?;

    // 按状态过滤
    if let Some(status_str) = &params.status {
        let filter_status = match status_str.to_uppercase().as_str() {
            "PENDING" => TaskRunStatus::Pending,
            "DISPATCHED" => TaskRunStatus::Dispatched,
            "RUNNING" => TaskRunStatus::Running,
            "COMPLETED" => TaskRunStatus::Completed,
            "FAILED" => TaskRunStatus::Failed,
            "TIMEOUT" => TaskRunStatus::Timeout,
            _ => {
                return Err(ApiError::BadRequest(format!(
                    "无效的任务执行状态: {status_str}"
                )));
            }
        };
        all_runs.retain(|run| run.status == filter_status);
    }

    let total = all_runs.len() as i64;

    // 分页
    let start = ((page - 1) * page_size) as usize;
    let end = (start + page_size as usize).min(all_runs.len());
    let paginated_runs = if start < all_runs.len() {
        all_runs[start..end].to_vec()
    } else {
        vec![]
    };

    // 转换为响应格式
    let run_responses: Vec<TaskRunResponse> = paginated_runs
        .into_iter()
        .map(TaskRunResponse::from)
        .collect();

    let paginated_response = PaginatedResponse::new(run_responses, total, page, page_size);

    Ok(success(paginated_response))
}

/// 获取单个任务执行记录
pub async fn get_task_run(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<impl axum::response::IntoResponse> {
    let task_run = state
        .task_run_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let response = TaskRunResponse::from(task_run);
    Ok(success(response))
}

/// 获取任务执行统计信息
pub async fn get_task_execution_stats(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<TaskStatsQueryParams>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 首先检查任务是否存在
    let _task = state
        .task_repo
        .get_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let days = params.days.unwrap_or(30).clamp(1, 365);
    let stats = state.task_run_repo.get_execution_stats(id, days).await?;

    Ok(success(stats))
}

/// 任务统计查询参数
#[derive(Debug, Deserialize)]
pub struct TaskStatsQueryParams {
    pub days: Option<i32>,
}
