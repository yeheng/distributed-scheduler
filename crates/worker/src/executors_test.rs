#[cfg(test)]
mod tests {
    use crate::executors::MockTaskExecutor;
    use crate::{HttpExecutor, ShellExecutor};
    use scheduler_application::TaskExecutor;

    #[tokio::test]
    async fn test_shell_executor_cleanup() {
        let executor = ShellExecutor::with_cleanup_timeout(1000); // 1 second timeout for test

        // Call cleanup - should be a no-op but shouldn't fail
        let cleanup_result = executor.cleanup().await;
        assert!(
            cleanup_result.is_ok(),
            "ShellExecutor cleanup should succeed"
        );
    }

    #[tokio::test]
    async fn test_http_executor_cleanup() {
        let executor = HttpExecutor::new();

        // Call cleanup - should be a no-op but shouldn't fail
        let cleanup_result = executor.cleanup().await;
        assert!(
            cleanup_result.is_ok(),
            "HttpExecutor cleanup should succeed"
        );
    }

    #[tokio::test]
    async fn test_mock_executor_cleanup() {
        let executor = MockTaskExecutor::new("test".to_string(), true, 100);

        // Call cleanup - should be a no-op but shouldn't fail
        let cleanup_result = executor.cleanup().await;
        assert!(
            cleanup_result.is_ok(),
            "MockTaskExecutor cleanup should succeed"
        );
    }

    #[tokio::test]
    async fn test_shell_executor_new_methods() {
        let executor1 = ShellExecutor::new();
        let executor2 = ShellExecutor::with_cleanup_timeout(2000);

        // Both should be created successfully
        assert!(executor1.cleanup().await.is_ok());
        assert!(executor2.cleanup().await.is_ok());
    }

    #[tokio::test]
    async fn test_drop_safety() {
        // Test that executors can be dropped safely
        {
            let _executor1 = ShellExecutor::new();
            let _executor2 = HttpExecutor::new();
            let _executor3 = MockTaskExecutor::new("test".to_string(), true, 100);
        } // executors are dropped here

        // If we reach this point, the Drop implementations didn't panic
        assert!(true);
    }
}
