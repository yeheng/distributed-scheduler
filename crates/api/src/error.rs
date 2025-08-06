use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use scheduler_core::errors::SchedulerError;
use serde_json::json;

/// API错误类型
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("调度器错误: {0}")]
    Scheduler(#[from] SchedulerError),

    #[error("验证错误: {0}")]
    Validation(String),

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

/// API结果类型
pub type ApiResult<T> = Result<T, ApiError>;
