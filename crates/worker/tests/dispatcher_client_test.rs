#[cfg(test)]
mod tests {
    use scheduler_worker::components::DispatcherClient;

    #[test]
    fn test_dispatcher_client_creation() {
        // Test that we can create a client without panicking
        let _client = DispatcherClient::new(
            Some("http://localhost:8080".to_string()),
            "worker-1".to_string(),
            "host1".to_string(),
            "127.0.0.1".parse().unwrap(),
        );

        // If we get here without panicking, the test passes
        assert!(true);
    }

    #[test]
    fn test_dispatcher_client_no_url() {
        // Test that we can create a client without URL
        let _client = DispatcherClient::new(
            None,
            "worker-2".to_string(),
            "host2".to_string(),
            "127.0.0.1".parse().unwrap(),
        );

        // If we get here without panicking, the test passes
        assert!(true);
    }

    #[tokio::test]
    async fn test_dispatcher_client_register_no_url() {
        let client = DispatcherClient::new(
            None,
            "worker-1".to_string(),
            "host1".to_string(),
            "127.0.0.1".parse().unwrap(),
        );

        // Should return Ok without making HTTP request when no URL is configured
        let result = client.register(vec!["shell".to_string()]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_dispatcher_client_register_with_url() {
        let client = DispatcherClient::new(
            Some("http://localhost:8080".to_string()),
            "worker-1".to_string(),
            "host1".to_string(),
            "127.0.0.1".parse().unwrap(),
        );

        // Should attempt to make HTTP request when URL is configured
        // This will likely fail due to no server running, but we can test the interface
        let _result = client.register(vec!["shell".to_string()]).await;
        
        // We don't care about the result, just that it doesn't panic
        // The result will likely be an error since no server is running
        assert!(true);
    }

    #[tokio::test]
    async fn test_dispatcher_client_send_heartbeat_no_url() {
        let client = DispatcherClient::new(
            None,
            "worker-1".to_string(),
            "host1".to_string(),
            "127.0.0.1".parse().unwrap(),
        );

        // Should return Ok without making HTTP request when no URL is configured
        let result = client.send_heartbeat(5).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_dispatcher_client_send_heartbeat_with_url() {
        let client = DispatcherClient::new(
            Some("http://localhost:8080".to_string()),
            "worker-1".to_string(),
            "host1".to_string(),
            "127.0.0.1".parse().unwrap(),
        );

        // Should attempt to make HTTP request when URL is configured
        let _result = client.send_heartbeat(5).await;
        
        // We don't care about the result, just that it doesn't panic
        assert!(true);
    }
}