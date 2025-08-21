#[cfg(test)]
mod tests {
    use scheduler_dispatcher::state_listener::*;

    #[test]
    fn test_state_listener_creation() {
        let task_run_repo = scheduler_testing_utils::mocks::MockTaskRunRepository::new();
        let worker_repo = scheduler_testing_utils::mocks::MockWorkerRepository::new();
        let message_queue = scheduler_testing_utils::mocks::MockMessageQueue::new();

        let _listener = StateListener::new(
            std::sync::Arc::new(task_run_repo),
            std::sync::Arc::new(worker_repo),
            std::sync::Arc::new(message_queue),
            "status_queue".to_string(),
            "heartbeat_queue".to_string(),
        );

        // Test passes if we can create without panicking
    }

    #[tokio::test]
    async fn test_state_listener_start_stop() {
        let task_run_repo = scheduler_testing_utils::mocks::MockTaskRunRepository::new();
        let worker_repo = scheduler_testing_utils::mocks::MockWorkerRepository::new();
        let message_queue = scheduler_testing_utils::mocks::MockMessageQueue::new();

        let listener = StateListener::new(
            std::sync::Arc::new(task_run_repo),
            std::sync::Arc::new(worker_repo),
            std::sync::Arc::new(message_queue),
            "status_queue".to_string(),
            "heartbeat_queue".to_string(),
        );

        // Test that we can start and stop without panicking
        // Note: In a real test, we'd need to handle the async listening loop
        let _result = listener.stop().await;
        assert!(!listener.is_running().await);
    }

    #[tokio::test]
    async fn test_state_listener_running_state() {
        let task_run_repo = scheduler_testing_utils::mocks::MockTaskRunRepository::new();
        let worker_repo = scheduler_testing_utils::mocks::MockWorkerRepository::new();
        let message_queue = scheduler_testing_utils::mocks::MockMessageQueue::new();

        let listener = StateListener::new(
            std::sync::Arc::new(task_run_repo),
            std::sync::Arc::new(worker_repo),
            std::sync::Arc::new(message_queue),
            "status_queue".to_string(),
            "heartbeat_queue".to_string(),
        );

        // Test initial state
        assert!(!listener.is_running().await);

        // Test that stop works even when not running
        let _result = listener.stop().await;
        assert!(!listener.is_running().await);
    }
}
