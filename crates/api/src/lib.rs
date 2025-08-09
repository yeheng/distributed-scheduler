//! # Scheduler API
//!
//! 分布式任务调度系统的REST API服务模块，提供HTTP接口用于任务管理和系统监控。
//!
//! ## 概述
//!
//! 此模块基于Axum框架构建，提供完整的RESTful API接口，包括：
//! - 任务管理（创建、查询、更新、删除）
//! - Worker节点管理（注册、心跳、状态查询）
//! - 系统监控（健康检查、性能指标）
//! - 实时状态查询（任务运行状态、Worker负载）
//!
//! ## 架构设计
//!
//! ```
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Scheduler API                             │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Routes  │  Handlers  │  Middleware  │  Response  │  Error  │
//! │  (路由定义) │ (处理器)   │ (中间件)     │ (响应格式) │ (错误处理) │
//! └─────────────────────────────────────────────────────────────┘
//!                       ↓
//!                ┌─────────────────┐
//!                │  Core Services  │
//!                │  (核心服务层)    │
//!                └─────────────────┘
//! ```
//!
//! ## API 端点
//!
//! ### 任务管理
//! - `GET /api/tasks` - 获取任务列表
//! - `POST /api/tasks` - 创建新任务
//! - `GET /api/tasks/{id}` - 获取任务详情
//! - `PUT /api/tasks/{id}` - 更新任务
//! - `DELETE /api/tasks/{id}` - 删除任务
//! - `POST /api/tasks/{id}/trigger` - 手动触发任务
//! - `POST /api/tasks/{id}/pause` - 暂停任务
//! - `POST /api/tasks/{id}/resume` - 恢复任务
//!
//! ### Worker管理
//! - `GET /api/workers` - 获取Worker列表
//! - `POST /api/workers` - 注册Worker
//! - `GET /api/workers/{id}` - 获取Worker详情
//! - `PUT /api/workers/{id}` - 更新Worker
//! - `DELETE /api/workers/{id}` - 注销Worker
//! - `POST /api/workers/{id}/heartbeat` - Worker心跳
//!
//! ### 系统监控
//! - `GET /api/system/health` - 系统健康检查
//! - `GET /api/system/stats` - 系统统计信息
//! - `GET /api/system/metrics` - 性能指标
//! - `GET /api/system/logs` - 系统日志
//!
//! ### 任务运行管理
//! - `GET /api/runs` - 获取任务运行列表
//! - `GET /api/runs/{id}` - 获取运行详情
//! - `POST /api/runs/{id}/restart` - 重启任务运行
//! - `POST /api/runs/{id}/abort` - 中止任务运行
//!
//! ## 使用示例
//!
//! ### 创建应用
//!
//! ```rust
//! use scheduler_api::create_app;
//! use scheduler_core::traits::*;
//! use std::sync::Arc;
//!
//! // 创建依赖的服务
//! let task_repo = Arc::new(MyTaskRepository::new());
//! let task_run_repo = Arc::new(MyTaskRunRepository::new());
//! let worker_repo = Arc::new(MyWorkerRepository::new());
//! let task_controller = Arc::new(MyTaskController::new());
//!
//! // 创建API应用
//! let app = create_app(
//!     task_repo,
//!     task_run_repo, 
//!     worker_repo,
//!     task_controller
//! );
//!
//! // 启动服务器
//! let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
//! axum::serve(listener, app).await?;
//! ```
//!
//! ### API 调用示例
//!
//! ```bash
//! # 创建任务
//! curl -X POST http://localhost:8080/api/tasks \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "name": "data_processing",
//!     "description": "处理每日数据",
//!     "task_type": "batch",
//!     "executor": "python",
//!     "command": "process_data.py",
//!     "schedule": "0 2 * * *"
//!   }'
//!
//! # 获取任务列表
//! curl -X GET http://localhost:8080/api/tasks
//!
//! # 触发任务
//! curl -X POST http://localhost:8080/api/tasks/1/trigger
//!
//! # 系统健康检查
//! curl -X GET http://localhost:8080/api/system/health
//! ```
//!
//! ## 中间件
//!
//! ### 内置中间件
//! - **CORS**: 跨域资源共享
//! - **日志记录**: 请求和响应日志
//! - **追踪**: 分布式追踪支持
//! - **错误处理**: 统一错误响应格式
//!
//! ### 自定义中间件
//!
//! ```rust
//! use axum::middleware;
//! use tower::ServiceBuilder;
//!
//! // 添加自定义中间件
//! let app = create_routes(state).layer(
//!     ServiceBuilder::new()
//!         .layer(trace_layer())
//!         .layer(cors_layer())
//!         .layer(axum::middleware::from_fn(custom_middleware))
//! );
//! ```
//!
//! ## 响应格式
//!
//! ### 成功响应
//! ```json
//! {
//!   "success": true,
//!   "data": {
//!     "id": 1,
//!     "name": "data_processing",
//!     "status": "active"
//!   },
//!   "timestamp": "2024-01-01T00:00:00Z"
//! }
//! ```
//!
//! ### 错误响应
//! ```json
//! {
//!   "success": false,
//!   "error": {
//!     "code": "VALIDATION_ERROR",
//!     "message": "任务名称不能为空",
//!     "details": {
//!       "field": "name",
//!       "rule": "required"
//!     }
//!   },
//!   "timestamp": "2024-01-01T00:00:00Z"
//! }
//! ```
//!
//! ## 错误处理
//!
//! API模块提供统一的错误处理机制：
//!
//! ```rust
//! use scheduler_api::error::ApiError;
//!
//! // 处理验证错误
//! let error = ApiError::validation("任务名称不能为空");
//!
//! // 处理数据库错误
//! let error = ApiError::database("数据库连接失败");
//!
//! // 处理内部错误
//! let error = ApiError::internal("服务器内部错误");
//! ```
//!
//! ## 测试
//!
//! ### 单元测试
//!
//! ```rust
//! #[tokio::test]
//! async fn test_create_task_endpoint() {
//!     // 准备测试数据
//!     let app = create_test_app();
//!     
//!     // 发送请求
//!     let response = app
//!         .oneshot(Request::builder()
//!             .uri("/api/tasks")
//!             .method("POST")
//!             .header("content-type", "application/json")
//!             .body(Body::from(r#"{"name":"test"}"#))
//!             .unwrap())
//!         .await
//!         .unwrap();
//!     
//!     // 验证响应
//!     assert_eq!(response.status(), StatusCode::CREATED);
//! }
//! ```
//!
//! ### 集成测试
//!
//! ```bash
//! # 运行API测试
//! cargo test -p scheduler-api
//!
//! # 运行集成测试
//! cargo test --test api_integration_tests
//! ```
//!
//! ## 性能优化
//!
//! ### 数据库查询优化
//! - 使用连接池
//! - 批量查询
//! - 索引优化
//!
//! ### 响应缓存
//! - 静态资源缓存
//! - 查询结果缓存
//! - ETag支持
//!
//! ### 并发处理
//! - 异步处理
//! - 请求限流
//! - 超时控制
//!
//! ## 监控和日志
//!
//! ### 结构化日志
//! ```rust
//! use tracing::{info, error, warn};
//!
//! info!(method = %method, path = %path, "请求处理开始");
//! error!(error = %e, "请求处理失败");
//! ```
//!
//! ### 性能指标
//! - 请求处理时间
//! - 响应大小
//! - 错误率
//! - 并发连接数
//!
//! ## 安全考虑
//!
//! ### 输入验证
//! - 所有输入参数验证
//! - SQL注入防护
//! - XSS攻击防护
//!
//! ### 访问控制
//! - API密钥认证
//! - 权限验证
//! - 速率限制
//!
//! ## 部署配置
//!
//! ### 环境变量
//! ```bash
//! # 服务器配置
//! RUST_LOG=info
//! API_PORT=8080
//! API_HOST=0.0.0.0
//!
//! # 数据库配置
//! DATABASE_URL=postgresql://user:pass@localhost/scheduler
//!
//! # 安全配置
//! API_KEY=your-secret-key
//! CORS_ORIGINS=http://localhost:3000
//! ```
//!
//! ## 相关模块
//!
//! - [`scheduler_core`] - 核心数据模型和服务接口
//! - [`scheduler_dispatcher`] - 任务调度器
//! - [`scheduler_worker`] - Worker节点
//! - [`scheduler_infrastructure`] - 基础设施实现
//!
//! [`scheduler_core`]: ../core/index.html
//! [`scheduler_dispatcher`]: ../dispatcher/index.html
//! [`scheduler_worker`]: ../worker/index.html
//! [`scheduler_infrastructure`]: ../infrastructure/index.html

pub mod error;
pub mod handlers;
pub mod middleware;
pub mod response;
pub mod routes;
pub mod auth;

use axum::Router;
use std::sync::Arc;
use tower::ServiceBuilder;

use middleware::{cors_layer, request_logging, trace_layer};
use routes::{create_routes, AppState};
use scheduler_core::config::models::api_observability::AuthConfig as CoreAuthConfig;
use scheduler_core::traits::repository::*;
use scheduler_core::traits::scheduler::*;
use scheduler_core::config::models::api_observability::ApiConfig;

/// 创建完整的API应用
pub fn create_app(
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    worker_repo: Arc<dyn WorkerRepository>,
    task_controller: Arc<dyn TaskControlService>,
    api_config: ApiConfig,
) -> Router {
    // 转换配置格式
    let auth_config = convert_auth_config(&api_config.auth);

    let state = AppState {
        task_repo,
        task_run_repo,
        worker_repo,
        task_controller,
        auth_config,
    };

    create_routes(state).layer(
        ServiceBuilder::new()
            .layer(trace_layer())
            .layer(cors_layer())
            .layer(axum::middleware::from_fn(request_logging)),
    )
}

/// 简化的应用创建函数（向后兼容）
pub fn create_simple_app(
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    worker_repo: Arc<dyn WorkerRepository>,
    task_controller: Arc<dyn TaskControlService>,
) -> Router {
    let default_api_config = ApiConfig {
        enabled: true,
        bind_address: "0.0.0.0:8080".to_string(),
        cors_enabled: true,
        cors_origins: vec!["*".to_string()],
        request_timeout_seconds: 30,
        max_request_size_mb: 10,
        auth: CoreAuthConfig::default(),
    };

    create_app(task_repo, task_run_repo, worker_repo, task_controller, default_api_config)
}

// 转换配置格式的辅助函数
fn convert_auth_config(core_config: &CoreAuthConfig) -> Arc<auth::AuthConfig> {
    let mut auth_api_keys = std::collections::HashMap::new();

    for (hash, key_config) in &core_config.api_keys {
        let permissions: Vec<auth::Permission> = key_config
            .permissions
            .iter()
            .filter_map(|p| match p.as_str() {
                "TaskRead" => Some(auth::Permission::TaskRead),
                "TaskWrite" => Some(auth::Permission::TaskWrite),
                "TaskDelete" => Some(auth::Permission::TaskDelete),
                "WorkerRead" => Some(auth::Permission::WorkerRead),
                "WorkerWrite" => Some(auth::Permission::WorkerWrite),
                "SystemRead" => Some(auth::Permission::SystemRead),
                "SystemWrite" => Some(auth::Permission::SystemWrite),
                "Admin" => Some(auth::Permission::Admin),
                _ => None,
            })
            .collect();

        auth_api_keys.insert(
            hash.clone(),
            auth::ApiKeyInfo {
                name: key_config.name.clone(),
                permissions,
                is_active: key_config.is_active,
            },
        );
    }

    Arc::new(auth::AuthConfig {
        enabled: core_config.enabled,
        jwt_secret: core_config.jwt_secret.clone(),
        api_keys: auth_api_keys,
        jwt_expiration_hours: core_config.jwt_expiration_hours,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    // Mock implementations for testing
    struct MockTaskRepository;
    struct MockTaskRunRepository;
    struct MockWorkerRepository;
    struct MockTaskController;

    #[async_trait::async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn create(
            &self,
            _task: &scheduler_domain::entities::Task,
        ) -> scheduler_core::SchedulerResult<scheduler_domain::entities::Task> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _id: i64,
        ) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn get_by_name(
            &self,
            _name: &str,
        ) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _task: &scheduler_domain::entities::Task,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn list(
            &self,
            _filter: &scheduler_domain::entities::TaskFilter,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            Ok(vec![])
        }

        async fn get_active_tasks(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn get_schedulable_tasks(
            &self,
            _current_time: chrono::DateTime<chrono::Utc>,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn check_dependencies(&self, _task_id: i64) -> scheduler_core::SchedulerResult<bool> {
            unimplemented!()
        }

        async fn get_dependencies(
            &self,
            _task_id: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::Task>> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _task_ids: &[i64],
            _status: scheduler_domain::entities::TaskStatus,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl TaskRunRepository for MockTaskRunRepository {
        async fn create(
            &self,
            _task_run: &scheduler_domain::entities::TaskRun,
        ) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _id: i64,
        ) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _task_run: &scheduler_domain::entities::TaskRun,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_by_task_id(
            &self,
            _task_id: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_by_worker_id(
            &self,
            _worker_id: &str,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_by_status(
            &self,
            _status: scheduler_domain::entities::TaskRunStatus,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            Ok(vec![])
        }

        async fn get_pending_runs(
            &self,
            _limit: Option<i64>,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_running_runs(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_timeout_runs(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _id: i64,
            _status: scheduler_domain::entities::TaskRunStatus,
            _worker_id: Option<&str>,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn update_result(
            &self,
            _id: i64,
            _result: Option<&str>,
            _error_message: Option<&str>,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_recent_runs(
            &self,
            _task_id: i64,
            _limit: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
            unimplemented!()
        }

        async fn get_execution_stats(
            &self,
            _task_id: i64,
            _days: i32,
        ) -> scheduler_core::SchedulerResult<TaskExecutionStats>
        {
            unimplemented!()
        }

        async fn cleanup_old_runs(&self, _days: i32) -> scheduler_core::SchedulerResult<u64> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _run_ids: &[i64],
            _status: scheduler_domain::entities::TaskRunStatus,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl WorkerRepository for MockWorkerRepository {
        async fn register(
            &self,
            _worker: &scheduler_domain::entities::WorkerInfo,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn unregister(&self, _worker_id: &str) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _worker_id: &str,
        ) -> scheduler_core::SchedulerResult<Option<scheduler_domain::entities::WorkerInfo>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _worker: &scheduler_domain::entities::WorkerInfo,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn list(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
            Ok(vec![])
        }

        async fn get_alive_workers(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
            Ok(vec![])
        }

        async fn get_workers_by_task_type(
            &self,
            _task_type: &str,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
            unimplemented!()
        }

        async fn update_heartbeat(
            &self,
            _worker_id: &str,
            _heartbeat_time: chrono::DateTime<chrono::Utc>,
            _current_task_count: i32,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _worker_id: &str,
            _status: scheduler_domain::entities::WorkerStatus,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn get_timeout_workers(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::SchedulerResult<Vec<scheduler_domain::entities::WorkerInfo>> {
            unimplemented!()
        }

        async fn cleanup_offline_workers(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::SchedulerResult<u64> {
            unimplemented!()
        }

        async fn get_worker_load_stats(
            &self,
        ) -> scheduler_core::SchedulerResult<Vec<WorkerLoadStats>>
        {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _worker_ids: &[String],
            _status: scheduler_domain::entities::WorkerStatus,
        ) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl TaskControlService for MockTaskController {
        async fn trigger_task(
            &self,
            _task_id: i64,
        ) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn pause_task(&self, _task_id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn resume_task(&self, _task_id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }

        async fn restart_task_run(
            &self,
            _task_run_id: i64,
        ) -> scheduler_core::SchedulerResult<scheduler_domain::entities::TaskRun> {
            unimplemented!()
        }

        async fn abort_task_run(&self, _task_run_id: i64) -> scheduler_core::SchedulerResult<()> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let task_repo = Arc::new(MockTaskRepository)
            as Arc<dyn TaskRepository>;
        let task_run_repo = Arc::new(MockTaskRunRepository)
            as Arc<dyn TaskRunRepository>;
        let worker_repo = Arc::new(MockWorkerRepository)
            as Arc<dyn WorkerRepository>;
        let task_controller = Arc::new(MockTaskController)
            as Arc<dyn TaskControlService>;

        let app = create_simple_app(task_repo, task_run_repo, worker_repo, task_controller);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_routes_exist() {
        let task_repo = Arc::new(MockTaskRepository)
            as Arc<dyn TaskRepository>;
        let task_run_repo = Arc::new(MockTaskRunRepository)
            as Arc<dyn TaskRunRepository>;
        let worker_repo = Arc::new(MockWorkerRepository)
            as Arc<dyn WorkerRepository>;
        let task_controller = Arc::new(MockTaskController)
            as Arc<dyn TaskControlService>;

        let app = create_simple_app(task_repo, task_run_repo, worker_repo, task_controller);

        // Test tasks endpoint
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/tasks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Test workers endpoint
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/workers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Test system stats endpoint
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/system/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Test system health endpoint
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/system/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
