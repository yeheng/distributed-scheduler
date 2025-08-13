#[cfg(test)]
pub mod mocks {
    // Re-export shared mock implementations from testing-utils
    pub use scheduler_testing_utils::{
        MockTaskRepository, MockTaskRunRepository, MockWorkerRepository, TaskBuilder,
        TaskRunBuilder, WorkerInfoBuilder,
    };

    use crate::retry_service::RetryService;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use scheduler_foundation::SchedulerResult;
    use scheduler_domain::entities::TaskRun;

    #[derive(Debug, Clone)]
    pub struct MockRetryService;

    impl MockRetryService {
        pub fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl RetryService for MockRetryService {
        async fn handle_failed_task(&self, _task_run_id: i64) -> SchedulerResult<bool> {
            Ok(true)
        }

        async fn handle_timeout_task(&self, _task_run_id: i64) -> SchedulerResult<bool> {
            Ok(true)
        }

        async fn handle_worker_failure(&self, _worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
            Ok(vec![])
        }

        async fn scan_retry_tasks(&self) -> SchedulerResult<Vec<TaskRun>> {
            Ok(vec![])
        }

        fn calculate_next_retry_time(&self, _retry_count: i32) -> DateTime<Utc> {
            Utc::now()
        }
    }
}
