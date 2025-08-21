#[cfg(test)]
mod error_tests {
    use crate::*;

    #[test]
    fn test_scheduler_error_display() {
        // Test DatabaseOperation error
        let db_op_error = SchedulerError::DatabaseOperation("Connection failed".to_string());
        assert_eq!(db_op_error.to_string(), "数据库操作错误: Connection failed");

        // Test TaskNotFound error
        let task_error = SchedulerError::TaskNotFound { id: 123 };
        assert_eq!(task_error.to_string(), "任务未找到: 123");

        // Test TaskRunNotFound error
        let task_run_error = SchedulerError::TaskRunNotFound { id: 456 };
        assert_eq!(task_run_error.to_string(), "任务运行实例未找到: 456");

        // Test WorkerNotFound error
        let worker_error = SchedulerError::WorkerNotFound {
            id: "worker-1".to_string(),
        };
        assert_eq!(worker_error.to_string(), "Worker未找到: worker-1");

        // Test ExecutionTimeout error
        let timeout_error = SchedulerError::ExecutionTimeout;
        assert_eq!(timeout_error.to_string(), "任务执行超时");

        // Test CircularDependency error
        let circular_error = SchedulerError::CircularDependency;
        assert_eq!(circular_error.to_string(), "检测到循环依赖");

        // Test LeadershipLost error
        let leadership_error = SchedulerError::LeadershipLost;
        assert_eq!(leadership_error.to_string(), "失去领导权");

        // Test MessageQueue error
        let mq_error = SchedulerError::MessageQueue("Connection failed".to_string());
        assert_eq!(mq_error.to_string(), "消息队列错误: Connection failed");

        // Test Serialization error
        let serial_error = SchedulerError::Serialization("JSON parse error".to_string());
        assert_eq!(serial_error.to_string(), "序列化错误: JSON parse error");

        // Test Configuration error
        let config_error = SchedulerError::Configuration("Missing required field".to_string());
        assert_eq!(config_error.to_string(), "配置错误: Missing required field");

        // Test TaskExecution error
        let exec_error = SchedulerError::TaskExecution("Command not found".to_string());
        assert_eq!(exec_error.to_string(), "任务执行错误: Command not found");

        // Test Network error
        let network_error = SchedulerError::Network("Connection refused".to_string());
        assert_eq!(network_error.to_string(), "网络错误: Connection refused");

        // Test Internal error
        let internal_error = SchedulerError::Internal("Unexpected error".to_string());
        assert_eq!(internal_error.to_string(), "内部错误: Unexpected error");

        // Test InvalidTaskParams error
        let params_error = SchedulerError::InvalidTaskParams("Invalid parameter".to_string());
        assert_eq!(
            params_error.to_string(),
            "无效的任务参数: Invalid parameter"
        );

        // Test ValidationError error
        let validation_error = SchedulerError::ValidationError("Invalid input".to_string());
        assert_eq!(validation_error.to_string(), "数据验证失败: Invalid input");

        // Test Timeout error
        let timeout_error = SchedulerError::Timeout("Operation timed out".to_string());
        assert_eq!(timeout_error.to_string(), "操作超时: Operation timed out");

        // Test ResourceExhausted error
        let resource_error = SchedulerError::ResourceExhausted("Out of memory".to_string());
        assert_eq!(resource_error.to_string(), "资源不足: Out of memory");

        // Test Permission error
        let perm_error = SchedulerError::Permission("Access denied".to_string());
        assert_eq!(perm_error.to_string(), "权限不足: Access denied");

        // Test CacheError error
        let cache_error = SchedulerError::CacheError("Cache miss".to_string());
        assert_eq!(cache_error.to_string(), "缓存错误: Cache miss");
    }

    #[test]
    fn test_scheduler_error_creation_methods() {
        // Test database_error
        let error = SchedulerError::database_error("Connection failed");
        assert!(matches!(error, SchedulerError::DatabaseOperation(_)));

        // Test task_not_found
        let error = SchedulerError::task_not_found(123);
        assert!(matches!(error, SchedulerError::TaskNotFound { id: 123 }));

        // Test worker_not_found with string
        let error = SchedulerError::worker_not_found("worker-1");
        assert!(matches!(error, SchedulerError::WorkerNotFound { id: _ }));

        // Test task_run_not_found
        let error = SchedulerError::task_run_not_found(456);
        assert!(matches!(error, SchedulerError::TaskRunNotFound { id: 456 }));

        // Test invalid_params
        let error = SchedulerError::invalid_params("Invalid parameter");
        assert!(matches!(error, SchedulerError::InvalidTaskParams(_)));

        // Test config_error
        let error = SchedulerError::config_error("Missing config");
        assert!(matches!(error, SchedulerError::Configuration(_)));

        // Test validation_error
        let error = SchedulerError::validation_error("Invalid input");
        assert!(matches!(error, SchedulerError::ValidationError(_)));

        // Test timeout_error
        let error = SchedulerError::timeout_error("Operation timed out");
        assert!(matches!(error, SchedulerError::Timeout(_)));
    }

    #[test]
    fn test_is_fatal() {
        // Test fatal errors
        assert!(SchedulerError::Internal("Critical error".to_string()).is_fatal());
        assert!(SchedulerError::Configuration("Invalid config".to_string()).is_fatal());
        assert!(SchedulerError::ResourceExhausted("Out of memory".to_string()).is_fatal());

        // Test non-fatal errors
        assert!(!SchedulerError::TaskNotFound { id: 123 }.is_fatal());
        assert!(!SchedulerError::TaskExecution("Failed".to_string()).is_fatal());
        assert!(!SchedulerError::Network("Connection failed".to_string()).is_fatal());
        assert!(!SchedulerError::Timeout("Timed out".to_string()).is_fatal());
        assert!(!SchedulerError::MessageQueue("MQ error".to_string()).is_fatal());
    }

    #[test]
    fn test_is_retryable() {
        // Test retryable errors
        assert!(SchedulerError::DatabaseOperation("Temporary failure".to_string()).is_retryable());
        assert!(SchedulerError::MessageQueue("Queue full".to_string()).is_retryable());
        assert!(SchedulerError::Network("Connection timeout".to_string()).is_retryable());
        assert!(SchedulerError::Timeout("Operation timeout".to_string()).is_retryable());

        // Test non-retryable errors
        assert!(!SchedulerError::TaskNotFound { id: 123 }.is_retryable());
        assert!(!SchedulerError::InvalidTaskParams("Invalid params".to_string()).is_retryable());
        assert!(!SchedulerError::Configuration("Invalid config".to_string()).is_retryable());
        assert!(!SchedulerError::ValidationError("Invalid input".to_string()).is_retryable());
        assert!(!SchedulerError::Internal("Critical error".to_string()).is_retryable());
        assert!(!SchedulerError::ResourceExhausted("Out of memory".to_string()).is_retryable());
        assert!(!SchedulerError::Permission("Access denied".to_string()).is_retryable());
        assert!(!SchedulerError::CircularDependency.is_retryable());
    }

    #[test]
    fn test_user_message() {
        // Test specific user messages
        assert_eq!(
            SchedulerError::TaskNotFound { id: 123 }.user_message(),
            "请求的任务不存在"
        );
        assert_eq!(
            SchedulerError::WorkerNotFound {
                id: "worker-1".to_string()
            }
            .user_message(),
            "请求的Worker节点不存在"
        );
        assert_eq!(
            SchedulerError::TaskRunNotFound { id: 456 }.user_message(),
            "请求的任务执行记录不存在"
        );
        assert_eq!(
            SchedulerError::InvalidTaskParams("Invalid params".to_string()).user_message(),
            "任务参数配置有误"
        );
        assert_eq!(
            SchedulerError::ValidationError("Invalid input".to_string()).user_message(),
            "输入数据验证失败"
        );
        assert_eq!(
            SchedulerError::Permission("Access denied".to_string()).user_message(),
            "您没有执行此操作的权限"
        );
        assert_eq!(
            SchedulerError::ResourceExhausted("Out of memory".to_string()).user_message(),
            "系统资源不足，请稍后重试"
        );
        assert_eq!(
            SchedulerError::Timeout("Operation timeout".to_string()).user_message(),
            "操作超时，请稍后重试"
        );

        // Test generic fallback message
        assert_eq!(
            SchedulerError::Internal("Critical error".to_string()).user_message(),
            "系统繁忙，请稍后重试"
        );
        assert_eq!(
            SchedulerError::CircularDependency.user_message(),
            "系统繁忙，请稍后重试"
        );
    }

    #[test]
    fn test_scheduler_result_type() {
        // Test SchedulerResult with Ok value
        let result: SchedulerResult<i32> = Ok(42);
        assert_eq!(result.expect("Should be Ok"), 42);

        // Test SchedulerResult with Err value
        let result: SchedulerResult<i32> = Err(SchedulerError::TaskNotFound { id: 123 });
        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Should be Err"),
            SchedulerError::TaskNotFound { .. }
        ));
    }

    #[test]
    fn test_error_from_sqlx() {
        // Test conversion from sqlx::Error
        let sqlx_error = sqlx::Error::RowNotFound;
        let scheduler_error: SchedulerError = sqlx_error.into();
        assert!(matches!(scheduler_error, SchedulerError::Database(_)));
    }

    #[test]
    fn test_error_from_serde_json() {
        // Test conversion from serde_json::Error
        let json_error = serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "JSON parse error",
        ));
        let scheduler_error: SchedulerError = json_error.into();
        assert!(matches!(scheduler_error, SchedulerError::Serialization(_)));
    }

    #[test]
    fn test_error_from_anyhow() {
        // Test conversion from anyhow::Error
        let anyhow_error = anyhow::Error::msg("Some error");
        let scheduler_error: SchedulerError = anyhow_error.into();
        assert!(matches!(scheduler_error, SchedulerError::Internal(_)));
    }

    #[test]
    fn test_error_debug() {
        // Test Debug implementation
        let error = SchedulerError::TaskNotFound { id: 123 };
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("TaskNotFound"));
        assert!(debug_str.contains("123"));
    }

    #[test]
    fn test_scheduler_error_send_sync() {
        // Test that SchedulerError is Send and Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SchedulerError>();
    }

    #[test]
    fn test_comprehensive_error_scenarios() {
        // Test realistic error scenarios
        let scenarios = vec![
            (
                SchedulerError::DatabaseOperation("Connection pool exhausted".to_string()),
                true,  // is_retryable
                false, // is_fatal
                "系统繁忙，请稍后重试",
            ),
            (
                SchedulerError::TaskNotFound { id: 999 },
                false, // is_retryable
                false, // is_fatal
                "请求的任务不存在",
            ),
            (
                SchedulerError::Configuration("Missing database URL".to_string()),
                false, // is_retryable
                true,  // is_fatal
                "系统繁忙，请稍后重试",
            ),
            (
                SchedulerError::ExecutionTimeout,
                false, // is_retryable
                false, // is_fatal
                "系统繁忙，请稍后重试",
            ),
            (
                SchedulerError::CircularDependency,
                false, // is_retryable
                false, // is_fatal
                "系统繁忙，请稍后重试",
            ),
        ];

        for (error, expected_retryable, expected_fatal, expected_user_msg) in scenarios {
            assert_eq!(error.is_retryable(), expected_retryable);
            assert_eq!(error.is_fatal(), expected_fatal);
            assert_eq!(error.user_message(), expected_user_msg);
        }
    }

    #[test]
    fn test_error_chain_compatibility() {
        // Test that errors can be used with anyhow::Error
        let result: Result<(), SchedulerError> = Err(SchedulerError::TaskNotFound { id: 123 });
        let anyhow_result: Result<(), anyhow::Error> = result.map_err(|e| e.into());
        assert!(anyhow_result.is_err());
        assert!(anyhow_result
            .expect_err("Should be Err")
            .to_string()
            .contains("任务未找到"));
    }

    #[test]
    fn test_custom_error_messages() {
        // Test error messages with special characters and unicode
        let error = SchedulerError::TaskExecution("命令执行失败: 找不到文件".to_string());
        assert!(error.to_string().contains("命令执行失败"));
        assert!(error.to_string().contains("找不到文件"));
    }

    #[test]
    fn test_error_construction_with_various_types() {
        // Test construction with different string types
        let error1 = SchedulerError::DatabaseOperation("String".to_string());
        let error2 = SchedulerError::DatabaseOperation("&str".to_string());
        let error3 = SchedulerError::DatabaseOperation(format!("{} {}", "formatted", "string"));

        assert_eq!(error1.to_string(), "数据库操作错误: String");
        assert_eq!(error2.to_string(), "数据库操作错误: &str");
        assert_eq!(error3.to_string(), "数据库操作错误: formatted string");
    }
}
