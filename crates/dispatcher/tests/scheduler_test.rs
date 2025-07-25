#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::{DateTime, Timelike, Utc};
    use scheduler_core::models::*;
    use scheduler_core::traits::*;
    use scheduler_core::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use scheduler_dispatcher::scheduler::*;

    // Mock MessageQueue for testing
    #[derive(Debug, Clone)]
    struct MockMessageQueue {
        queues: Arc<Mutex<HashMap<String, Vec<Message>>>>,
    }

    impl MockMessageQueue {
        fn new() -> Self {
            Self {
                queues: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn get_queue_messages(&self, queue: &str) -> Vec<Message> {
            self.queues
                .lock()
                .unwrap()
                .get(queue)
                .cloned()
                .unwrap_or_default()
        }
    }

    #[async_trait]
    impl MessageQueue for MockMessageQueue {
        async fn publish_message(&self, queue: &str, message: &Message) -> Result<()> {
            let mut queues = self.queues.lock().unwrap();
            queues
                .entry(queue.to_string())
                .or_default()
                .push(message.clone());
            Ok(())
        }

        async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>> {
            let mut queues = self.queues.lock().unwrap();
            let messages = queues.remove(queue).unwrap_or_default();
            Ok(messages)
        }

        async fn ack_message(&self, _message_id: &str) -> Result<()> {
            Ok(())
        }

        async fn nack_message(&self, _message_id: &str, _requeue: bool) -> Result<()> {
            Ok(())
        }

        async fn create_queue(&self, queue: &str, _durable: bool) -> Result<()> {
            let mut queues = self.queues.lock().unwrap();
            queues.entry(queue.to_string()).or_default();
            Ok(())
        }

        async fn delete_queue(&self, queue: &str) -> Result<()> {
            let mut queues = self.queues.lock().unwrap();
            queues.remove(queue);
            Ok(())
        }

        async fn get_queue_size(&self, queue: &str) -> Result<u32> {
            let queues = self.queues.lock().unwrap();
            let size = queues.get(queue).map(|q| q.len()).unwrap_or(0) as u32;
            Ok(size)
        }

        async fn purge_queue(&self, queue: &str) -> Result<()> {
            let mut queues = self.queues.lock().unwrap();
            if let Some(queue_messages) = queues.get_mut(queue) {
                queue_messages.clear();
            }
            Ok(())
        }
    }

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

        async fn get_schedulable_tasks(&self, _current_time: DateTime<Utc>) -> Result<Vec<Task>> {
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
        task_runs: Arc<Mutex<HashMap<i64, TaskRun>>>,
        next_id: Arc<Mutex<i64>>,
    }

    impl MockTaskRunRepository {
        fn new() -> Self {
            Self {
                task_runs: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(Mutex::new(1)),
            }
        }
    }

    #[async_trait]
    impl TaskRunRepository for MockTaskRunRepository {
        async fn create(&self, task_run: &TaskRun) -> Result<TaskRun> {
            let mut task_runs = self.task_runs.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();

            let mut new_run = task_run.clone();
            new_run.id = *next_id;
            *next_id += 1;

            task_runs.insert(new_run.id, new_run.clone());
            Ok(new_run)
        }

        async fn get_by_id(&self, id: i64) -> Result<Option<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            Ok(task_runs.get(&id).cloned())
        }

        async fn update(&self, task_run: &TaskRun) -> Result<()> {
            let mut task_runs = self.task_runs.lock().unwrap();
            task_runs.insert(task_run.id, task_run.clone());
            Ok(())
        }

        async fn delete(&self, id: i64) -> Result<()> {
            let mut task_runs = self.task_runs.lock().unwrap();
            task_runs.remove(&id);
            Ok(())
        }

        async fn get_by_task_id(&self, task_id: i64) -> Result<Vec<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            Ok(task_runs
                .values()
                .filter(|r| r.task_id == task_id)
                .cloned()
                .collect())
        }

        async fn get_by_worker_id(&self, worker_id: &str) -> Result<Vec<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            Ok(task_runs
                .values()
                .filter(|r| r.worker_id.as_ref() == Some(&worker_id.to_string()))
                .cloned()
                .collect())
        }

        async fn get_by_status(&self, status: TaskRunStatus) -> Result<Vec<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            Ok(task_runs
                .values()
                .filter(|r| r.status == status)
                .cloned()
                .collect())
        }

        async fn get_pending_runs(&self, _limit: Option<i64>) -> Result<Vec<TaskRun>> {
            self.get_by_status(TaskRunStatus::Pending).await
        }

        async fn get_running_runs(&self) -> Result<Vec<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            Ok(task_runs
                .values()
                .filter(|r| r.is_running())
                .cloned()
                .collect())
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
            let mut task_runs = self.task_runs.lock().unwrap();
            if let Some(run) = task_runs.get_mut(&id) {
                run.status = status;
                if let Some(worker) = worker_id {
                    run.worker_id = Some(worker.to_string());
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
            let mut task_runs = self.task_runs.lock().unwrap();
            if let Some(run) = task_runs.get_mut(&id) {
                run.result = result.map(|s| s.to_string());
                run.error_message = error_message.map(|s| s.to_string());
            }
            Ok(())
        }

        async fn get_recent_runs(&self, task_id: i64, limit: i64) -> Result<Vec<TaskRun>> {
            let task_runs = self.task_runs.lock().unwrap();
            let mut runs: Vec<_> = task_runs
                .values()
                .filter(|r| r.task_id == task_id)
                .cloned()
                .collect();

            runs.sort_by(|a, b| b.scheduled_at.cmp(&a.scheduled_at));
            runs.truncate(limit as usize);
            Ok(runs)
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
    async fn test_task_scheduler_creation() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo,
            task_run_repo,
            message_queue,
            "test_queue".to_string(),
        );

        // 测试调度器创建成功
        assert_eq!(scheduler.task_queue_name, "test_queue");
    }

    #[tokio::test]
    async fn test_should_schedule_task() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo.clone(),
            task_run_repo,
            message_queue,
            "test_queue".to_string(),
        );

        // 创建一个每分钟执行的任务
        let task = Task {
            id: 1,
            name: "test_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * * *".to_string(), // 每分钟执行 (6字段格式: 秒 分 时 日 月 周)
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(task.clone());

        // 测试活跃任务应该被考虑调度
        let should_schedule = scheduler.should_schedule_task(&task).await.unwrap();
        // 由于没有历史执行记录，且CRON表达式有效，应该返回true
        assert!(should_schedule);

        // 测试非活跃任务不应该被调度
        let mut inactive_task = task.clone();
        inactive_task.status = TaskStatus::Inactive;
        let should_schedule = scheduler
            .should_schedule_task(&inactive_task)
            .await
            .unwrap();
        assert!(!should_schedule);
    }

    #[tokio::test]
    async fn test_check_dependencies() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo.clone(),
            task_run_repo.clone(),
            message_queue,
            "test_queue".to_string(),
        );

        // 创建没有依赖的任务
        let task_no_deps = Task {
            id: 1,
            name: "no_deps_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 测试没有依赖的任务
        let deps_ok = scheduler.check_dependencies(&task_no_deps).await.unwrap();
        assert!(deps_ok);

        // 创建有依赖的任务
        let task_with_deps = Task {
            id: 2,
            name: "with_deps_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * * *".to_string(), // 6字段格式: 秒 分 时 日 月 周
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![1], // 依赖任务1
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 测试有依赖但依赖任务未执行的情况
        let deps_ok = scheduler.check_dependencies(&task_with_deps).await.unwrap();
        assert!(!deps_ok); // 依赖任务未执行，应该返回false

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

        task_run_repo.create(&successful_run).await.unwrap();

        // 现在依赖应该满足
        let deps_ok = scheduler.check_dependencies(&task_with_deps).await.unwrap();
        assert!(deps_ok);
    }

    #[tokio::test]
    async fn test_create_task_run() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo,
            task_run_repo.clone(),
            message_queue,
            "test_queue".to_string(),
        );

        let task = Task {
            id: 1,
            name: "test_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * * *".to_string(), // 6字段格式: 秒 分 时 日 月 周
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let task_run = scheduler.create_task_run(&task).await.unwrap();

        assert_eq!(task_run.task_id, task.id);
        assert_eq!(task_run.status, TaskRunStatus::Pending);
        assert_eq!(task_run.retry_count, 0);

        // 验证任务运行实例已保存到仓储
        let saved_run = task_run_repo.get_by_id(task_run.id).await.unwrap();
        assert!(saved_run.is_some());
    }

    #[tokio::test]
    async fn test_dispatch_to_queue() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo.clone(),
            task_run_repo.clone(),
            message_queue.clone(),
            "test_queue".to_string(),
        );

        let task = Task {
            id: 1,
            name: "test_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * *".to_string(),
            parameters: json!({"command": "echo hello"}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(task.clone());

        let task_run = TaskRun {
            id: 1,
            task_id: 1,
            status: TaskRunStatus::Pending,
            worker_id: None,
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: Utc::now(),
        };

        let created_run = task_run_repo.create(&task_run).await.unwrap();

        // 分发任务到队列
        scheduler.dispatch_to_queue(&created_run).await.unwrap();

        // 验证消息已发送到队列
        let messages = message_queue.get_queue_messages("test_queue");
        assert_eq!(messages.len(), 1);

        // 验证任务运行状态已更新为已分发
        let updated_run = task_run_repo
            .get_by_id(created_run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated_run.status, TaskRunStatus::Dispatched);
    }

    #[tokio::test]
    async fn test_scan_and_schedule() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo.clone(),
            task_run_repo.clone(),
            message_queue.clone(),
            "test_queue".to_string(),
        );

        // 创建一个应该被调度的任务
        let task = Task {
            id: 1,
            name: "schedulable_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * * *".to_string(), // 每分钟执行 (6字段格式: 秒 分 时 日 月 周)
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(task);

        // 执行调度
        let scheduled_runs = scheduler.scan_and_schedule().await.unwrap();

        // 验证有任务被调度
        assert_eq!(scheduled_runs.len(), 1);
        assert_eq!(scheduled_runs[0].task_id, 1);

        // 验证消息已发送到队列
        let messages = message_queue.get_queue_messages("test_queue");
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_detect_overdue_tasks() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo.clone(),
            task_run_repo.clone(),
            message_queue,
            "test_queue".to_string(),
        );

        // 创建一个每分钟执行的任务
        let task = Task {
            id: 1,
            name: "overdue_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 * * * * *".to_string(), // 每分钟执行
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(task);

        // 创建一个很久之前的执行记录
        let old_run = TaskRun {
            id: 1,
            task_id: 1,
            status: TaskRunStatus::Completed,
            worker_id: Some("worker-001".to_string()),
            retry_count: 0,
            shard_index: None,
            shard_total: None,
            scheduled_at: Utc::now() - chrono::Duration::minutes(10), // 10分钟前
            started_at: Some(Utc::now() - chrono::Duration::minutes(10)),
            completed_at: Some(Utc::now() - chrono::Duration::minutes(9)),
            result: Some("success".to_string()),
            error_message: None,
            created_at: Utc::now() - chrono::Duration::minutes(10),
        };

        task_run_repo.create(&old_run).await.unwrap();

        // 检测过期任务（宽限期2分钟）
        let overdue_tasks = scheduler.detect_overdue_tasks(2).await.unwrap();

        // 应该检测到过期任务
        assert_eq!(overdue_tasks.len(), 1);
        assert_eq!(overdue_tasks[0].id, 1);
    }

    #[tokio::test]
    async fn test_get_next_execution_time() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo.clone(),
            task_run_repo,
            message_queue,
            "test_queue".to_string(),
        );

        // 创建一个每小时执行的任务
        let task = Task {
            id: 1,
            name: "hourly_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 * * * *".to_string(), // 每小时执行
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(task);

        // 获取下次执行时间
        let next_time = scheduler.get_next_execution_time(1).await.unwrap();

        assert!(next_time.is_some());
        let next = next_time.unwrap();
        assert_eq!(next.minute(), 0);
        assert_eq!(next.second(), 0);
    }

    #[tokio::test]
    async fn test_validate_task_schedule() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let message_queue = Arc::new(MockMessageQueue::new());

        let scheduler = TaskScheduler::new(
            task_repo.clone(),
            task_run_repo,
            message_queue,
            "test_queue".to_string(),
        );

        // 创建一个有效CRON表达式的任务
        let valid_task = Task {
            id: 1,
            name: "valid_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "0 0 * * * *".to_string(), // 有效的CRON表达式
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(valid_task);

        // 创建一个无效CRON表达式的任务
        let invalid_task = Task {
            id: 2,
            name: "invalid_task".to_string(),
            task_type: "shell".to_string(),
            schedule: "invalid cron".to_string(), // 无效的CRON表达式
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        task_repo.add_task(invalid_task);

        // 验证有效任务
        let is_valid = scheduler.validate_task_schedule(1).await.unwrap();
        assert!(is_valid);

        // 验证无效任务
        let is_invalid = scheduler.validate_task_schedule(2).await.unwrap();
        assert!(!is_invalid);
    }
}
