use chrono::{DateTime, Utc};
use scheduler_domain::entities::TaskRunStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusUpdate {
    pub task_run_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: String,
    pub result: Option<String>,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_domain::entities::TaskRunStatus;

    #[test]
    fn test_task_status_update_creation() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 123,
            status: TaskRunStatus::Completed,
            worker_id: "worker-1".to_string(),
            result: Some("Task completed successfully".to_string()),
            error_message: None,
            timestamp,
        };

        assert_eq!(update.task_run_id, 123);
        assert_eq!(update.status, TaskRunStatus::Completed);
        assert_eq!(update.worker_id, "worker-1");
        assert_eq!(update.result, Some("Task completed successfully".to_string()));
        assert!(update.error_message.is_none());
    }

    #[test]
    fn test_task_status_update_with_error() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 456,
            status: TaskRunStatus::Failed,
            worker_id: "worker-2".to_string(),
            result: None,
            error_message: Some("Task failed due to timeout".to_string()),
            timestamp,
        };

        assert_eq!(update.task_run_id, 456);
        assert_eq!(update.status, TaskRunStatus::Failed);
        assert_eq!(update.worker_id, "worker-2");
        assert!(update.result.is_none());
        assert_eq!(update.error_message, Some("Task failed due to timeout".to_string()));
    }

    #[test]
    fn test_task_status_update_running() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 789,
            status: TaskRunStatus::Running,
            worker_id: "worker-3".to_string(),
            result: None,
            error_message: None,
            timestamp,
        };

        assert_eq!(update.task_run_id, 789);
        assert_eq!(update.status, TaskRunStatus::Running);
        assert_eq!(update.worker_id, "worker-3");
        assert!(update.result.is_none());
        assert!(update.error_message.is_none());
    }

    #[test]
    fn test_task_status_update_clone() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 100,
            status: TaskRunStatus::Pending,
            worker_id: "worker-test".to_string(),
            result: Some("Test result".to_string()),
            error_message: Some("Test error".to_string()),
            timestamp,
        };

        let cloned_update = update.clone();
        assert_eq!(update.task_run_id, cloned_update.task_run_id);
        assert_eq!(update.status, cloned_update.status);
        assert_eq!(update.worker_id, cloned_update.worker_id);
        assert_eq!(update.result, cloned_update.result);
        assert_eq!(update.error_message, cloned_update.error_message);
        assert_eq!(update.timestamp, cloned_update.timestamp);
    }

    #[test]
    fn test_task_status_update_debug() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 42,
            status: TaskRunStatus::Completed,
            worker_id: "debug-worker".to_string(),
            result: Some("Debug result".to_string()),
            error_message: None,
            timestamp,
        };

        let debug_str = format!("{:?}", update);
        assert!(debug_str.contains("TaskStatusUpdate"));
        assert!(debug_str.contains("task_run_id"));
        assert!(debug_str.contains("42"));
        assert!(debug_str.contains("debug-worker"));
    }

    #[test]
    fn test_task_status_update_serialization() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 999,
            status: TaskRunStatus::Failed,
            worker_id: "serial-worker".to_string(),
            result: None,
            error_message: Some("Serialization test".to_string()),
            timestamp,
        };

        // Test JSON serialization
        let json_str = serde_json::to_string(&update).unwrap();
        let deserialized: TaskStatusUpdate = serde_json::from_str(&json_str).unwrap();

        assert_eq!(update.task_run_id, deserialized.task_run_id);
        assert_eq!(update.status, deserialized.status);
        assert_eq!(update.worker_id, deserialized.worker_id);
        assert_eq!(update.result, deserialized.result);
        assert_eq!(update.error_message, deserialized.error_message);
    }

    #[test]
    fn test_task_status_update_different_statuses() {
        let timestamp = Utc::now();
        
        let statuses = vec![
            TaskRunStatus::Pending,
            TaskRunStatus::Running,
            TaskRunStatus::Completed,
            TaskRunStatus::Failed,
        ];

        for status in statuses {
            let update = TaskStatusUpdate {
                task_run_id: 1,
                status: status.clone(),
                worker_id: "test-worker".to_string(),
                result: None,
                error_message: None,
                timestamp,
            };

            assert_eq!(update.status, status);
        }
    }

    #[test]
    fn test_task_status_update_with_long_worker_id() {
        let timestamp = Utc::now();
        let long_worker_id = "very-long-worker-id-with-many-characters-123456789".to_string();
        let update = TaskStatusUpdate {
            task_run_id: 1,
            status: TaskRunStatus::Running,
            worker_id: long_worker_id.clone(),
            result: None,
            error_message: None,
            timestamp,
        };

        assert_eq!(update.worker_id, long_worker_id);
        assert!(update.worker_id.len() > 20);
    }

    #[test]
    fn test_task_status_update_with_empty_result() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 1,
            status: TaskRunStatus::Completed,
            worker_id: "worker-1".to_string(),
            result: Some("".to_string()),
            error_message: None,
            timestamp,
        };

        assert_eq!(update.result, Some("".to_string()));
    }

    #[test]
    fn test_task_status_update_with_empty_error_message() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 1,
            status: TaskRunStatus::Failed,
            worker_id: "worker-1".to_string(),
            result: None,
            error_message: Some("".to_string()),
            timestamp,
        };

        assert_eq!(update.error_message, Some("".to_string()));
    }

    #[test]
    fn test_task_status_update_negative_task_run_id() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: -1,
            status: TaskRunStatus::Pending,
            worker_id: "worker-1".to_string(),
            result: None,
            error_message: None,
            timestamp,
        };

        assert_eq!(update.task_run_id, -1);
    }

    #[test]
    fn test_task_status_update_zero_task_run_id() {
        let timestamp = Utc::now();
        let update = TaskStatusUpdate {
            task_run_id: 0,
            status: TaskRunStatus::Pending,
            worker_id: "worker-1".to_string(),
            result: None,
            error_message: None,
            timestamp,
        };

        assert_eq!(update.task_run_id, 0);
    }
}
