use scheduler_api::routes::{create_routes, AppState};
use scheduler_core::traits::{
    TaskControlService, TaskRepository, TaskRunRepository, WorkerRepository,
};
use scheduler_infrastructure::database::postgres_repositories::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::net::TcpListener;
use tokio::time::{sleep, Duration};

/// 测试应用状态
pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
}

impl TestApp {
    /// 启动测试应用
    pub async fn spawn() -> TestApp {
        // Start PostgreSQL container
        let postgres_image = Postgres::default()
            .with_db_name("scheduler_test")
            .with_user("test_user")
            .with_password("test_password")
            .with_tag("16-alpine");

        let container = postgres_image
            .start()
            .await
            .expect("Failed to start postgres container");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get port");

        let database_url = format!(
            "postgresql://test_user:test_password@localhost:{}/scheduler_test",
            port
        );

        // Wait for database to be ready
        let mut retry_count = 0;
        let db_pool = loop {
            match PgPool::connect(&database_url).await {
                Ok(pool) => break pool,
                Err(_) if retry_count < 30 => {
                    retry_count += 1;
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => panic!("Failed to connect to test database: {}", e),
            }
        };

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

        TestApp {
            address,
            db_pool,
            container,
        }
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
