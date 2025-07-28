use scheduler_api::routes::{create_routes, AppState};
use scheduler_core::traits::{
    TaskControlService, TaskRepository, TaskRunRepository, WorkerRepository,
};
use scheduler_infrastructure::database::postgres_repositories::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::net::TcpListener;

/// 测试应用状态
pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

impl TestApp {
    /// 启动测试应用
    pub async fn spawn() -> TestApp {
        // 设置测试数据库连接
        let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:123456@localhost:5432/scheduler".to_string()
        });

        let db_pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database");

        // 运行数据库迁移
        sqlx::migrate!("../../migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");

        // 创建仓储实例
        let task_repo: Arc<dyn TaskRepository> =
            Arc::new(PostgresTaskRepository::new(db_pool.clone()));
        let task_run_repo: Arc<dyn TaskRunRepository> =
            Arc::new(PostgresTaskRunRepository::new(db_pool.clone()));
        let worker_repo: Arc<dyn WorkerRepository> =
            Arc::new(PostgresWorkerRepository::new(db_pool.clone()));

        // 创建模拟的任务控制服务
        let task_controller: Arc<dyn TaskControlService> = Arc::new(MockTaskControlService);

        // 创建应用状态
        let app_state = AppState {
            task_repo,
            task_run_repo,
            worker_repo,
            task_controller,
        };

        // 创建路由
        let app = create_routes(app_state);

        // 启动测试服务器
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind random port");
        let address = format!("http://{}", listener.local_addr().unwrap());

        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("Failed to start test server");
        });

        TestApp { address, db_pool }
    }

    /// 清理测试数据
    pub async fn cleanup(&self) {
        sqlx::query("TRUNCATE TABLE task_runs, tasks, workers RESTART IDENTITY CASCADE")
            .execute(&self.db_pool)
            .await
            .expect("Failed to cleanup test data");
    }
}

/// 模拟任务控制服务
struct MockTaskControlService;

#[async_trait::async_trait]
impl TaskControlService for MockTaskControlService {
    async fn trigger_task(
        &self,
        task_id: i64,
    ) -> scheduler_core::Result<scheduler_core::models::TaskRun> {
        use chrono::Utc;
        use scheduler_core::models::TaskRun;

        Ok(TaskRun::new(task_id, Utc::now()))
    }

    async fn pause_task(&self, _task_id: i64) -> scheduler_core::Result<()> {
        Ok(())
    }

    async fn resume_task(&self, _task_id: i64) -> scheduler_core::Result<()> {
        Ok(())
    }

    async fn restart_task_run(
        &self,
        task_run_id: i64,
    ) -> scheduler_core::Result<scheduler_core::models::TaskRun> {
        use chrono::Utc;
        use scheduler_core::models::TaskRun;

        let mut task_run = TaskRun::new(1, Utc::now());
        task_run.id = task_run_id;
        Ok(task_run)
    }

    async fn abort_task_run(&self, _task_run_id: i64) -> scheduler_core::Result<()> {
        Ok(())
    }
}
