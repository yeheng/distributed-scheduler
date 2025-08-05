//! # Worker管理API处理器
//!
//! 提供Worker节点管理的REST API端点实现，包括Worker列表查询、
//! 详细信息获取和统计数据查看等功能。
//!
//! ## API端点概览
//!
//! - `GET /workers` - 获取Worker列表（支持分页和状态过滤）
//! - `GET /workers/{id}` - 获取单个Worker详细信息
//! - `GET /workers/{id}/stats` - 获取Worker统计数据
//!
//! ## 设计原则
//!
//! - **RESTful设计**: 遵循REST API设计规范
//! - **分页支持**: 大量数据时支持分页查询
//! - **状态过滤**: 支持按Worker状态过滤
//! - **错误处理**: 统一的错误响应格式
//! - **性能优化**: 合理的缓存和查询优化
//!
//! ## 使用示例
//!
//! ### 获取所有活跃Worker
//! ```bash
//! curl "http://localhost:8080/api/v1/workers?status=ALIVE&page=1&page_size=20"
//! ```
//!
//! ### 获取特定Worker信息
//! ```bash
//! curl "http://localhost:8080/api/v1/workers/worker-001"
//! ```
//!
//! ### 获取Worker统计数据
//! ```bash
//! curl "http://localhost:8080/api/v1/workers/worker-001/stats"
//! ```

use axum::extract::{Path, Query, State};
use scheduler_core::models::{WorkerInfo, WorkerStatus};
use serde::{Deserialize, Serialize};

use crate::{
    error::ApiResult,
    response::{success, PaginatedResponse},
    routes::AppState,
};

/// Worker查询参数结构体
///
/// 定义Worker列表查询API的查询参数，支持状态过滤和分页控制。
/// 所有参数都是可选的，提供合理的默认值。
///
/// # 字段说明
///
/// - `status`: Worker状态过滤，支持 "ALIVE", "DOWN" 等
/// - `page`: 页码，从1开始，默认为1
/// - `page_size`: 每页大小，范围1-100，默认为20
///
/// # 使用示例
///
/// ```rust
/// // URL: /workers?status=ALIVE&page=2&page_size=10
/// let params = WorkerQueryParams {
///     status: Some("ALIVE".to_string()),
///     page: Some(2),
///     page_size: Some(10),
/// };
/// ```
///
/// # 验证规则
///
/// - `page`: 最小值为1，无效值会被调整为1
/// - `page_size`: 范围限制在1-100之间
/// - `status`: 不区分大小写，支持的值包括：
///   - "ALIVE": 活跃的Worker（有心跳）
///   - "DOWN": 离线的Worker
///   - 其他值或空值: 返回所有Worker
#[derive(Debug, Deserialize)]
pub struct WorkerQueryParams {
    /// Worker状态过滤条件
    ///
    /// 可选参数，用于按状态过滤Worker列表。
    /// 支持的状态值：
    /// - "ALIVE": 返回有心跳的活跃Worker
    /// - "DOWN": 返回离线的Worker
    /// - 其他值或null: 返回所有Worker
    pub status: Option<String>,

    /// 页码
    ///
    /// 分页查询的页码，从1开始。
    /// 默认值为1，最小值为1。
    pub page: Option<i64>,

    /// 每页大小
    ///
    /// 每页返回的Worker数量。
    /// 默认值为20，范围限制在1-100之间。
    pub page_size: Option<i64>,
}

/// Worker详细信息API响应结构体
///
/// 定义Worker详细信息的API响应格式，包含Worker的完整状态和配置信息。
/// 这个结构体专门为API响应设计，包含了前端需要的所有信息。
///
/// # 设计考虑
///
/// - **API友好**: 字段名称和类型适合JSON序列化
/// - **计算字段**: 包含负载百分比等计算得出的字段
/// - **时间格式**: 使用ISO 8601格式的UTC时间
/// - **完整信息**: 包含监控和管理所需的所有数据
///
/// # 字段说明
///
/// ## 标识信息
/// - `id`: Worker唯一标识符
/// - `hostname`: 主机名
/// - `ip_address`: IP地址
///
/// ## 能力信息
/// - `supported_task_types`: 支持的任务类型列表
/// - `max_concurrent_tasks`: 最大并发任务数
///
/// ## 状态信息
/// - `current_task_count`: 当前任务数量
/// - `status`: Worker状态枚举
/// - `load_percentage`: 负载百分比（0-100）
///
/// ## 时间信息
/// - `last_heartbeat`: 最后心跳时间
/// - `registered_at`: 注册时间
///
/// # JSON响应示例
///
/// ```json
/// {
///   "id": "worker-001",
///   "hostname": "compute-node-1",
///   "ip_address": "192.168.1.100",
///   "supported_task_types": ["python", "shell", "docker"],
///   "max_concurrent_tasks": 10,
///   "current_task_count": 3,
///   "status": "Available",
///   "load_percentage": 30.0,
///   "last_heartbeat": "2024-01-15T10:30:00Z",
///   "registered_at": "2024-01-15T09:00:00Z"
/// }
/// ```
///
/// # 使用场景
///
/// - Worker管理界面显示
/// - 监控仪表板数据
/// - 负载均衡决策参考
/// - 系统健康检查报告
#[derive(Debug, Serialize)]
pub struct WorkerDetailResponse {
    /// Worker唯一标识符
    ///
    /// 全局唯一的Worker ID，用于标识和引用特定的Worker节点。
    pub id: String,

    /// 主机名
    ///
    /// Worker所在服务器的主机名，便于运维人员识别物理位置。
    pub hostname: String,

    /// IP地址
    ///
    /// Worker的网络地址，用于网络通信和故障排查。
    pub ip_address: String,

    /// 支持的任务类型列表
    ///
    /// 该Worker能够执行的任务类型，用于任务分配时的匹配。
    /// 例如：["python", "shell", "docker"]
    pub supported_task_types: Vec<String>,

    /// 最大并发任务数
    ///
    /// Worker配置的最大同时执行任务数量，用于负载控制。
    pub max_concurrent_tasks: i32,

    /// 当前任务数量
    ///
    /// Worker当前正在执行的任务数量，实时更新。
    pub current_task_count: i32,

    /// Worker状态
    ///
    /// 当前Worker的运行状态，影响任务分配决策。
    pub status: WorkerStatus,

    /// 负载百分比
    ///
    /// 当前负载占最大容量的百分比，范围0-100。
    /// 计算公式：(current_task_count / max_concurrent_tasks) * 100
    pub load_percentage: f64,

    /// 最后心跳时间
    ///
    /// Worker最后一次发送心跳的UTC时间，用于健康检查。
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,

    /// 注册时间
    ///
    /// Worker首次注册到系统的UTC时间。
    pub registered_at: chrono::DateTime<chrono::Utc>,
}

impl From<WorkerInfo> for WorkerDetailResponse {
    /// 从WorkerInfo转换为API响应格式
    ///
    /// 将内部的WorkerInfo模型转换为适合API响应的格式。
    /// 这个转换过程包括计算派生字段和格式化数据。
    ///
    /// # 转换逻辑
    ///
    /// 1. **直接映射**: 大部分字段直接从WorkerInfo复制
    /// 2. **计算字段**: 计算负载百分比等派生数据
    /// 3. **格式转换**: 确保数据格式适合JSON序列化
    ///
    /// # 参数
    ///
    /// * `worker` - 要转换的WorkerInfo实例
    ///
    /// # 返回值
    ///
    /// 返回转换后的WorkerDetailResponse实例。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let worker_info = WorkerInfo {
    ///     id: "worker-001".to_string(),
    ///     current_task_count: 3,
    ///     max_concurrent_tasks: 10,
    ///     // ... 其他字段
    /// };
    ///
    /// let response = WorkerDetailResponse::from(worker_info);
    /// assert_eq!(response.load_percentage, 30.0);
    /// ```
    fn from(worker: WorkerInfo) -> Self {
        let load_percentage = worker.load_percentage();
        Self {
            id: worker.id,
            hostname: worker.hostname,
            ip_address: worker.ip_address,
            supported_task_types: worker.supported_task_types,
            max_concurrent_tasks: worker.max_concurrent_tasks,
            current_task_count: worker.current_task_count,
            status: worker.status,
            load_percentage,
            last_heartbeat: worker.last_heartbeat,
            registered_at: worker.registered_at,
        }
    }
}

/// 获取Worker列表API处理器
///
/// 处理GET /workers请求，返回系统中Worker节点的分页列表。
/// 支持按状态过滤和分页查询，适用于管理界面和监控系统。
///
/// # 请求参数
///
/// ## 查询参数（可选）
/// - `status`: Worker状态过滤，支持 "ALIVE", "DOWN"
/// - `page`: 页码，从1开始，默认为1
/// - `page_size`: 每页大小，范围1-100，默认为20
///
/// # 响应格式
///
/// 返回分页的Worker列表，包含总数、当前页等分页信息。
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "items": [
///       {
///         "id": "worker-001",
///         "hostname": "compute-node-1",
///         "status": "Available",
///         "load_percentage": 30.0,
///         // ... 其他字段
///       }
///     ],
///     "total": 50,
///     "page": 1,
///     "page_size": 20,
///     "total_pages": 3
///   }
/// }
/// ```
///
/// # 错误响应
///
/// - `500 Internal Server Error`: 数据库查询失败
/// - `400 Bad Request`: 参数验证失败
///
/// # 实现逻辑
///
/// 1. **参数验证**: 验证和标准化查询参数
/// 2. **状态过滤**: 根据status参数过滤Worker
/// 3. **分页处理**: 计算分页偏移量和限制
/// 4. **数据转换**: 将内部模型转换为API响应格式
/// 5. **响应构建**: 构建标准化的分页响应
///
/// # 性能考虑
///
/// - 使用数据库索引优化状态查询
/// - 限制最大页面大小防止过载
/// - 考虑缓存热点数据
///
/// # 使用示例
///
/// ```bash
/// # 获取所有活跃Worker的第一页
/// curl "http://localhost:8080/api/v1/workers?status=ALIVE&page=1&page_size=10"
///
/// # 获取所有Worker（默认分页）
/// curl "http://localhost:8080/api/v1/workers"
/// ```
pub async fn list_workers(
    State(state): State<AppState>,
    Query(params): Query<WorkerQueryParams>,
) -> ApiResult<impl axum::response::IntoResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);

    // 获取所有workers
    let mut workers = if let Some(status) = params.status {
        match status.to_uppercase().as_str() {
            "ALIVE" => state.worker_repo.get_alive_workers().await?,
            "DOWN" => {
                let all_workers = state.worker_repo.list().await?;
                all_workers
                    .into_iter()
                    .filter(|w| matches!(w.status, WorkerStatus::Down))
                    .collect()
            }
            _ => state.worker_repo.list().await?,
        }
    } else {
        state.worker_repo.list().await?
    };

    let total = workers.len() as i64;

    // 分页处理
    let start = ((page - 1) * page_size) as usize;
    let _end = (start + page_size as usize).min(workers.len());

    if start < workers.len() {
        workers = workers
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();
    } else {
        workers = Vec::new();
    }

    let worker_responses: Vec<WorkerDetailResponse> = workers
        .into_iter()
        .map(WorkerDetailResponse::from)
        .collect();

    let paginated_response = PaginatedResponse::new(worker_responses, total, page, page_size);
    Ok(success(paginated_response))
}

/// 获取单个Worker详细信息API处理器
///
/// 处理GET /workers/{id}请求，返回指定Worker的详细信息。
/// 用于查看特定Worker的完整状态和配置。
///
/// # 路径参数
///
/// - `id`: Worker的唯一标识符
///
/// # 响应格式
///
/// ## 成功响应 (200 OK)
/// ```json
/// {
///   "success": true,
///   "data": {
///     "id": "worker-001",
///     "hostname": "compute-node-1",
///     "ip_address": "192.168.1.100",
///     "supported_task_types": ["python", "shell"],
///     "max_concurrent_tasks": 10,
///     "current_task_count": 3,
///     "status": "Available",
///     "load_percentage": 30.0,
///     "last_heartbeat": "2024-01-15T10:30:00Z",
///     "registered_at": "2024-01-15T09:00:00Z"
///   }
/// }
/// ```
///
/// ## 错误响应
/// - `404 Not Found`: Worker不存在
/// - `500 Internal Server Error`: 数据库查询失败
///
/// # 实现逻辑
///
/// 1. **参数提取**: 从URL路径提取Worker ID
/// 2. **数据查询**: 通过仓储查询Worker信息
/// 3. **存在性检查**: 验证Worker是否存在
/// 4. **数据转换**: 转换为API响应格式
/// 5. **响应返回**: 返回成功或404错误
///
/// # 使用场景
///
/// - Worker详情页面显示
/// - 监控系统获取特定Worker状态
/// - 故障排查和诊断
/// - 配置验证和审计
///
/// # 示例请求
///
/// ```bash
/// # 获取特定Worker信息
/// curl "http://localhost:8080/api/v1/workers/worker-001"
///
/// # 使用curl检查响应状态
/// curl -I "http://localhost:8080/api/v1/workers/nonexistent"
/// # 返回: HTTP/1.1 404 Not Found
/// ```
///
/// # 安全考虑
///
/// - Worker ID应该进行输入验证
/// - 考虑访问权限控制
/// - 记录访问日志用于审计
pub async fn get_worker(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<impl axum::response::IntoResponse> {
    match state.worker_repo.get_by_id(&id).await? {
        Some(worker) => {
            let response = WorkerDetailResponse::from(worker);
            Ok(success(response))
        }
        None => Err(crate::error::ApiError::NotFound),
    }
}

/// 获取Worker统计信息API处理器
///
/// 处理GET /workers/{id}/stats请求，返回指定Worker的详细统计数据。
/// 包含任务执行统计、性能指标和历史数据，用于监控和分析。
///
/// # 路径参数
///
/// - `id`: Worker的唯一标识符
///
/// # 响应格式
///
/// ## 成功响应 (200 OK)
/// ```json
/// {
///   "success": true,
///   "data": {
///     "worker_id": "worker-001",
///     "current_task_count": 3,
///     "max_concurrent_tasks": 10,
///     "load_percentage": 30.0,
///     "total_completed_tasks": 1250,
///     "total_failed_tasks": 25,
///     "average_task_duration_ms": 5500.0,
///     "last_heartbeat": "2024-01-15T10:30:00Z"
///   }
/// }
/// ```
///
/// ## 错误响应
/// - `404 Not Found`: Worker不存在
/// - `500 Internal Server Error`: 统计数据查询失败
///
/// # 统计数据说明
///
/// ## 实时指标
/// - `current_task_count`: 当前执行任务数
/// - `load_percentage`: 当前负载百分比
/// - `last_heartbeat`: 最后心跳时间
///
/// ## 历史统计
/// - `total_completed_tasks`: 累计完成任务数
/// - `total_failed_tasks`: 累计失败任务数
/// - `average_task_duration_ms`: 平均任务执行时间（毫秒）
///
/// # 实现逻辑
///
/// 1. **Worker验证**: 首先验证Worker是否存在
/// 2. **统计查询**: 查询Worker的负载统计数据
/// 3. **数据合并**: 合并实时状态和历史统计
/// 4. **默认处理**: 为新Worker提供基础统计信息
/// 5. **响应构建**: 构建标准化响应格式
///
/// # 数据来源
///
/// - **实时数据**: 来自Worker心跳和状态更新
/// - **历史数据**: 来自任务执行记录的聚合统计
/// - **计算数据**: 基于原始数据计算的派生指标
///
/// # 性能考虑
///
/// - 统计数据可能涉及复杂查询，考虑缓存
/// - 大量历史数据的聚合计算可能较慢
/// - 建议使用预计算的统计表
///
/// # 使用场景
///
/// - Worker性能监控仪表板
/// - 负载均衡决策参考
/// - 容量规划和资源优化
/// - 故障分析和性能调优
/// - SLA监控和报告
///
/// # 示例请求
///
/// ```bash
/// # 获取Worker统计信息
/// curl "http://localhost:8080/api/v1/workers/worker-001/stats"
///
/// # 结合jq工具解析响应
/// curl -s "http://localhost:8080/api/v1/workers/worker-001/stats" | \
///   jq '.data.load_percentage'
/// ```
///
/// # 扩展功能
///
/// 未来可以考虑添加：
/// - 时间范围过滤（如最近24小时的统计）
/// - 更详细的性能指标（CPU、内存使用率）
/// - 任务类型维度的统计分解
/// - 趋势分析和预测数据
pub async fn get_worker_stats(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<impl axum::response::IntoResponse> {
    // 首先检查worker是否存在
    let worker = match state.worker_repo.get_by_id(&id).await? {
        Some(worker) => worker,
        None => return Err(crate::error::ApiError::NotFound),
    };

    // 获取worker的负载统计
    let load_stats = state.worker_repo.get_worker_load_stats().await?;
    let worker_load_stat = load_stats.into_iter().find(|stat| stat.worker_id == id);

    match worker_load_stat {
        Some(stat) => Ok(success(stat)),
        None => {
            // 如果没有找到统计信息，创建一个基本的统计信息
            let load_percentage = worker.load_percentage();
            let basic_stat = scheduler_core::traits::WorkerLoadStats {
                worker_id: worker.id,
                current_task_count: worker.current_task_count,
                max_concurrent_tasks: worker.max_concurrent_tasks,
                load_percentage,
                total_completed_tasks: 0,
                total_failed_tasks: 0,
                average_task_duration_ms: None,
                last_heartbeat: worker.last_heartbeat,
            };
            Ok(success(basic_stat))
        }
    }
}
