pub mod interfaces;
pub mod services;

pub use interfaces::*;
pub use services::*;

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use scheduler_domain::entities::{Task, TaskRun, TaskRunStatus, WorkerInfo, WorkerStatus};
    use serde_json::json;

    #[test]
    fn test_helper_functions() {
        fn create_test_task() -> Task {
            Task {
                id: 1,
                name: "test_task".to_string(),
                task_type: "test_type".to_string(),
                schedule: "0 0 * * *".to_string(),
                parameters: json!({"command": "echo test"}),
                timeout_seconds: 300,
                max_retries: 3,
                status: scheduler_domain::entities::TaskStatus::Active,
                dependencies: Vec::new(),
                shard_config: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }

        fn create_test_task_run() -> TaskRun {
            TaskRun {
                id: 1,
                task_id: 1,
                status: TaskRunStatus::Pending,
                worker_id: Some("worker-1".to_string()),
                retry_count: 0,
                shard_index: None,
                shard_total: None,
                scheduled_at: Utc::now(),
                started_at: None,
                completed_at: None,
                result: None,
                error_message: None,
                created_at: Utc::now(),
            }
        }

        fn create_test_worker() -> WorkerInfo {
            WorkerInfo {
                id: "worker-1".to_string(),
                hostname: "test-host".to_string(),
                ip_address: "127.0.0.1".to_string(),
                supported_task_types: vec!["test_type".to_string()],
                max_concurrent_tasks: 5,
                current_task_count: 0,
                status: WorkerStatus::Alive,
                last_heartbeat: Utc::now(),
                registered_at: Utc::now(),
            }
        }

        let task = create_test_task();
        assert_eq!(task.name, "test_task");
        assert_eq!(task.task_type, "test_type");
        assert!(task.is_active());

        let task_run = create_test_task_run();
        assert_eq!(task_run.task_id, 1);
        assert_eq!(task_run.status, TaskRunStatus::Pending);

        let worker = create_test_worker();
        assert_eq!(worker.id, "worker-1");
        assert_eq!(worker.hostname, "test-host");
        assert!(worker.is_alive());
    }

    #[test]
    fn test_task_creation() {
        let task = Task::new(
            "test_task".to_string(),
            "test_type".to_string(),
            "0 0 * * *".to_string(),
            json!({"command": "echo test"}),
        );

        assert_eq!(task.name, "test_task");
        assert_eq!(task.task_type, "test_type");
        assert_eq!(task.schedule, "0 0 * * *");
        assert_eq!(task.timeout_seconds, 300);
        assert_eq!(task.max_retries, 0);
        assert!(task.is_active());
        assert!(!task.has_dependencies());
    }

    #[test]
    fn test_task_run_creation() {
        let scheduled_at = Utc::now();
        let task_run = TaskRun::new(1, scheduled_at);

        assert_eq!(task_run.task_id, 1);
        assert_eq!(task_run.status, TaskRunStatus::Pending);
        assert_eq!(task_run.scheduled_at, scheduled_at);
        assert!(task_run.worker_id.is_none());
        assert_eq!(task_run.retry_count, 0);
        assert!(!task_run.is_running());
        assert!(!task_run.is_finished());
        assert!(!task_run.is_successful());
    }

    #[test]
    fn test_task_run_status_updates() {
        let mut task_run = TaskRun::new(1, Utc::now());
        
        // Test status update to Running
        task_run.update_status(TaskRunStatus::Running);
        assert_eq!(task_run.status, TaskRunStatus::Running);
        assert!(task_run.started_at.is_some());
        assert!(task_run.is_running());

        // Test status update to Completed
        task_run.update_status(TaskRunStatus::Completed);
        assert_eq!(task_run.status, TaskRunStatus::Completed);
        assert!(task_run.completed_at.is_some());
        assert!(!task_run.is_running());
        assert!(task_run.is_finished());
        assert!(task_run.is_successful());
    }

    #[test]
    fn test_worker_info_creation() {
        let registration = scheduler_domain::entities::WorkerRegistration {
            worker_id: "worker-1".to_string(),
            hostname: "test-host".to_string(),
            ip_address: "127.0.0.1".to_string(),
            supported_task_types: vec!["test_type".to_string()],
            max_concurrent_tasks: 5,
        };

        let worker = WorkerInfo::new(registration);
        assert_eq!(worker.id, "worker-1");
        assert_eq!(worker.hostname, "test-host");
        assert_eq!(worker.ip_address, "127.0.0.1");
        assert_eq!(worker.max_concurrent_tasks, 5);
        assert_eq!(worker.current_task_count, 0);
        assert!(worker.is_alive());
        assert!(worker.can_accept_task("test_type"));
        assert!(!worker.can_accept_task("unsupported_type"));
        assert_eq!(worker.load_percentage(), 0.0);
    }

    #[test]
    fn test_worker_load_percentage() {
        let registration = scheduler_domain::entities::WorkerRegistration {
            worker_id: "worker-1".to_string(),
            hostname: "test-host".to_string(),
            ip_address: "127.0.0.1".to_string(),
            supported_task_types: vec!["test_type".to_string()],
            max_concurrent_tasks: 5,
        };

        let mut worker = WorkerInfo::new(registration);
        worker.current_task_count = 2;
        
        assert_eq!(worker.load_percentage(), 40.0);
        assert!(worker.can_accept_task("test_type"));
        
        worker.current_task_count = 5;
        assert_eq!(worker.load_percentage(), 100.0);
        assert!(!worker.can_accept_task("test_type"));
    }

    #[test]
    fn test_cron_scheduler_creation() {
        use crate::services::cron_utils::CronScheduler;
        
        let scheduler = CronScheduler::new("0 0 * * * *").unwrap();
        assert_eq!(scheduler.get_schedule_description(), "Cron schedule: 0 0 * * * *");
    }

    #[test]
    fn test_cron_scheduler_invalid_expression() {
        use crate::services::cron_utils::CronScheduler;
        use scheduler_foundation::SchedulerError;
        
        let result = CronScheduler::new("invalid cron expression");
        assert!(result.is_err());
        
        match result.unwrap_err() {
            SchedulerError::InvalidCron { expr, .. } => {
                assert_eq!(expr, "invalid cron expression");
            }
            _ => panic!("Expected InvalidCron error"),
        }
    }

    #[test]
    fn test_dependency_check_result_creation() {
        use crate::services::dependency_checker::DependencyCheckResult;
        
        let result = DependencyCheckResult {
            can_execute: true,
            blocking_dependencies: vec![1, 2],
            reason: Some("Waiting for dependencies".to_string()),
        };
        
        assert!(result.can_execute);
        assert_eq!(result.blocking_dependencies, vec![1, 2]);
        assert_eq!(result.reason, Some("Waiting for dependencies".to_string()));
    }

    #[test]
    fn test_scheduler_stats_creation() {
        use crate::interfaces::service_interfaces::SchedulerStats;
        
        let stats = SchedulerStats {
            total_tasks: 100,
            active_tasks: 50,
            running_task_runs: 10,
            pending_task_runs: 5,
            uptime_seconds: 3600,
            last_schedule_time: Some(Utc::now()),
        };

        assert_eq!(stats.total_tasks, 100);
        assert_eq!(stats.active_tasks, 50);
        assert_eq!(stats.running_task_runs, 10);
        assert_eq!(stats.pending_task_runs, 5);
        assert_eq!(stats.uptime_seconds, 3600);
        assert!(stats.last_schedule_time.is_some());
    }

    #[test]
    fn test_worker_load_stats_creation() {
        use crate::interfaces::service_interfaces::WorkerLoadStats;
        
        let stats = WorkerLoadStats {
            worker_id: "worker-1".to_string(),
            current_task_count: 3,
            max_concurrent_tasks: 5,
            system_load: Some(2.5),
            memory_usage_mb: Some(1024),
            last_heartbeat: Utc::now(),
        };

        assert_eq!(stats.worker_id, "worker-1");
        assert_eq!(stats.current_task_count, 3);
        assert_eq!(stats.max_concurrent_tasks, 5);
        assert_eq!(stats.system_load, Some(2.5));
        assert_eq!(stats.memory_usage_mb, Some(1024));
    }

    #[test]
    fn test_worker_heartbeat_creation() {
        use crate::interfaces::service_interfaces::WorkerHeartbeat;
        
        let heartbeat = WorkerHeartbeat {
            current_task_count: 2,
            system_load: Some(1.5),
            memory_usage_mb: Some(512),
            timestamp: Utc::now(),
        };

        assert_eq!(heartbeat.current_task_count, 2);
        assert_eq!(heartbeat.system_load, Some(1.5));
        assert_eq!(heartbeat.memory_usage_mb, Some(512));
    }

    #[test]
    fn test_data_structures_are_send_sync() {
        use crate::interfaces::service_interfaces::*;
        
        fn assert_send_sync<T: Send + Sync>() {}
        
        // Test that data structures are Send and Sync
        assert_send_sync::<SchedulerStats>();
        assert_send_sync::<WorkerLoadStats>();
        assert_send_sync::<WorkerHeartbeat>();
    }

    #[test]
    fn test_data_structures_are_clone() {
        use crate::interfaces::service_interfaces::*;
        
        let stats = SchedulerStats {
            total_tasks: 100,
            active_tasks: 50,
            running_task_runs: 10,
            pending_task_runs: 5,
            uptime_seconds: 3600,
            last_schedule_time: Some(Utc::now()),
        };

        let cloned_stats = stats.clone();
        assert_eq!(stats.total_tasks, cloned_stats.total_tasks);
        assert_eq!(stats.active_tasks, cloned_stats.active_tasks);
    }

    #[test]
    fn test_data_structures_debug() {
        use crate::interfaces::service_interfaces::*;
        
        let stats = SchedulerStats {
            total_tasks: 1,
            active_tasks: 1,
            running_task_runs: 1,
            pending_task_runs: 1,
            uptime_seconds: 1,
            last_schedule_time: Some(Utc::now()),
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("SchedulerStats"));
        assert!(debug_str.contains("total_tasks"));
    }
}
