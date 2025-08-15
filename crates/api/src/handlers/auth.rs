use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    auth::models::{LoginRequest, LoginResponse},
    auth::{JwtService, Permission},
    error::{ApiError, ApiResult},
    response::ApiResponse,
};

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub permissions: Vec<String>,
    pub expires_in_days: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub api_key: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ValidateTokenResponse {
    pub valid: bool,
    pub user_id: String,
    pub permissions: Vec<String>,
    pub expires_at: i64,
}

pub async fn login(
    State(state): State<crate::routes::AppState>,
    Json(request): Json<LoginRequest>,
) -> ApiResult<Json<ApiResponse<LoginResponse>>> {
    info!("User login attempt: {}", request.username);

    // For now, use the existing credential validation for backward compatibility
    let (user_id, permissions) = validate_user_credentials(&request.username, &request.password)?;

    let jwt_service = JwtService::new(
        &state.auth_config.jwt_secret,
        state.auth_config.jwt_expiration_hours,
    );

    let access_token = jwt_service
        .generate_token(&user_id, &permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to generate token: {e}")))?;

    let response = LoginResponse {
        access_token,
        refresh_token: "dummy_refresh_token".to_string(), // TODO: Implement proper refresh tokens
        token_type: "Bearer".to_string(),
        expires_in: state.auth_config.jwt_expiration_hours * 3600,
        user: crate::auth::models::UserResponse {
            id: uuid::Uuid::new_v4(),
            username: user_id.clone(),
            email: format!("{user_id}@example.com"),
            role: if permissions.contains(&Permission::Admin) {
                crate::auth::models::UserRole::Admin
            } else if permissions.contains(&Permission::TaskWrite) {
                crate::auth::models::UserRole::Operator
            } else {
                crate::auth::models::UserRole::Viewer
            },
            permissions,
        },
    };

    info!("User {} logged in successfully", user_id);

    Ok(Json(ApiResponse::success(response)))
}

pub async fn refresh_token(
    State(state): State<crate::routes::AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> ApiResult<Json<ApiResponse<LoginResponse>>> {
    info!("Token refresh request received");

    let jwt_service = JwtService::new(
        &state.auth_config.jwt_secret,
        state.auth_config.jwt_expiration_hours,
    );

    // Validate the refresh token (simplified for now)
    let claims = jwt_service
        .validate_token(&request.refresh_token)
        .map_err(|_| ApiError::Authentication(crate::auth::AuthError::InvalidToken))?;

    // Generate new access token
    let permissions: Vec<Permission> = claims
        .permissions
        .iter()
        .filter_map(|p| crate::auth::parse_permission(p))
        .collect();

    let access_token = jwt_service
        .generate_token(&claims.user_id, &permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to generate token: {e}")))?;

    let response = LoginResponse {
        access_token,
        refresh_token: request.refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth_config.jwt_expiration_hours * 3600,
        user: crate::auth::models::UserResponse {
            id: uuid::Uuid::new_v4(),
            username: claims.user_id.clone(),
            email: format!("{}@example.com", claims.user_id),
            role: if permissions.contains(&Permission::Admin) {
                crate::auth::models::UserRole::Admin
            } else if permissions.contains(&Permission::TaskWrite) {
                crate::auth::models::UserRole::Operator
            } else {
                crate::auth::models::UserRole::Viewer
            },
            permissions,
        },
    };

    Ok(Json(ApiResponse::success(response)))
}

pub async fn create_api_key(
    current_user: crate::auth::AuthenticatedUser,
    Json(request): Json<CreateApiKeyRequest>,
) -> ApiResult<Json<ApiResponse<CreateApiKeyResponse>>> {
    current_user.require_permission(Permission::Admin)?;

    info!(
        "Creating API key: {} for user: {}",
        request.name, current_user.user_id
    );
    let permissions: Result<Vec<Permission>, _> = request
        .permissions
        .iter()
        .map(|p| parse_permission_string(p))
        .collect();

    let permissions = permissions
        .map_err(|_| ApiError::BadRequest("Invalid permissions specified".to_string()))?;
    let api_key = crate::auth::ApiKeyService::generate_api_key();

    let created_at = chrono::Utc::now();
    let expires_at = request
        .expires_in_days
        .map(|days| created_at + chrono::Duration::days(days as i64));

    let response = CreateApiKeyResponse {
        api_key: api_key.clone(),
        name: request.name,
        permissions: permissions.iter().map(|p| format!("{p:?}")).collect(),
        created_at,
        expires_at,
    };

    info!("API key created successfully");

    Ok(Json(ApiResponse::success(response)))
}

pub async fn validate_token(
    current_user: crate::auth::AuthenticatedUser,
) -> ApiResult<Json<ApiResponse<ValidateTokenResponse>>> {
    let expires_at = match &current_user.auth_type {
        crate::auth::AuthType::Jwt(claims) => claims.exp,
        crate::auth::AuthType::ApiKey(_) => {
            (chrono::Utc::now() + chrono::Duration::days(365 * 10)).timestamp()
        }
    };

    let response = ValidateTokenResponse {
        valid: true,
        user_id: current_user.user_id,
        permissions: current_user
            .permissions
            .iter()
            .map(|p| format!("{p:?}"))
            .collect(),
        expires_at,
    };

    Ok(Json(ApiResponse::success(response)))
}

pub async fn logout(
    current_user: crate::auth::AuthenticatedUser,
) -> ApiResult<Json<ApiResponse<()>>> {
    info!("User {} logged out", current_user.user_id);

    Ok(Json(ApiResponse::success(())))
}
fn validate_user_credentials(
    username: &str,
    password: &str,
) -> ApiResult<(String, Vec<Permission>)> {
    match username {
        "admin" => {
            if password == "admin123" {
                Ok(("admin".to_string(), vec![Permission::Admin]))
            } else {
                Err(ApiError::Authentication(
                    crate::auth::AuthError::InvalidToken,
                ))
            }
        }
        "operator" => {
            if password == "op123" {
                Ok((
                    "operator".to_string(),
                    vec![
                        Permission::TaskRead,
                        Permission::TaskWrite,
                        Permission::WorkerRead,
                        Permission::SystemRead,
                    ],
                ))
            } else {
                Err(ApiError::Authentication(
                    crate::auth::AuthError::InvalidToken,
                ))
            }
        }
        "viewer" => {
            if password == "view123" {
                Ok((
                    "viewer".to_string(),
                    vec![
                        Permission::TaskRead,
                        Permission::WorkerRead,
                        Permission::SystemRead,
                    ],
                ))
            } else {
                Err(ApiError::Authentication(
                    crate::auth::AuthError::InvalidToken,
                ))
            }
        }
        _ => Err(ApiError::Authentication(
            crate::auth::AuthError::InvalidToken,
        )),
    }
}

fn parse_permission_string(permission: &str) -> Result<Permission, ()> {
    match permission {
        "TaskRead" => Ok(Permission::TaskRead),
        "TaskWrite" => Ok(Permission::TaskWrite),
        "TaskDelete" => Ok(Permission::TaskDelete),
        "WorkerRead" => Ok(Permission::WorkerRead),
        "WorkerWrite" => Ok(Permission::WorkerWrite),
        "SystemRead" => Ok(Permission::SystemRead),
        "SystemWrite" => Ok(Permission::SystemWrite),
        "Admin" => Ok(Permission::Admin),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_user_credentials() {
        let result = validate_user_credentials("admin", "admin123");
        assert!(result.is_ok());

        let (user_id, permissions) = result.unwrap();
        assert_eq!(user_id, "admin");
        assert!(permissions.contains(&Permission::Admin));
    }

    #[tokio::test]
    async fn test_invalid_credentials() {
        let result = validate_user_credentials("admin", "wrong_password");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_permission_string() {
        assert_eq!(parse_permission_string("Admin").unwrap(), Permission::Admin);
        assert_eq!(
            parse_permission_string("TaskRead").unwrap(),
            Permission::TaskRead
        );
        assert!(parse_permission_string("InvalidPermission").is_err());
    }
}
