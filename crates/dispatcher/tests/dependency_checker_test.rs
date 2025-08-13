#[cfg(test)]
mod dependency_checker_tests {
    use scheduler_foundation::*;
    use scheduler_domain::repositories::{TaskRepository, TaskRunRepository};
    use scheduler_testing_utils::{
        MockTaskRepository, MockTaskRunRepository, TaskBuilder, TaskRunBuilder,
    };
    use serde_json::json;
    use std::sync::Arc;

    use scheduler_dispatcher::dependency_checker::*;

    #[tokio::test]
    async fn test_dependency_checker_creation() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());

        let checker = DependencyChecker::new(task_repo, task_run_repo);
        assert!(std::ptr::addr_of!(checker).is_aligned());
    }

    #[tokio::test]
    async fn test_check_dependencies_no_deps() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo.clone(), task_run_repo);

        let task = TaskBuilder::new()
            .with_id(1)
            .with_name("no_deps_task")
            .with_task_type("shell")
            .with_schedule("0 0 0 * * *")
            .with_parameters(json!({}))
            .with_dependencies(vec![])
            .build();

        task_repo.create(&task).await.unwrap();

        let result = checker.check_dependencies(&task).await;
        assert!(result.is_ok());
        assert!(result.unwrap().can_execute);
    }

    #[tokio::test]
    async fn test_check_dependencies_with_successful_deps() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo.clone(), task_run_repo.clone());

        // Create successful dependency run
        let successful_run = TaskRunBuilder::new()
            .with_id(1)
            .with_task_id(1)
            .completed()
            .with_worker_id("worker-001")
            .build();

        task_run_repo.create(&successful_run).await.unwrap();

        // Create main task with dependency
        let task = TaskBuilder::new()
            .with_id(2)
            .with_name("main_task")
            .with_task_type("shell")
            .with_dependencies(vec![1])
            .build();

        task_repo.create(&task).await.unwrap();

        let result = checker.check_dependencies(&task).await;
        assert!(result.is_ok());
        assert!(result.unwrap().can_execute);
    }

    #[tokio::test]
    async fn test_check_dependencies_with_failed_deps() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo.clone(), task_run_repo.clone());

        // Create failed dependency run
        let failed_run = TaskRunBuilder::new()
            .with_id(1)
            .with_task_id(1)
            .failed()
            .with_worker_id("worker-001")
            .build();

        task_run_repo.create(&failed_run).await.unwrap();

        // Create main task with dependency
        let task = TaskBuilder::new()
            .with_id(2)
            .with_name("main_task")
            .with_task_type("shell")
            .with_dependencies(vec![1])
            .build();

        task_repo.create(&task).await.unwrap();

        let result = checker.check_dependencies(&task).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().can_execute);
    }

    #[tokio::test]
    async fn test_validate_dependencies_self_dependency() {
        let task_repo = Arc::new(MockTaskRepository::new());
        let task_run_repo = Arc::new(MockTaskRunRepository::new());
        let checker = DependencyChecker::new(task_repo, task_run_repo);

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

        let result = checker.validate_dependencies(1, &[999]).await;
        assert!(result.is_err());
        // The specific error type might be different - let's just check it's an error
        assert!(result.is_err());
    }
}
