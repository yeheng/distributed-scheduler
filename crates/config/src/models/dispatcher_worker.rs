use crate::validation::{ConfigValidator, ValidationUtils};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatcherConfig {
    pub enabled: bool,
    pub schedule_interval_seconds: u64,
    pub max_concurrent_dispatches: usize,
    pub worker_timeout_seconds: u64,
    pub dispatch_strategy: String,
}

impl ConfigValidator for DispatcherConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        ValidationUtils::validate_timeout_seconds(self.schedule_interval_seconds)?;
        ValidationUtils::validate_count(
            self.max_concurrent_dispatches,
            "dispatcher.max_concurrent_dispatches",
            10000,
        )?;
        ValidationUtils::validate_timeout_seconds(self.worker_timeout_seconds)?;

        ValidationUtils::validate_not_empty(
            &self.dispatch_strategy,
            "dispatcher.dispatch_strategy",
        )?;

        // Validate dispatch strategy
        let valid_strategies = ["round_robin", "random", "least_loaded", "priority"];
        if !valid_strategies.contains(&self.dispatch_strategy.as_str()) {
            return Err(crate::ConfigError::Validation(format!(
                "Invalid dispatch strategy: {}. Valid options: {:?}",
                self.dispatch_strategy, valid_strategies
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub enabled: bool,
    pub worker_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub max_concurrent_tasks: usize,
    pub supported_task_types: Vec<String>,
    pub heartbeat_interval_seconds: u64,
    pub task_poll_interval_seconds: u64,
}

impl ConfigValidator for WorkerConfig {
    fn validate(&self) -> crate::ConfigResult<()> {
        ValidationUtils::validate_not_empty(&self.worker_id, "worker.worker_id")?;
        ValidationUtils::validate_not_empty(&self.hostname, "worker.hostname")?;
        ValidationUtils::validate_not_empty(&self.ip_address, "worker.ip_address")?;

        ValidationUtils::validate_count(
            self.max_concurrent_tasks,
            "worker.max_concurrent_tasks",
            1000,
        )?;
        ValidationUtils::validate_timeout_seconds(self.heartbeat_interval_seconds)?;
        ValidationUtils::validate_timeout_seconds(self.task_poll_interval_seconds)?;

        // Validate supported task types
        if self.supported_task_types.is_empty() {
            return Err(crate::ConfigError::Validation(
                "worker.supported_task_types cannot be empty".to_string(),
            ));
        }

        for task_type in &self.supported_task_types {
            ValidationUtils::validate_not_empty(task_type, "worker.supported_task_types")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatcher_config_validation() {
        let config = DispatcherConfig {
            enabled: true,
            schedule_interval_seconds: 10,
            max_concurrent_dispatches: 100,
            worker_timeout_seconds: 90,
            dispatch_strategy: "round_robin".to_string(),
        };

        assert!(config.validate().is_ok());

        // Test invalid schedule_interval_seconds
        let mut invalid_config = config.clone();
        invalid_config.schedule_interval_seconds = 0;
        assert!(invalid_config.validate().is_err());

        // Test invalid dispatch_strategy
        let mut invalid_config = config.clone();
        invalid_config.dispatch_strategy = "invalid_strategy".to_string();
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_worker_config_validation() {
        let config = WorkerConfig {
            enabled: true,
            worker_id: "worker-001".to_string(),
            hostname: "localhost".to_string(),
            ip_address: "127.0.0.1".to_string(),
            max_concurrent_tasks: 5,
            supported_task_types: vec!["shell".to_string(), "http".to_string()],
            heartbeat_interval_seconds: 30,
            task_poll_interval_seconds: 5,
        };

        assert!(config.validate().is_ok());

        // Test invalid worker_id
        let mut invalid_config = config.clone();
        invalid_config.worker_id = "".to_string();
        assert!(invalid_config.validate().is_err());

        // Test empty supported_task_types
        let mut invalid_config = config.clone();
        invalid_config.supported_task_types = vec![];
        assert!(invalid_config.validate().is_err());

        // Test invalid heartbeat_interval_seconds
        let mut invalid_config = config.clone();
        invalid_config.heartbeat_interval_seconds = 0;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_dispatcher_config_serialization() {
        let config = DispatcherConfig {
            enabled: true,
            schedule_interval_seconds: 10,
            max_concurrent_dispatches: 100,
            worker_timeout_seconds: 90,
            dispatch_strategy: "round_robin".to_string(),
        };

        let serialized = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: DispatcherConfig =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(
            config.schedule_interval_seconds,
            deserialized.schedule_interval_seconds
        );
        assert_eq!(config.dispatch_strategy, deserialized.dispatch_strategy);
    }
}
