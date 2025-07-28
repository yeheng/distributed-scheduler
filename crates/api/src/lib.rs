pub mod error;
pub mod handlers;
pub mod middleware;
pub mod response;
pub mod routes;

use axum::Router;
use std::sync::Arc;
use tower::ServiceBuilder;

use middleware::{cors_layer, request_logging, trace_layer};
use routes::{create_routes, AppState};

/// 创建完整的API应用
pub fn create_app(
    task_repo: Arc<dyn scheduler_core::traits::repository::TaskRepository>,
    task_run_repo: Arc<dyn scheduler_core::traits::repository::TaskRunRepository>,
    worker_repo: Arc<dyn scheduler_core::traits::repository::WorkerRepository>,
    task_controller: Arc<dyn scheduler_core::traits::scheduler::TaskControlService>,
) -> Router {
    let state = AppState {
        task_repo,
        task_run_repo,
        worker_repo,
        task_controller,
    };

    create_routes(state).layer(
        ServiceBuilder::new()
            .layer(trace_layer())
            .layer(cors_layer())
            .layer(axum::middleware::from_fn(request_logging)),
    )
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
    impl scheduler_core::traits::repository::TaskRepository for MockTaskRepository {
        async fn create(
            &self,
            _task: &scheduler_core::models::Task,
        ) -> scheduler_core::Result<scheduler_core::models::Task> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _id: i64,
        ) -> scheduler_core::Result<Option<scheduler_core::models::Task>> {
            unimplemented!()
        }

        async fn get_by_name(
            &self,
            _name: &str,
        ) -> scheduler_core::Result<Option<scheduler_core::models::Task>> {
            unimplemented!()
        }

        async fn update(&self, _task: &scheduler_core::models::Task) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn list(
            &self,
            _filter: &scheduler_core::models::TaskFilter,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::Task>> {
            Ok(vec![])
        }

        async fn get_active_tasks(
            &self,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::Task>> {
            unimplemented!()
        }

        async fn get_schedulable_tasks(
            &self,
            _current_time: chrono::DateTime<chrono::Utc>,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::Task>> {
            unimplemented!()
        }

        async fn check_dependencies(&self, _task_id: i64) -> scheduler_core::Result<bool> {
            unimplemented!()
        }

        async fn get_dependencies(
            &self,
            _task_id: i64,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::Task>> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _task_ids: &[i64],
            _status: scheduler_core::models::TaskStatus,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl scheduler_core::traits::repository::TaskRunRepository for MockTaskRunRepository {
        async fn create(
            &self,
            _task_run: &scheduler_core::models::TaskRun,
        ) -> scheduler_core::Result<scheduler_core::models::TaskRun> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _id: i64,
        ) -> scheduler_core::Result<Option<scheduler_core::models::TaskRun>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _task_run: &scheduler_core::models::TaskRun,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn delete(&self, _id: i64) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn get_by_task_id(
            &self,
            _task_id: i64,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::TaskRun>> {
            unimplemented!()
        }

        async fn get_by_worker_id(
            &self,
            _worker_id: &str,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::TaskRun>> {
            unimplemented!()
        }

        async fn get_by_status(
            &self,
            _status: scheduler_core::models::TaskRunStatus,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::TaskRun>> {
            Ok(vec![])
        }

        async fn get_pending_runs(
            &self,
            _limit: Option<i64>,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::TaskRun>> {
            unimplemented!()
        }

        async fn get_running_runs(
            &self,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::TaskRun>> {
            unimplemented!()
        }

        async fn get_timeout_runs(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::TaskRun>> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _id: i64,
            _status: scheduler_core::models::TaskRunStatus,
            _worker_id: Option<&str>,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn update_result(
            &self,
            _id: i64,
            _result: Option<&str>,
            _error_message: Option<&str>,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn get_recent_runs(
            &self,
            _task_id: i64,
            _limit: i64,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::TaskRun>> {
            unimplemented!()
        }

        async fn get_execution_stats(
            &self,
            _task_id: i64,
            _days: i32,
        ) -> scheduler_core::Result<scheduler_core::traits::repository::TaskExecutionStats>
        {
            unimplemented!()
        }

        async fn cleanup_old_runs(&self, _days: i32) -> scheduler_core::Result<u64> {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _run_ids: &[i64],
            _status: scheduler_core::models::TaskRunStatus,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl scheduler_core::traits::repository::WorkerRepository for MockWorkerRepository {
        async fn register(
            &self,
            _worker: &scheduler_core::models::WorkerInfo,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn unregister(&self, _worker_id: &str) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn get_by_id(
            &self,
            _worker_id: &str,
        ) -> scheduler_core::Result<Option<scheduler_core::models::WorkerInfo>> {
            unimplemented!()
        }

        async fn update(
            &self,
            _worker: &scheduler_core::models::WorkerInfo,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn list(&self) -> scheduler_core::Result<Vec<scheduler_core::models::WorkerInfo>> {
            Ok(vec![])
        }

        async fn get_alive_workers(
            &self,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::WorkerInfo>> {
            Ok(vec![])
        }

        async fn get_workers_by_task_type(
            &self,
            _task_type: &str,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::WorkerInfo>> {
            unimplemented!()
        }

        async fn update_heartbeat(
            &self,
            _worker_id: &str,
            _heartbeat_time: chrono::DateTime<chrono::Utc>,
            _current_task_count: i32,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn update_status(
            &self,
            _worker_id: &str,
            _status: scheduler_core::models::WorkerStatus,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn get_timeout_workers(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::Result<Vec<scheduler_core::models::WorkerInfo>> {
            unimplemented!()
        }

        async fn cleanup_offline_workers(
            &self,
            _timeout_seconds: i64,
        ) -> scheduler_core::Result<u64> {
            unimplemented!()
        }

        async fn get_worker_load_stats(
            &self,
        ) -> scheduler_core::Result<Vec<scheduler_core::traits::repository::WorkerLoadStats>>
        {
            unimplemented!()
        }

        async fn batch_update_status(
            &self,
            _worker_ids: &[String],
            _status: scheduler_core::models::WorkerStatus,
        ) -> scheduler_core::Result<()> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl scheduler_core::traits::scheduler::TaskControlService for MockTaskController {
        async fn trigger_task(
            &self,
            _task_id: i64,
        ) -> scheduler_core::Result<scheduler_core::models::TaskRun> {
            unimplemented!()
        }

        async fn pause_task(&self, _task_id: i64) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn resume_task(&self, _task_id: i64) -> scheduler_core::Result<()> {
            unimplemented!()
        }

        async fn restart_task_run(
            &self,
            _task_run_id: i64,
        ) -> scheduler_core::Result<scheduler_core::models::TaskRun> {
            unimplemented!()
        }

        async fn abort_task_run(&self, _task_run_id: i64) -> scheduler_core::Result<()> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let task_repo = Arc::new(MockTaskRepository)
            as Arc<dyn scheduler_core::traits::repository::TaskRepository>;
        let task_run_repo = Arc::new(MockTaskRunRepository)
            as Arc<dyn scheduler_core::traits::repository::TaskRunRepository>;
        let worker_repo = Arc::new(MockWorkerRepository)
            as Arc<dyn scheduler_core::traits::repository::WorkerRepository>;
        let task_controller = Arc::new(MockTaskController)
            as Arc<dyn scheduler_core::traits::scheduler::TaskControlService>;

        let app = create_app(task_repo, task_run_repo, worker_repo, task_controller);

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
            as Arc<dyn scheduler_core::traits::repository::TaskRepository>;
        let task_run_repo = Arc::new(MockTaskRunRepository)
            as Arc<dyn scheduler_core::traits::repository::TaskRunRepository>;
        let worker_repo = Arc::new(MockWorkerRepository)
            as Arc<dyn scheduler_core::traits::repository::WorkerRepository>;
        let task_controller = Arc::new(MockTaskController)
            as Arc<dyn scheduler_core::traits::scheduler::TaskControlService>;

        let app = create_app(task_repo, task_run_repo, worker_repo, task_controller);

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
