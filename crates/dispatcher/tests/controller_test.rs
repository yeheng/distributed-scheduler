#[cfg(test)]
mod tests {
    use scheduler_dispatcher::controller::*;
    #[test]
    fn test_task_status_summary_creation() {
        let summary = TaskStatusSummary::default();
        assert_eq!(summary.total(), 0);
        assert_eq!(summary.active(), 0);
        assert_eq!(summary.finished(), 0);
    }

    #[test]
    fn test_task_status_summary_calculations() {
        let mut summary = TaskStatusSummary::default();
        summary.pending = 5;
        summary.running = 3;
        summary.completed = 10;
        summary.failed = 2;

        assert_eq!(summary.total(), 20);
        assert_eq!(summary.active(), 8); // pending + running
        assert_eq!(summary.finished(), 12); // completed + failed + cancelled (0)
    }

    #[test]
    fn test_task_status_summary_clone() {
        let mut summary = TaskStatusSummary::default();
        summary.pending = 1;
        summary.running = 2;
        summary.completed = 3;

        let cloned = summary.clone();
        assert_eq!(cloned.pending, 1);
        assert_eq!(cloned.running, 2);
        assert_eq!(cloned.completed, 3);
        assert_eq!(cloned.total(), summary.total());
    }

    #[test]
    fn test_task_status_summary_debug() {
        let summary = TaskStatusSummary::default();
        let debug_str = format!("{:?}", summary);
        assert!(debug_str.contains("TaskStatusSummary"));
    }

    #[tokio::test]
    async fn test_controller_lifecycle() {
        // Test that we can create and drop a controller without panicking
        let task_repo = scheduler_testing_utils::mocks::MockTaskRepository::new();
        let task_run_repo = scheduler_testing_utils::mocks::MockTaskRunRepository::new();
        let message_queue = scheduler_testing_utils::mocks::MockMessageQueue::new();

        let _controller = TaskController::new(
            std::sync::Arc::new(task_repo),
            std::sync::Arc::new(task_run_repo),
            std::sync::Arc::new(message_queue),
            "test_control_queue".to_string(),
        );

        // If we get here without panicking, the test passes
    }
}
