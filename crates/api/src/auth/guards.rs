use axum::{
    extract::{Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use crate::auth::{AuthenticatedUser, Permission};

pub async fn require_permission_middleware(
    permission: Permission,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let user = req.extensions().get::<AuthenticatedUser>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    if user.has_permission(permission) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

pub async fn require_role_middleware(
    role: crate::auth::models::UserRole,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let user = req.extensions().get::<AuthenticatedUser>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let has_required_role = match role {
        crate::auth::models::UserRole::Admin => user.permissions.contains(&Permission::Admin),
        crate::auth::models::UserRole::Operator => 
            user.permissions.contains(&Permission::TaskWrite) || user.permissions.contains(&Permission::Admin),
        crate::auth::models::UserRole::Viewer => 
            user.permissions.contains(&Permission::TaskRead) || 
            user.permissions.contains(&Permission::TaskWrite) || 
            user.permissions.contains(&Permission::Admin),
    };
    
    if has_required_role {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

pub async fn optional_auth_middleware(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // This middleware allows requests to proceed without authentication
    // but sets user context if authentication is provided
    Ok(next.run(req).await)
}