#[cfg(test)]
mod dependency_checker_tests {
    use async_trait::async_trait;
    use chrono::Utc;
    use scheduler_core::models::{TaskRun, TaskRunStatus, TaskStatus};
    use scheduler_core::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use scheduler_dispatcher::dependency_checker::*;

    // Mock TaskRepository for testing
    #[derive(Debug, Clone)]
    struct MockTaskRepository {
        tasks: Arc<Mutex<HashMap<i64, Task>>>,
    }

    impl MockTaskRepository {
        fn new() -> Self {
            Self {
                tasks: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn add_task(&self, task: Task) {
            self.tasks.lock().unwrap().insert(task.id, task);
        }
    }

    #[async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn create(&self, task: &Task) -> Result<Task> {
            let mut tasks = self.tasks.lock().unwrap();
            let mut new_task = task.clone();
            new_task.id = (tasks.len() + 1) as i64;
            tasks.insert(new_task.id, new_task.clone());
            Ok(new_task)
        }

        async fn get_by_id(&self, id: i64) -> Result<Option<Task>> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks.get(&id).cloned())
        }

        async fn get_by_name(&self, name: &str) -> Result<Option<Task>> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks.values().find(|t| t.name == name).cloned())
        }

        async fn update(&self, task: &Task) -> Result<()> {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(task.id, task.clone());
            Ok(())
        }

        async fn delete(&self, id: i64) -> Result<()> {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.remove(&id);
            Ok(())
        }

        async fn list(&self, _filter: &scheduler_core::models::TaskFilter) -> Result<Vec<Task>> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks.values().cloned().collect())
        }

        async fn get_active_tasks(&self) -> Result<Vec<Task>> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks
                .values()
                .filter(|t| t.status == TaskStatus::Active)
                .cloned()
                .collect())
        }

        async fn get_schedulable_tasks(
            &self,
            _current_time: chrono::DateTime<Utc>,
        ) -> Result<Vec<Task>> {
            self.get_active_tasks().await
        }

        async fn check_dependencies(&self, _task_id: i64) -> Result<bool> {
            Ok(true)
        }

        async fn get_dependencies(&self, _task_id: i64) -> Result<Vec<Task>> {
            Ok(vec![])
        }

        async fn batch_update_status(&self, _task_ids: &[i64], _status: TaskStatus) -> Result<()> {
            Ok(())
        }
    }

    // Mock TaskRunRepository for testing
    #[derive(Debug, Clone)]
    struct MockTaskRunRepository {
        task_runs: Arc<Mutex<HashMap<i64, Vec<TaskRun>>>>,
        next_id: Arc<Mutex<i64>>,
    }

    impl MockTaskRunRepository {
        fn new() -> Self {
            Self {
                task_runs: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(Mutex::new(1)),
            }
        }

        fn add_task_run(&self, task_run: TaskRun) {
            let mut runs = self.task_runs.lock().unwrap();
            runs.entry(task_run.task_id).or_default().push(task_run);
        }
    }

    #[async_trait]
    impl TaskRunRepository for MockTaskRunRepository {
        async fn create(&self, task_run: &TaskRun) -> Result<TaskRun> {
            let mut runs = self.task_runs.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let mut new_run = task_run.clone();
            new_run.id = *next_id;
            *next_id += 1;
            runs.entry(new_run.task_id)
                .or_default()
                .push(new_run.clone());
            Ok(new_run)
        }

        async fn get_by_id(&self, id: i64) -> Result<Option<TaskRun>> {
            let runs = self.task_runs.lock().unwrap();
            for task_runs in runs.values() {
                if let Some(run) = task_runs.iter().find(|r| r.id == id) {
                    return Ok(Some(run.clone()));
                }
            }
            Ok(None)
        }

        async fn update(&self, task_run: &TaskRun) -> Result<()> {
            let mut runs = self.task_runs.lock().unwrap();
            if let Some(task_runs) = runs.get_mut(&task_run.task_id) {
                if let Some(existing_run) = task_runs.iter_mut().find(|r| r.id == task_run.id) {
                    *existing_run = task_run.clone();
                }
            }
            Ok(())
        }

        async fn delete(&self, id: i64) -> Result<()> {
            let mut runs = self.task_runs.lock().unwrap();
            for task_runs in runs.values_mut() {
                task_runs.retain(|r| r.id != id);
            }
            Ok(())
        }

        async fn get_by_task_id(&self, task_id: i64) -> Result<Vec<TaskRun>> {
            let runs = self.task_runs.lock().unwrap();
            Ok(runs.get(&task_id).cloned().unwrap_or_default())
        }

        async fn get_by_worker_id(&self, worker_id: &str) -> Result<Vec<TaskRun>> {
            let runs = self.task_runs.lock().unwrap();
            let mut result = Vec::new();
            for task_runs in runs.values() {
                for run in task_runs {
                    if run.worker_id.as_ref() == Some(&worker_id.to_string()) {
                        result.push(run.clone());
                    }
                }
            }
            Ok(result)
        }

        async fn get_by_status(&self, status: TaskRunStatus) -> Result<Vec<TaskRun>> {
            let runs = self.task_runs.lock().unwrap();
            let mut result = Vec::new();
            for task_runs in runs.values() {
                for run in task_runs {
                    if run.status == status {
                        result.push(run.clone());
                    }
                }
            }
            Ok(result)
        }

        async fn get_pending_runs(&self, _limit: Option<i64>) -> Result<Vec<TaskRun>> {
            self.get_by_status(TaskRunStatus::Pending).await
        }

        async fn get_running_runs(&self) -> Result<Vec<TaskRun>> {
            let runs = self.task_runs.lock().unwrap();
            let mut result = Vec::new();
            for task_runs in runs.values() {
                for run in task_runs {
                    if run.is_running() {
                        result.push(run.clone());
                    }
                }
            }
            Ok(result)
        }

        async fn get_timeout_runs(&self, _timeout_seconds: i64) -> Result<Vec<TaskRun>> {
            Ok(vec![])
        }

        async fn update_status(
            &self,
            id: i64,
            status: TaskRunStatus,
            worker_id: Option<&str>,
        ) -> Result<()> {
            let mut runs = self.task_runs.lock().unwrap();
            for task_runs in runs.values_mut() {
                if let Some(run) = task_runs.iter_mut().find(|r| r.id == id) {
                    run.status = status;
                    if let Some(worker) = worker_id {
                        run.worker_id = Some(worker.to_string());
                    }
                }
            }
            Ok(())
        }

        async fn update_result(
            &self,
            id: i64,
            result: Option<&str>,
            error_message: Option<&str>,
        ) -> Result<()> {
            let mut runs = self.task_runs.lock().unwrap();
            for task_runs in runs.values_mut() {
                if let Some(run) = task_runs.iter_mut().find(|r| r.id == id) {
                    run.result = result.map(|s| s.to_string());
                    run.error_message = error_message.map(|s| s.to_string());
                }
            }
            Ok(())
        }

        async fn get_recent_runs(&self, task_id: i64, limit: i64) -> Result<Vec<TaskRun>> {
            let runs = self.task_runs.lock().unwrap();
            if let Some(task_runs) = runs.get(&task_id) {
                let mut sorted_runs = task_runs.clone();
                sorted_runs.sort_by(|a, b| b.scheduled_at.cmp(&a.scheduled_at));
                sorted_runs.truncate(limit as usize);
                Ok(sorted_runs)
            } else {
                Ok(vec![])
            }
        }

        async fn get_execution_stats(
            &self,
            _task_id: i64,
            _days: i32,
        ) -> Result<scheduler_core::traits::repository::TaskExecutionStats> {
            todo!()
        }

        async fn cleanup_old_runs(&self, _days: i32) -> Result<u64> {
            Ok(0)
        }

        async fn batch_update_status(
            &self,
            _run_ids: &[i64],
            _status: TaskRunStatus,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_dependency_checker_creation() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());

        let checker = DependencyChecker::new(task_repo, task_run_repo);

        // 测试创建成功
        assert!(std::ptr::addr_of!(checker).is_aligned());
    }

    #[tokio::test]
    async fn test_check_dependencies_no_deps() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo, task_run_repo);

        let task = Task {
            id: 1,
            name: "no_deps_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![], // 无依赖
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = checker.check_dependencies(&task).await.unwrap();
        assert!(result.can_execute);
        assert!(result.blocking_dependencies.is_empty());
        assert!(result.reason.is_none());
    }

    #[tokio::test]
    async fn test_check_dependencies_with_successful_deps() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo.clone(), task_run_repo.clone());

        // 创建依赖任务的成功执行记录
        let successful_run = TaskRun {
            id: 1,
            task_id: 1,
            status: TaskRunStatus::Completed,
            worker_id: Some("worker-001".to_string()),
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            result: Some("success".to_string()),
            error_message: None,
            created_at: Utc::now(),
        };

        task_run_repo.add_task_run(successful_run);

        let task = Task {
            id: 2,
            name: "with_deps_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![1], // 依赖任务1
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = checker.check_dependencies(&task).await.unwrap();
        assert!(result.can_execute);
        assert!(result.blocking_dependencies.is_empty());
        assert!(result.reason.is_none());
    }

    #[tokio::test]
    async fn test_check_dependencies_with_failed_deps() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo.clone(), task_run_repo.clone());

        // 创建依赖任务的失败执行记录
        let failed_run = TaskRun {
            id: 1,
            task_id: 1,
            status: TaskRunStatus::Failed,
            worker_id: Some("worker-001".to_string()),
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            result: None,
            error_message: Some("execution failed".to_string()),
            created_at: Utc::now(),
        };

        task_run_repo.add_task_run(failed_run);

        let task = Task {
            id: 2,
            name: "with_deps_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![1], // 依赖任务1
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = checker.check_dependencies(&task).await.unwrap();
        assert!(!result.can_execute);
        assert_eq!(result.blocking_dependencies, vec![1]);
        assert!(result.reason.is_some());
    }

    #[tokio::test]
    async fn test_validate_dependencies_self_dependency() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo, task_run_repo);

        // 测试自依赖
        let result = checker.validate_dependencies(1, &[1]).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SchedulerError::CircularDependency
        ));
    }

    #[tokio::test]
    async fn test_validate_dependencies_nonexistent_task() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo, task_run_repo);

        // 测试依赖不存在的任务
        let result = checker.validate_dependencies(1, &[999]).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SchedulerError::TaskNotFound { id: 999 }
        ));
    }

    #[tokio::test]
    async fn test_detect_circular_dependency() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo.clone(), task_run_repo);

        // 创建任务形成循环依赖: 1 -> 2 -> 3 -> 1
        let task1 = Task {
            id: 1,
            name: "task1".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![3], // 依赖任务3
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let task2 = Task {
            id: 2,
            name: "task2".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![1], // 依赖任务1
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let task3 = Task {
            id: 3,
            name: "task3".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![], // 暂时无依赖
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(task1);
        task_repo.add_task(task2);
        task_repo.add_task(task3);

        // 测试添加会形成循环的依赖
        let has_cycle = checker.detect_circular_dependency(3, &[2]).await.unwrap();
        assert!(has_cycle);

        // 测试不会形成循环的依赖
        let has_cycle = checker.detect_circular_dependency(3, &[]).await.unwrap();
        assert!(!has_cycle);
    }

    #[tokio::test]
    async fn test_get_transitive_dependencies() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo.clone(), task_run_repo);

        // 创建任务依赖链: 4 -> 3 -> 2 -> 1
        let task1 = Task {
            id: 1,
            name: "task1".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![], // 无依赖
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let task2 = Task {
            id: 2,
            name: "task2".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![1], // 依赖任务1
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let task3 = Task {
            id: 3,
            name: "task3".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![2], // 依赖任务2
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let task4 = Task {
            id: 4,
            name: "task4".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![3], // 依赖任务3
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(task1);
        task_repo.add_task(task2);
        task_repo.add_task(task3);
        task_repo.add_task(task4);

        let transitive_deps = checker.get_transitive_dependencies(4).await.unwrap();

        // 任务4的传递依赖应该包含3, 2, 1
        assert_eq!(transitive_deps.len(), 3);
        assert!(transitive_deps.contains(&3));
        assert!(transitive_deps.contains(&2));
        assert!(transitive_deps.contains(&1));
    }
}
