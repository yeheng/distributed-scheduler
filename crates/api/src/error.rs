use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use scheduler_errors::SchedulerError;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("调度器错误: {0}")]
    Scheduler(#[from] SchedulerError),

    #[error("验证错误: {0}")]
    Validation(#[from] validator::ValidationErrors),

    #[error("验证错误: {0}")]
    ValidationError(#[from] validator::ValidationError),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("内部服务器错误: {0}")]
    Internal(String),

    #[error("未找到资源")]
    NotFound,

    #[error("请求冲突: {0}")]
    Conflict(String),

    #[error("请求参数错误: {0}")]
    BadRequest(String),

    #[error("认证错误: {0}")]
    Authentication(#[from] crate::auth::AuthError),

    #[error("权限不足")]
    Forbidden,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message, error_type, suggestions) = match &self {
            ApiError::Scheduler(SchedulerError::TaskNotFound { id }) => {
                (
                    StatusCode::NOT_FOUND,
                    format!("任务 ID {} 不存在", id),
                    "TASK_NOT_FOUND".to_string(),
                    vec![
                        "请检查任务ID是否正确".to_string(),
                        "使用 GET /api/tasks 查看所有可用任务".to_string(),
                    ],
                )
            }
            ApiError::Scheduler(SchedulerError::WorkerNotFound { id }) => {
                (
                    StatusCode::NOT_FOUND,
                    format!("Worker {} 不存在", id),
                    "WORKER_NOT_FOUND".to_string(),
                    vec![
                        "请检查Worker ID是否正确".to_string(),
                        "使用 GET /api/workers 查看所有可用Worker".to_string(),
                    ],
                )
            }
            ApiError::Scheduler(SchedulerError::TaskRunNotFound { id }) => {
                (
                    StatusCode::NOT_FOUND,
                    format!("任务执行记录 ID {} 不存在", id),
                    "TASK_RUN_NOT_FOUND".to_string(),
                    vec![
                        "请检查任务执行记录ID是否正确".to_string(),
                        "使用 GET /api/tasks/{task_id}/runs 查看任务的执行记录".to_string(),
                    ],
                )
            }
            ApiError::Scheduler(SchedulerError::InvalidTaskParams(msg)) => {
                (
                    StatusCode::BAD_REQUEST,
                    format!("任务参数无效: {}", msg),
                    "INVALID_TASK_PARAMS".to_string(),
                    vec![
                        "请检查任务参数格式是否正确".to_string(),
                        "参考 GET /api/docs 查看参数规范".to_string(),
                    ],
                )
            }
            ApiError::Scheduler(SchedulerError::InvalidCron { expr, message }) => {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Cron表达式 '{}' 无效: {}", expr, message),
                    "INVALID_CRON_EXPRESSION".to_string(),
                    vec![
                        "请使用标准的Cron表达式格式".to_string(),
                        "示例: '0 */5 * * * *' (每5分钟执行一次)".to_string(),
                        "在线工具: https://crontab.guru/ 可以帮助验证表达式".to_string(),
                    ],
                )
            }
            ApiError::Scheduler(SchedulerError::CircularDependency) => {
                (
                    StatusCode::BAD_REQUEST,
                    "检测到任务依赖循环".to_string(),
                    "CIRCULAR_DEPENDENCY".to_string(),
                    vec![
                        "请检查任务依赖关系，确保没有循环依赖".to_string(),
                        "任务A不能依赖任务B，同时任务B又依赖任务A".to_string(),
                    ],
                )
            }
            ApiError::Validation(errors) => {
                let error_details: Vec<String> = errors
                    .field_errors()
                    .iter()
                    .map(|(field, errors)| {
                        let messages: Vec<String> = errors.iter().map(|e| e.message.as_ref().unwrap_or(&std::borrow::Cow::Borrowed("验证失败")).to_string()).collect();
                        format!("{}: {}", field, messages.join(", "))
                    })
                    .collect();
                
                (
                    StatusCode::BAD_REQUEST,
                    format!("请求参数验证失败: {}", error_details.join("; ")),
                    "VALIDATION_ERROR".to_string(),
                    vec![
                        "请检查请求参数是否符合要求".to_string(),
                        "参考 GET /api/docs 查看参数规范".to_string(),
                    ],
                )
            }
            ApiError::ValidationError(error) => {
                (
                    StatusCode::BAD_REQUEST,
                    format!("参数验证失败: {}", error.message.as_ref().unwrap_or(&std::borrow::Cow::Borrowed("验证失败"))),
                    "VALIDATION_ERROR".to_string(),
                    vec![
                        "请检查请求参数格式".to_string(),
                        "参考 GET /api/docs 查看参数规范".to_string(),
                    ],
                )
            }
            ApiError::BadRequest(msg) => {
                (
                    StatusCode::BAD_REQUEST,
                    format!("请求参数错误: {}", msg),
                    "BAD_REQUEST".to_string(),
                    vec![
                        "请检查请求格式和参数".to_string(),
                        "确保Content-Type正确设置".to_string(),
                    ],
                )
            }
            ApiError::Authentication(auth_error) => {
                let (msg, suggestions) = match auth_error {
                    crate::auth::AuthError::MissingToken => (
                        "缺少认证令牌".to_string(),
                        vec![
                            "请在请求头中添加 Authorization: Bearer <token>".to_string(),
                            "使用 POST /api/auth/login 获取令牌".to_string(),
                        ]
                    ),
                    crate::auth::AuthError::InvalidToken => (
                        "认证令牌无效".to_string(),
                        vec![
                            "请检查令牌格式是否正确".to_string(),
                            "令牌可能已过期，请重新登录".to_string(),
                        ]
                    ),
                    _ => (
                        "认证失败".to_string(),
                        vec!["请检查认证信息".to_string()]
                    ),
                };
                (StatusCode::UNAUTHORIZED, msg, "AUTHENTICATION_ERROR".to_string(), suggestions)
            }
            ApiError::Forbidden => {
                (
                    StatusCode::FORBIDDEN,
                    "权限不足".to_string(),
                    "FORBIDDEN".to_string(),
                    vec![
                        "您没有执行此操作的权限".to_string(),
                        "请联系管理员获取相应权限".to_string(),
                    ],
                )
            }
            ApiError::NotFound => {
                (
                    StatusCode::NOT_FOUND,
                    "请求的资源不存在".to_string(),
                    "NOT_FOUND".to_string(),
                    vec![
                        "请检查请求URL是否正确".to_string(),
                        "访问 GET / 查看可用的API端点".to_string(),
                    ],
                )
            }
            ApiError::Conflict(msg) => {
                (
                    StatusCode::CONFLICT,
                    format!("资源冲突: {}", msg),
                    "CONFLICT".to_string(),
                    vec![
                        "请求的操作与当前资源状态冲突".to_string(),
                        "请刷新资源状态后重试".to_string(),
                    ],
                )
            }
            ApiError::Serialization(err) => {
                (
                    StatusCode::BAD_REQUEST,
                    "请求数据格式错误".to_string(),
                    "SERIALIZATION_ERROR".to_string(),
                    vec![
                        "请检查JSON格式是否正确".to_string(),
                        "确保所有必需字段都已提供".to_string(),
                        format!("详细错误: {}", err),
                    ],
                )
            }
            ApiError::Scheduler(_) => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "系统内部错误".to_string(),
                    "INTERNAL_ERROR".to_string(),
                    vec![
                        "系统遇到内部错误，请稍后重试".to_string(),
                        "如果问题持续存在，请联系系统管理员".to_string(),
                        "查看 GET /health 检查系统状态".to_string(),
                    ],
                )
            }
            ApiError::Internal(msg) => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "系统内部错误".to_string(),
                    "INTERNAL_ERROR".to_string(),
                    vec![
                        "系统遇到内部错误，请稍后重试".to_string(),
                        "如果问题持续存在，请联系系统管理员".to_string(),
                        format!("错误详情: {}", msg),
                    ],
                )
            }
        };

        let body = Json(json!({
            "error": {
                "message": error_message,
                "type": error_type,
                "code": status.as_u16(),
                "suggestions": suggestions,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "documentation": "/api/docs"
            }
        }));

        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_errors::SchedulerError;

    #[test]
    fn test_api_error_scheduler_error_conversion() {
        let scheduler_error = SchedulerError::TaskNotFound { id: 123 };
        let api_error: ApiError = scheduler_error.into();

        match api_error {
            ApiError::Scheduler(SchedulerError::TaskNotFound { id }) => {
                assert_eq!(id, 123);
            }
            _ => panic!("Expected SchedulerError::TaskNotFound"),
        }
    }

    #[test]
    fn test_api_error_into_response_not_found() {
        let error = ApiError::Scheduler(SchedulerError::TaskNotFound { id: 123 });
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_api_error_into_response_bad_request() {
        let error = ApiError::BadRequest("Invalid parameter".to_string());
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_api_error_into_response_unauthorized() {
        let error = ApiError::Authentication(crate::auth::AuthError::MissingToken);
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_api_error_into_response_forbidden() {
        let error = ApiError::Forbidden;
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_api_error_into_response_internal() {
        let error = ApiError::Internal("Internal error".to_string());
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_api_error_display() {
        let error = ApiError::NotFound;
        let display_str = format!("{}", error);
        assert_eq!(display_str, "未找到资源");
    }

    #[test]
    fn test_api_error_from_validation_errors() {
        use validator::ValidationErrors;

        // This is a simplified test since creating actual ValidationErrors is complex
        let validation_error = validator::ValidationError::new("test field");
        let mut errors = ValidationErrors::new();
        errors.add("test_field", validation_error);

        let api_error: ApiError = errors.into();
        match api_error {
            ApiError::Validation(_) => {}
            _ => panic!("Expected Validation error"),
        }
    }

    #[test]
    fn test_api_error_from_json_error() {
        let json_error = serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "JSON error",
        ));
        let api_error: ApiError = json_error.into();

        match api_error {
            ApiError::Serialization(_) => {}
            _ => panic!("Expected Serialization error"),
        }
    }

    #[test]
    fn test_api_error_from_auth_error() {
        let auth_error = crate::auth::AuthError::MissingToken;
        let api_error: ApiError = auth_error.into();

        match api_error {
            ApiError::Authentication(crate::auth::AuthError::MissingToken) => {}
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_api_error_conflict() {
        let error = ApiError::Conflict("Resource conflict".to_string());
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_api_error_validation_error() {
        use validator::ValidationError;

        let validation_error = ValidationError::new("field");
        let api_error: ApiError = validation_error.into();

        match api_error {
            ApiError::ValidationError(_) => {}
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_scheduler_error_variants() {
        // Test various scheduler error types
        let test_cases = vec![
            SchedulerError::TaskNotFound { id: 1 },
            SchedulerError::WorkerNotFound {
                id: "test".to_string(),
            },
            SchedulerError::TaskRunNotFound { id: 1 },
            SchedulerError::InvalidTaskParams("Invalid params".to_string()),
            SchedulerError::InvalidCron {
                expr: "invalid".to_string(),
                message: "Invalid cron expression".to_string(),
            },
            SchedulerError::CircularDependency,
        ];

        for scheduler_error in test_cases {
            let api_error: ApiError = scheduler_error.into();
            let response = api_error.into_response();

            // All scheduler errors should result in appropriate status codes
            assert!(matches!(
                response.status(),
                StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST | StatusCode::INTERNAL_SERVER_ERROR
            ));
        }
    }

    #[test]
    fn test_api_error_debug_format() {
        let error = ApiError::Internal("Test error".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Internal"));
        assert!(debug_str.contains("Test error"));
    }
}
