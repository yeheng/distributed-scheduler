#[cfg(test)]
mod figment_tests {
    use crate::AppConfig;

    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_figment_config_loading() {
        std::env::remove_var("SCHEDULER_ENV");
        
        let config = AppConfig::default_with_env().unwrap();
        assert_eq!(config.database.url, "postgresql://localhost/scheduler");
        assert_eq!(config.worker.worker_id, "worker-001");
    }

    #[test]
    fn test_environment_variables() {
        std::env::set_var("SCHEDULER_DATABASE_MAX_CONNECTIONS", "50");
        std::env::set_var("SCHEDULER_WORKER_WORKER_ID", "env-worker");
        
        let config = AppConfig::default_with_env().unwrap();
        
        assert_eq!(config.database.max_connections, 50);
        assert_eq!(config.worker.worker_id, "env-worker");
        
        std::env::remove_var("SCHEDULER_DATABASE_MAX_CONNECTIONS");
        std::env::remove_var("SCHEDULER_WORKER_WORKER_ID");
    }

    #[test]
    fn test_json_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.json");
        
        let json_content = r#"{
    "database": {
        "url": "postgresql://localhost/json_test",
        "max_connections": 15
    }
}"#;
        
        fs::write(&config_path, json_content).unwrap();
        
        let config = AppConfig::from_file(&config_path).unwrap();
        assert_eq!(config.database.url, "postgresql://localhost/json_test");
        assert_eq!(config.database.max_connections, 15);
    }

    #[test]
    fn test_environment_detection() {
        std::env::remove_var("SCHEDULER_ENV");
        let config = AppConfig::default();
        assert_eq!(config.environment(), "development");
        assert!(config.is_development());
        
        std::env::set_var("SCHEDULER_ENV", "production");
        let config = AppConfig::default();
        assert_eq!(config.environment(), "production");
        assert!(config.is_production());
        
        std::env::remove_var("SCHEDULER_ENV");
    }
}