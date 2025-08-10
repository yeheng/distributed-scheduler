#[cfg(test)]
mod tests {
    use chrono::{Timelike, Utc};
    use scheduler_application::interfaces::scheduler::TaskSchedulerService;
    use scheduler_core::models::*;
    use scheduler_core::*;
    use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};
    use scheduler_domain::TaskStatus;
    use serde_json::json;
    use std::sync::Arc;

    use scheduler_dispatcher::scheduler::*;
    use scheduler_infrastructure::MetricsCollector;
    use scheduler_testing_utils::{
        MockMessageQueue, MockTaskRepository, MockTaskRunRepository, TaskBuilder, TaskRunBuilder,
    };

    fn create_test_metrics() -> Arc<MetricsCollector> {
        Arc::new(MetricsCollector::new().unwrap())
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
            create_test_metrics(),
        );
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
            create_test_metrics(),
        );

        let task = TaskBuilder::new()
            .with_id(1)
            .with_name("test_task")
            .with_task_type("shell")
            .with_schedule("0 * * * * *")
            .with_parameters(json!({}))
            .with_status(TaskStatus::Active)
            .build();

        task_repo.create(&task).await.unwrap();
        let should_schedule = scheduler.should_schedule_task(&task).await.unwrap();
        assert!(should_schedule);

        let inactive_task = TaskBuilder::new()
            .with_id(task.id)
            .with_name(&task.name)
            .with_task_type(&task.task_type)
            .with_schedule(&task.schedule)
            .with_parameters(task.parameters.clone())
            .inactive()
            .build();
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
            create_test_metrics(),
        );

        let task_no_deps = TaskBuilder::new()
            .with_id(1)
            .with_name("no_deps_task")
            .with_task_type("shell")
            .with_schedule("0 * * * *")
            .with_parameters(json!({}))
            .with_status(TaskStatus::Active)
            .build();

        let deps_ok = scheduler.check_dependencies(&task_no_deps).await.unwrap();
        assert!(deps_ok);

        let task_with_deps = TaskBuilder::new()
            .with_id(2)
            .with_name("with_deps_task")
            .with_task_type("shell")
            .with_schedule("0 * * * * *")
            .with_parameters(json!({}))
            .with_dependencies(vec![1])
            .with_status(TaskStatus::Active)
            .build();

        let deps_ok = scheduler.check_dependencies(&task_with_deps).await.unwrap();
        assert!(!deps_ok); // Dependency task has not run, should return false

        let successful_run = TaskRunBuilder::new()
            .with_task_id(1)
            .with_worker_id("worker-001")
            .completed()
            .with_result("success")
            .build();

        task_run_repo.create(&successful_run).await.unwrap();
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
            create_test_metrics(),
        );

        let task = TaskBuilder::new()
            .with_id(1)
            .with_name("test_task")
            .with_task_type("shell")
            .with_schedule("0 * * * * *")
            .with_parameters(json!({}))
            .with_status(TaskStatus::Active)
            .build();

        let task_run = scheduler.create_task_run(&task).await.unwrap();

        assert_eq!(task_run.task_id, task.id);
        assert_eq!(task_run.status, TaskRunStatus::Pending);
        assert_eq!(task_run.retry_count, 0);
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
            create_test_metrics(),
        );

        let task = TaskBuilder::new()
            .with_id(1)
            .with_name("test_task")
            .with_task_type("shell")
            .with_schedule("0 * * * *")
            .with_parameters(json!({"command": "echo hello"}))
            .with_status(TaskStatus::Active)
            .build();

        task_repo.create(&task).await.unwrap();

        let task_run = TaskRunBuilder::new()
            .with_id(1)
            .with_task_id(1)
            .with_status(TaskRunStatus::Pending)
            .build();

        let created_run = task_run_repo.create(&task_run).await.unwrap();
        scheduler.dispatch_to_queue(&created_run).await.unwrap();
        let messages = message_queue.get_queue_messages("test_queue");
        assert_eq!(messages.len(), 1);
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
            create_test_metrics(),
        );
        let task = TaskBuilder::new()
            .with_id(1)
            .with_name("schedulable_task")
            .with_task_type("shell")
            .with_schedule("0 * * * * *")
            .with_parameters(json!({}))
            .with_status(TaskStatus::Active)
            .build();

        task_repo.create(&task).await.unwrap();
        let scheduled_runs = scheduler.scan_and_schedule().await.unwrap();
        assert_eq!(scheduled_runs.len(), 1);
        assert_eq!(scheduled_runs[0].task_id, 1);
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
            create_test_metrics(),
        );
        let task = TaskBuilder::new()
            .with_id(1)
            .with_name("overdue_task")
            .with_task_type("shell")
            .with_schedule("0 * * * * *")
            .with_parameters(json!({}))
            .with_status(TaskStatus::Active)
            .build();

        task_repo.create(&task).await.unwrap();

        let old_run = TaskRunBuilder::new()
            .with_id(1)
            .with_task_id(1)
            .completed()
            .with_worker_id("worker-001")
            .with_result("success")
            .with_scheduled_at(Utc::now() - chrono::Duration::minutes(10))
            .build();

        task_run_repo.create(&old_run).await.unwrap();
        let overdue_tasks = scheduler.detect_overdue_tasks(2).await.unwrap();
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
            create_test_metrics(),
        );
        let task = TaskBuilder::new()
            .with_id(1)
            .with_name("hourly_task")
            .with_task_type("shell")
            .with_schedule("0 0 * * * *")
            .with_parameters(json!({}))
            .with_status(TaskStatus::Active)
            .build();

        task_repo.create(&task).await.unwrap();
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
            create_test_metrics(),
        );
        let valid_task = TaskBuilder::new()
            .with_id(1)
            .with_name("valid_task")
            .with_task_type("shell")
            .with_schedule("0 0 * * * *")
            .with_parameters(json!({}))
            .with_status(TaskStatus::Active)
            .build();

        task_repo.create(&valid_task).await.unwrap();
        let invalid_task = TaskBuilder::new()
            .with_id(2)
            .with_name("invalid_task")
            .with_task_type("shell")
            .with_schedule("invalid cron")
            .with_parameters(json!({}))
            .with_status(TaskStatus::Active)
            .build();

        task_repo.create(&invalid_task).await.unwrap();
        let is_valid = scheduler.validate_task_schedule(1).await.unwrap();
        assert!(is_valid);
        let is_invalid = scheduler.validate_task_schedule(2).await.unwrap();
        assert!(!is_invalid);
    }
}
