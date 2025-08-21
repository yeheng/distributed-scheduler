#[cfg(test)]
mod tests {

    use crate::WorkerConfig;

    #[tokio::test]
    async fn test_worker_config_builder() {
        let config = WorkerConfig::builder(
            "test_worker".to_string(),
            "test_tasks".to_string(),
            "test_status".to_string(),
        )
        .max_concurrent_tasks(10)
        .heartbeat_interval_seconds(60)
        .poll_interval_ms(500)
        .build();

        assert_eq!(config.worker_id, "test_worker");
        assert_eq!(config.task_queue, "test_tasks");
        assert_eq!(config.status_queue, "test_status");
        assert_eq!(config.max_concurrent_tasks, 10);
        assert_eq!(config.heartbeat_interval_seconds, 60);
        assert_eq!(config.poll_interval_ms, 500);
    }

}
