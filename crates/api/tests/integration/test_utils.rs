use scheduler_api::routes::{create_routes, AppState};
use scheduler_foundation::traits::TaskControlService;
use scheduler_domain::{TaskRepository, TaskRunRepository, WorkerRepository};
use scheduler_infrastructure::database::postgres::{
    PostgresTaskRepository, PostgresTaskRunRepository, PostgresWorkerRepository,
};
use scheduler_testing_utils::TaskRunBuilder;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::net::TcpListener;
use tokio::time::{sleep, Duration};

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
}

impl TestApp {
    pub async fn spawn() -> TestApp {
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
        sqlx::migrate!("../../migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run migrations");
        let task_repo: Arc<dyn TaskRepository> =
            Arc::new(PostgresTaskRepository::new(db_pool.clone()));
        let task_run_repo: Arc<dyn TaskRunRepository> =
            Arc::new(PostgresTaskRunRepository::new(db_pool.clone()));
        let worker_repo: Arc<dyn WorkerRepository> =
            Arc::new(PostgresWorkerRepository::new(db_pool.clone()));
        let task_controller: Arc<dyn TaskControlService> = Arc::new(MockTaskControlService);
        let auth_config = Arc::new(scheduler_api::auth::AuthConfig {
            enabled: false,
            jwt_secret: "test-secret".to_string(),
            api_keys: std::collections::HashMap::new(),
            jwt_expiration_hours: 24,
        });
        let app_state = AppState {
            task_repo,
            task_run_repo,
            worker_repo,
            task_controller,
            auth_config,
            rate_limiter: None,
        };
        let app = create_routes(app_state);
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
    pub async fn cleanup(&self) {
        sqlx::query("TRUNCATE TABLE task_runs, tasks, workers RESTART IDENTITY CASCADE")
            .execute(&self.db_pool)
            .await
            .expect("Failed to cleanup test data");
    }
}

struct MockTaskControlService;

#[async_trait::async_trait]
impl TaskControlService for MockTaskControlService {
    async fn trigger_task(
        &self,
        task_id: i64,
    ) -> scheduler_foundation::SchedulerResult<scheduler_domain::entities::TaskRun> {
        use chrono::Utc;

        Ok(TaskRunBuilder::new()
            .with_task_id(task_id)
            .with_scheduled_at(Utc::now())
            .build())
    }

    async fn pause_task(&self, _task_id: i64) -> scheduler_foundation::SchedulerResult<()> {
        Ok(())
    }

    async fn resume_task(&self, _task_id: i64) -> scheduler_foundation::SchedulerResult<()> {
        Ok(())
    }

    async fn restart_task_run(
        &self,
        task_run_id: i64,
    ) -> scheduler_foundation::SchedulerResult<scheduler_domain::entities::TaskRun> {
        use chrono::Utc;

        Ok(TaskRunBuilder::new()
            .with_id(task_run_id)
            .with_task_id(1)
            .with_scheduled_at(Utc::now())
            .build())
    }

    async fn abort_task_run(&self, _task_run_id: i64) -> scheduler_foundation::SchedulerResult<()> {
        Ok(())
    }

    async fn cancel_all_task_runs(&self, _task_id: i64) -> scheduler_foundation::SchedulerResult<usize> {
        Ok(0)
    }

    async fn has_running_instances(&self, _task_id: i64) -> scheduler_foundation::SchedulerResult<bool> {
        Ok(false)
    }

    async fn get_recent_executions(
        &self,
        _task_id: i64,
        _limit: usize,
    ) -> scheduler_foundation::SchedulerResult<Vec<scheduler_domain::entities::TaskRun>> {
        Ok(Vec::new())
    }
}
