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
        let (status, error_message) = match &self {
            ApiError::Scheduler(SchedulerError::TaskNotFound { .. }) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            ApiError::Scheduler(SchedulerError::WorkerNotFound { .. }) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            ApiError::Scheduler(SchedulerError::TaskRunNotFound { .. }) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            ApiError::Scheduler(SchedulerError::InvalidTaskParams(_)) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            ApiError::Scheduler(SchedulerError::InvalidCron { .. }) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            ApiError::Scheduler(SchedulerError::CircularDependency) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            ApiError::Validation(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::ValidationError(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::Authentication(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "权限不足".to_string()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "资源未找到".to_string()),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            ApiError::Serialization(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::Scheduler(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "内部服务器错误".to_string(),
            ),
            ApiError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "内部服务器错误".to_string(),
            ),
        };

        let body = Json(json!({
            "error": {
                "message": error_message,
                "code": status.as_u16()
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
            },
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
            ApiError::Validation(_) => {},
            _ => panic!("Expected Validation error"),
        }
    }

    #[test]
    fn test_api_error_from_json_error() {
        let json_error = serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, "JSON error"));
        let api_error: ApiError = json_error.into();
        
        match api_error {
            ApiError::Serialization(_) => {},
            _ => panic!("Expected Serialization error"),
        }
    }

    #[test]
    fn test_api_error_from_auth_error() {
        let auth_error = crate::auth::AuthError::MissingToken;
        let api_error: ApiError = auth_error.into();
        
        match api_error {
            ApiError::Authentication(crate::auth::AuthError::MissingToken) => {},
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
            ApiError::ValidationError(_) => {},
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_scheduler_error_variants() {
        // Test various scheduler error types
        let test_cases = vec![
            SchedulerError::TaskNotFound { id: 1 },
            SchedulerError::WorkerNotFound { id: "test".to_string() },
            SchedulerError::TaskRunNotFound { id: 1 },
            SchedulerError::InvalidTaskParams("Invalid params".to_string()),
            SchedulerError::InvalidCron { expr: "invalid".to_string(), message: "Invalid cron expression".to_string() },
            SchedulerError::CircularDependency,
        ];

        for scheduler_error in test_cases {
            let api_error: ApiError = scheduler_error.into();
            let response = api_error.into_response();
            
            // All scheduler errors should result in appropriate status codes
            assert!(matches!(response.status(), 
                StatusCode::NOT_FOUND | 
                StatusCode::BAD_REQUEST | 
                StatusCode::INTERNAL_SERVER_ERROR));
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
