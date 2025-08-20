#[cfg(test)]
mod tests {
    use scheduler_dispatcher::worker_failure_detector::*;

    #[test]
    fn test_worker_failure_detector_config_default() {
        let config = WorkerFailureDetectorConfig::default();
        assert_eq!(config.heartbeat_timeout_seconds, 90);
        assert_eq!(config.detection_interval_seconds, 30);
        assert!(config.auto_cleanup_offline_workers);
        assert_eq!(config.offline_cleanup_threshold_seconds, 300);
    }

    #[test]
    fn test_worker_failure_detector_config_custom() {
        let config = WorkerFailureDetectorConfig {
            heartbeat_timeout_seconds: 60,
            detection_interval_seconds: 15,
            auto_cleanup_offline_workers: false,
            offline_cleanup_threshold_seconds: 600,
        };
        
        assert_eq!(config.heartbeat_timeout_seconds, 60);
        assert_eq!(config.detection_interval_seconds, 15);
        assert!(!config.auto_cleanup_offline_workers);
        assert_eq!(config.offline_cleanup_threshold_seconds, 600);
    }

    #[test]
    fn test_config_clone() {
        let config = WorkerFailureDetectorConfig::default();
        let cloned = config.clone();
        
        assert_eq!(cloned.heartbeat_timeout_seconds, config.heartbeat_timeout_seconds);
        assert_eq!(cloned.detection_interval_seconds, config.detection_interval_seconds);
        assert_eq!(cloned.auto_cleanup_offline_workers, config.auto_cleanup_offline_workers);
        assert_eq!(cloned.offline_cleanup_threshold_seconds, config.offline_cleanup_threshold_seconds);
    }

    #[test]
    fn test_config_debug() {
        let config = WorkerFailureDetectorConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("WorkerFailureDetectorConfig"));
    }

    #[test]
    fn test_worker_failure_detector_service_trait_exists() {
        // This test just ensures the trait is accessible
        use scheduler_dispatcher::worker_failure_detector::WorkerFailureDetectorService;
        
        // Test compiles if trait is accessible
        fn _uses_trait<T: WorkerFailureDetectorService>() {}
        
        // If this compiles, the trait exists and is accessible
        assert!(true);
    }
}