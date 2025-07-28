use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

/// 标准API响应结构
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 创建成功响应带消息
    pub fn success_with_message(data: T, message: String) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

impl ApiResponse<()> {
    /// 创建无数据的成功响应
    pub fn success_empty() -> Self {
        Self {
            success: true,
            data: None,
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 创建无数据的成功响应带消息
    pub fn success_empty_with_message(message: String) -> Self {
        Self {
            success: true,
            data: None,
            message: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

/// 分页响应结构
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, total: i64, page: i64, page_size: i64) -> Self {
        let total_pages = if page_size > 0 {
            (total + page_size - 1) / page_size
        } else {
            0
        };

        Self {
            items,
            total,
            page,
            page_size,
            total_pages,
        }
    }
}

/// 创建成功响应的便捷函数
pub fn success<T: Serialize>(data: T) -> impl IntoResponse {
    (StatusCode::OK, ApiResponse::success(data))
}

/// 创建创建成功响应的便捷函数
pub fn created<T: Serialize>(data: T) -> impl IntoResponse {
    (StatusCode::CREATED, ApiResponse::success(data))
}

/// 创建无内容响应的便捷函数
pub fn no_content() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

/// 创建接受响应的便捷函数
pub fn accepted() -> impl IntoResponse {
    (StatusCode::ACCEPTED, ApiResponse::success_empty())
}
