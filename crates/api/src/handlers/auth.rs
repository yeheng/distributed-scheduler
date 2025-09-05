use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    auth::models::{LoginRequest, LoginResponse},
    auth::{JwtService, Permission, RefreshTokenService},
    error::{ApiError, ApiResult},
    response::ApiResponse,
};
use scheduler_application::AuthenticationService;
use scheduler_domain::repositories::UserRole;

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

    // 使用AuthenticationService进行用户认证
    let user = state
        .authentication_service
        .authenticate_user(&request.username, &request.password)
        .await
        .map_err(|e| ApiError::Internal(format!("Authentication failed: {e}")))?;

    let user = user.ok_or_else(|| {
        ApiError::Authentication(crate::auth::AuthError::InvalidToken)
    })?;

    // 将用户角色转换为权限列表
    let permissions = convert_user_role_to_permissions(&user.role);

    let jwt_service = JwtService::new(
        &state.auth_config.jwt_secret,
        state.auth_config.jwt_expiration_hours,
    );

    let refresh_service = RefreshTokenService::new(&state.auth_config.jwt_secret);

    let access_token = jwt_service
        .generate_token(&user.id.to_string(), &permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to generate token: {e}")))?;

    let refresh_token = refresh_service
        .generate_refresh_token(&user.id.to_string(), &permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to generate refresh token: {e}")))?;

    let response = LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth_config.jwt_expiration_hours * 3600,
        user: crate::auth::models::UserResponse {
            id: user.id,
            username: user.username.clone(),
            email: user.email.clone(),
            role: convert_domain_role_to_api_role(user.role),
            permissions,
        },
    };

    info!("User {} logged in successfully", user.username);

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

    let refresh_service = RefreshTokenService::new(&state.auth_config.jwt_secret);

    // Validate the refresh token using the proper service
    let refresh_data = refresh_service
        .validate_refresh_token(&request.refresh_token)
        .map_err(|_| ApiError::Authentication(crate::auth::AuthError::InvalidToken))?;

    // Parse permissions from the refresh token
    let permissions: Vec<Permission> = refresh_data
        .permissions
        .iter()
        .filter_map(|p| crate::auth::parse_permission(p))
        .collect();

    // Generate new access token
    let access_token = jwt_service
        .generate_token(&refresh_data.user_id, &permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to generate token: {e}")))?;

    // Generate new refresh token
    let new_refresh_token = refresh_service
        .generate_refresh_token(&refresh_data.user_id, &permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to generate refresh token: {e}")))?;

    // Revoke the old refresh token
    let _ = refresh_service.revoke_refresh_token(&request.refresh_token);

    let response = LoginResponse {
        access_token,
        refresh_token: new_refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth_config.jwt_expiration_hours * 3600,
        user: crate::auth::models::UserResponse {
            id: uuid::Uuid::new_v4(),
            username: refresh_data.user_id.clone(),
            email: format!("{}@example.com", refresh_data.user_id),
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

/// 将用户域角色转换为API权限列表
fn convert_user_role_to_permissions(role: &UserRole) -> Vec<Permission> {
    match role {
        UserRole::Admin => vec![Permission::Admin],
        UserRole::Operator => vec![
            Permission::TaskRead,
            Permission::TaskWrite,
            Permission::WorkerRead,
            Permission::SystemRead,
        ],
        UserRole::Viewer => vec![
            Permission::TaskRead,
            Permission::WorkerRead,
            Permission::SystemRead,
        ],
    }
}

/// 将域用户角色转换为API用户角色
fn convert_domain_role_to_api_role(domain_role: UserRole) -> crate::auth::models::UserRole {
    match domain_role {
        UserRole::Admin => crate::auth::models::UserRole::Admin,
        UserRole::Operator => crate::auth::models::UserRole::Operator,
        UserRole::Viewer => crate::auth::models::UserRole::Viewer,
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

    #[test]
    fn test_convert_user_role_to_permissions() {
        let admin_permissions = convert_user_role_to_permissions(&UserRole::Admin);
        assert!(admin_permissions.contains(&Permission::Admin));

        let operator_permissions = convert_user_role_to_permissions(&UserRole::Operator);
        assert!(operator_permissions.contains(&Permission::TaskRead));
        assert!(operator_permissions.contains(&Permission::TaskWrite));
        assert!(operator_permissions.contains(&Permission::WorkerRead));
        assert!(operator_permissions.contains(&Permission::SystemRead));

        let viewer_permissions = convert_user_role_to_permissions(&UserRole::Viewer);
        assert!(viewer_permissions.contains(&Permission::TaskRead));
        assert!(viewer_permissions.contains(&Permission::WorkerRead));
        assert!(viewer_permissions.contains(&Permission::SystemRead));
    }

    #[test]
    fn test_convert_domain_role_to_api_role() {
        assert_eq!(
            convert_domain_role_to_api_role(UserRole::Admin),
            crate::auth::models::UserRole::Admin
        );
        assert_eq!(
            convert_domain_role_to_api_role(UserRole::Operator),
            crate::auth::models::UserRole::Operator
        );
        assert_eq!(
            convert_domain_role_to_api_role(UserRole::Viewer),
            crate::auth::models::UserRole::Viewer
        );
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
