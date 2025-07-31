#[cfg(test)]
pub mod mocks {
    use crate::retry_service::RetryService;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use scheduler_core::models::{TaskFilter, TaskStatus};
    use scheduler_core::{
        SchedulerResult, Task, TaskRepository, TaskRun, TaskRunRepository, TaskRunStatus, WorkerInfo,
        WorkerRepository, WorkerStatus,
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    pub struct MockTaskRepository {
        tasks: Arc<Mutex<HashMap<i64, Task>>>,
    }

    impl MockTaskRepository {
        pub fn new() -> Self {
            Self {
                tasks: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn create(&self, task: &Task) -> SchedulerResult<Task> {
            let mut tasks = self.tasks.lock().unwrap();
            let mut new_task = task.clone();
            new_task.id = (tasks.len() + 1) as i64;
            tasks.insert(new_task.id, new_task.clone());
            Ok(new_task)
        }

        async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks.get(&id).cloned())
        }

        async fn get_by_name(&self, name: &str) -> SchedulerResult<Option<Task>> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks.values().find(|t| t.name == name).cloned())
        }

        async fn update(&self, task: &Task) -> SchedulerResult<()> {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(task.id, task.clone());
            Ok(())
        }

        async fn delete(&self, id: i64) -> SchedulerResult<()> {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.remove(&id);
            Ok(())
        }

        async fn list(&self, _filter: &TaskFilter) -> SchedulerResult<Vec<Task>> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks.values().cloned().collect())
        }

        async fn get_active_tasks(&self) -> SchedulerResult<Vec<Task>> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks
                .values()
                .filter(|t| t.status == TaskStatus::Active)
                .cloned()
                .collect())
        }

        async fn get_schedulable_tasks(&self, _now: DateTime<Utc>) -> SchedulerResult<Vec<Task>> {
            Ok(vec![])
        }

        async fn check_dependencies(&self, _task_id: i64) -> SchedulerResult<bool> {
            Ok(true)
        }

        async fn get_dependencies(&self, _task_id: i64) -> SchedulerResult<Vec<Task>> {
            Ok(vec![])
        }

        async fn batch_update_status(&self, _task_ids: &[i64], _status: TaskStatus) -> SchedulerResult<()> {
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    pub struct MockTaskRunRepository {
        task_runs: Arc<Mutex<HashMap<i64, TaskRun>>>,
    }

    impl MockTaskRunRepository {
        pub fn new() -> Self {
            Self {
                task_runs: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl TaskRunRepository for MockTaskRunRepository {
        async fn create(&self, task_run: &TaskRun) -> SchedulerResult<TaskRun> {
            let mut task_runs = self.task_runs.lock().unwrap();
            let mut new_task_run = task_run.clone();
            new_task_run.id = (task_runs.len() + 1) as i64;
            task_runs.insert(new_task_run.id, new_task_run.clone());
            Ok(new_task_run)
        }

        async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            Ok(task_runs.get(&id).cloned())
        }

        async fn update(&self, task_run: &TaskRun) -> SchedulerResult<()> {
            let mut task_runs = self.task_runs.lock().unwrap();
            task_runs.insert(task_run.id, task_run.clone());
            Ok(())
        }

        async fn delete(&self, id: i64) -> SchedulerResult<()> {
            let mut task_runs = self.task_runs.lock().unwrap();
            task_runs.remove(&id);
            Ok(())
        }

        async fn get_by_task_id(&self, task_id: i64) -> SchedulerResult<Vec<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            Ok(task_runs
                .values()
                .filter(|tr| tr.task_id == task_id)
                .cloned()
                .collect())
        }

        async fn get_by_worker_id(&self, _worker_id: &str) -> SchedulerResult<Vec<TaskRun>> {
            Ok(vec![])
        }

        async fn get_by_status(&self, status: TaskRunStatus) -> SchedulerResult<Vec<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            Ok(task_runs
                .values()
                .filter(|tr| tr.status == status)
                .cloned()
                .collect())
        }

        async fn get_pending_runs(&self, _limit: Option<i64>) -> SchedulerResult<Vec<TaskRun>> {
            Ok(vec![])
        }

        async fn get_running_runs(&self) -> SchedulerResult<Vec<TaskRun>> {
            Ok(vec![])
        }

        async fn get_timeout_runs(&self, _timeout_seconds: i64) -> SchedulerResult<Vec<TaskRun>> {
            Ok(vec![])
        }

        async fn update_status(
            &self,
            _id: i64,
            _status: TaskRunStatus,
            _reason: Option<&str>,
        ) -> SchedulerResult<()> {
            Ok(())
        }

        async fn update_result(
            &self,
            _id: i64,
            _result: Option<&str>,
            _error: Option<&str>,
        ) -> SchedulerResult<()> {
            Ok(())
        }

        async fn cleanup_old_runs(&self, _keep_days: i32) -> SchedulerResult<u64> {
            Ok(0)
        }

        async fn get_recent_runs(&self, _task_id: i64, _limit: i64) -> SchedulerResult<Vec<TaskRun>> {
            Ok(vec![])
        }

        async fn get_execution_stats(
            &self,
            task_id: i64,
            _days: i32,
        ) -> SchedulerResult<scheduler_core::TaskExecutionStats> {
            Ok(scheduler_core::TaskExecutionStats {
                task_id,
                total_runs: 0,
                successful_runs: 0,
                failed_runs: 0,
                timeout_runs: 0,
                average_execution_time_ms: None,
                success_rate: 0.0,
                last_execution: None,
            })
        }

        async fn batch_update_status(&self, _ids: &[i64], _status: TaskRunStatus) -> SchedulerResult<()> {
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    pub struct MockWorkerRepository {
        workers: Arc<Mutex<HashMap<String, WorkerInfo>>>,
    }

    impl MockWorkerRepository {
        pub fn new() -> Self {
            Self {
                workers: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl WorkerRepository for MockWorkerRepository {
        async fn register(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
            let mut workers = self.workers.lock().unwrap();
            workers.insert(worker.id.clone(), worker.clone());
            Ok(())
        }

        async fn unregister(&self, worker_id: &str) -> SchedulerResult<()> {
            let mut workers = self.workers.lock().unwrap();
            workers.remove(worker_id);
            Ok(())
        }

        async fn get_by_id(&self, worker_id: &str) -> SchedulerResult<Option<WorkerInfo>> {
            let workers = self.workers.lock().unwrap();
            Ok(workers.get(worker_id).cloned())
        }

        async fn update(&self, worker: &WorkerInfo) -> SchedulerResult<()> {
            let mut workers = self.workers.lock().unwrap();
            workers.insert(worker.id.clone(), worker.clone());
            Ok(())
        }

        async fn list(&self) -> SchedulerResult<Vec<WorkerInfo>> {
            let workers = self.workers.lock().unwrap();
            Ok(workers.values().cloned().collect())
        }

        async fn get_alive_workers(&self) -> SchedulerResult<Vec<WorkerInfo>> {
            Ok(vec![])
        }

        async fn get_workers_by_task_type(&self, _task_type: &str) -> SchedulerResult<Vec<WorkerInfo>> {
            Ok(vec![])
        }

        async fn update_heartbeat(
            &self,
            _worker_id: &str,
            _timestamp: DateTime<Utc>,
            _load: i32,
        ) -> SchedulerResult<()> {
            Ok(())
        }

        async fn update_status(&self, _worker_id: &str, _status: WorkerStatus) -> SchedulerResult<()> {
            Ok(())
        }

        async fn get_timeout_workers(&self, _timeout_seconds: i64) -> SchedulerResult<Vec<WorkerInfo>> {
            Ok(vec![])
        }

        async fn cleanup_offline_workers(&self, _offline_seconds: i64) -> SchedulerResult<u64> {
            Ok(0)
        }

        async fn get_worker_load_stats(
            &self,
        ) -> SchedulerResult<Vec<scheduler_core::traits::repository::WorkerLoadStats>> {
            Ok(vec![])
        }

        async fn batch_update_status(
            &self,
            _worker_ids: &[String],
            _status: WorkerStatus,
        ) -> SchedulerResult<()> {
            Ok(())
        }
    }

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
