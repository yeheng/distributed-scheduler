use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }
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
    pub fn success_empty() -> Self {
        Self {
            success: true,
            data: None,
            message: None,
            timestamp: chrono::Utc::now(),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub fn success<T: Serialize>(data: T) -> impl IntoResponse {
    (StatusCode::OK, ApiResponse::success(data))
}

pub fn created<T: Serialize>(data: T) -> impl IntoResponse {
    (StatusCode::CREATED, ApiResponse::success(data))
}

pub fn no_content() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

pub fn accepted() -> impl IntoResponse {
    (StatusCode::ACCEPTED, ApiResponse::success_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_api_response_success() {
        let data = "test_data";
        let response = ApiResponse::success(data);
        
        assert!(response.success);
        assert_eq!(response.data, Some("test_data"));
        assert!(response.message.is_none());
        assert!(response.timestamp <= Utc::now());
    }

    #[test]
    fn test_api_response_success_with_message() {
        let data = "test_data";
        let message = "Operation successful".to_string();
        let response = ApiResponse::success_with_message(data, message.clone());
        
        assert!(response.success);
        assert_eq!(response.data, Some("test_data"));
        assert_eq!(response.message, Some(message));
    }

    #[test]
    fn test_api_response_success_empty() {
        let response = ApiResponse::success_empty();
        
        assert!(response.success);
        assert!(response.data.is_none());
        assert!(response.message.is_none());
    }

    #[test]
    fn test_api_response_success_empty_with_message() {
        let message = "Operation completed".to_string();
        let response = ApiResponse::success_empty_with_message(message.clone());
        
        assert!(response.success);
        assert!(response.data.is_none());
        assert_eq!(response.message, Some(message));
    }

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse::success("test_data");
        let json = serde_json::to_string(&response).unwrap();
        
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"data\":\"test_data\""));
        assert!(json.contains("\"timestamp\""));
    }

    #[test]
    fn test_api_response_deserialization() {
        let json_str = r#"{
            "success": true,
            "data": "test_data",
            "message": "test message",
            "timestamp": "2024-01-01T00:00:00Z"
        }"#;
        
        let response: ApiResponse<String> = serde_json::from_str(json_str).unwrap();
        
        assert!(response.success);
        assert_eq!(response.data, Some("test_data".to_string()));
        assert_eq!(response.message, Some("test message".to_string()));
    }

    #[test]
    fn test_paginated_response_new() {
        let items = vec
!["item1", "item2", "item3"];
        let total = 10;
        let page = 2;
        let page_size = 3;
        
        let response: PaginatedResponse<&str> = PaginatedResponse::new(items.clone(), total, page, page_size);
        
        assert_eq!(response.items, items);
        assert_eq!(response.total, total);
        assert_eq!(response.page, page);
        assert_eq!(response.page_size, page_size);
        assert_eq!(response.total_pages, 4); // (10 + 3 - 1) / 3 = 4
    }

    #[test]
    fn test_paginated_response_zero_page_size() {
        let items = vec
!["item1", "item2"];
        let total = 2;
        let page = 1;
        let page_size = 0;
        
        let response: PaginatedResponse<&str> = PaginatedResponse::new(items.clone(), total, page, page_size);
        
        assert_eq!(response.items, items);
        assert_eq!(response.total, total);
        assert_eq!(response.page, page);
        assert_eq!(response.page_size, page_size);
        assert_eq!(response.total_pages, 0);
    }

    #[test]
    fn test_paginated_response_single_page() {
        let items = vec
!["item1", "item2"];
        let total = 2;
        let page = 1;
        let page_size = 10;
        
        let response: PaginatedResponse<&str> = PaginatedResponse::new(items.clone(), total, page, page_size);
        
        assert_eq!(response.items, items);
        assert_eq!(response.total, total);
        assert_eq!(response.page, page);
        assert_eq!(response.page_size, page_size);
        assert_eq!(response.total_pages, 1);
    }

    #[test]
    fn test_paginated_response_last_page() {
        let items = vec
!["item3"]; // Last page with 1 item
        let total = 10;
        let page = 4;
        let page_size = 3;
        
        let response: PaginatedResponse<&str> = PaginatedResponse::new(items.clone(), total, page, page_size);
        
        assert_eq!(response.items, items);
        assert_eq!(response.total, total);
        assert_eq!(response.page, page);
        assert_eq!(response.page_size, page_size);
        assert_eq!(response.total_pages, 4);
    }

    #[test]
    fn test_paginated_response_serialization() {
        let items = vec
!["item1", "item2"];
        let response: PaginatedResponse<&str> = PaginatedResponse::new(items, 5, 1, 2);
        let json = serde_json::to_string(&response).unwrap();
        
        assert!(json.contains("\"items\":[\"item1\",\"item2\"]"));
        assert!(json.contains("\"total\":5"));
        assert!(json.contains("\"page\":1"));
        assert!(json.contains("\"page_size\":2"));
        assert!(json.contains("\"total_pages\":3"));
    }

    #[test]
    fn test_success_helper_function() {
        let data = "test_data";
        let response = success(data);
        
        // This function returns an IntoResponse, so we can't easily test the exact structure
        // but we can verify it doesn't panic
        let _response = response;
    }

    #[test]
    fn test_created_helper_function() {
        let data = "test_data";
        let response = created(data);
        
        let _response = response;
    }

    #[test]
    fn test_no_content_helper_function() {
        let response = no_content();
        
        let _response = response;
    }

    #[test]
    fn test_accepted_helper_function() {
        let response = accepted();
        
        let _response = response;
    }

    #[test]
    fn test_api_response_into_response() {
        let response = ApiResponse::success("test");
        let http_response = response.into_response();
        
        // Verify the response can be converted without panicking
        let _http_response = http_response;
    }

    #[test]
    fn test_api_response_clone() {
        let response = ApiResponse::success("test_data");
        let cloned = response.clone();
        
        assert_eq!(response.success, cloned.success);
        assert_eq!(response.data, cloned.data);
        assert_eq!(response.message, cloned.message);
        // Note: timestamp might differ slightly
    }

    #[test]
    fn test_api_response_debug() {
        let response = ApiResponse::success("test_data");
        let debug_str = format!("{:?}", response);
        
        assert!(debug_str.contains("ApiResponse"));
        assert!(debug_str.contains("success: true"));
    }

    #[test]
    fn test_paginated_response_edge_cases() {
        // Empty items
        let response: PaginatedResponse<&str> = PaginatedResponse::new(vec
![], 0, 1, 10);
        assert_eq!(response.items.len(), 0);
        assert_eq!(response.total, 0);
        assert_eq!(response.total_pages, 0);
        
        // Single item
        let response: PaginatedResponse<&str> = PaginatedResponse::new(vec
!["single"], 1, 1, 10);
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.total, 1);
        assert_eq!(response.total_pages, 1);
    }
}
