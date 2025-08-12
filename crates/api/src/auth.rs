pub mod models;
pub mod service;
pub mod guards;

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{Duration, Utc};
use headers::{authorization::Bearer, Authorization, HeaderMapExt};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tracing::{error, warn};

pub const API_KEY_HEADER: &str = "X-API-Key";
pub const BEARER_PREFIX: &str = "Bearer ";

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt_secret: String,
    pub api_keys: HashMap<String, ApiKeyInfo>,
    pub jwt_expiration_hours: i64,
}

#[derive(Debug, Clone)]
pub struct ApiKeyInfo {
    pub name: String,
    pub permissions: Vec<Permission>,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Permission {
    TaskRead,
    TaskWrite,
    TaskDelete,
    WorkerRead,
    WorkerWrite,
    SystemRead,
    SystemWrite,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
    pub permissions: Vec<String>,
    pub user_id: String,
}

#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ExpiredToken,
    InsufficientPermissions,
    InvalidApiKey,
    MalformedHeader,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingToken => write!(f, "Missing authentication token"),
            AuthError::InvalidToken => write!(f, "Invalid authentication token"),
            AuthError::ExpiredToken => write!(f, "Authentication token has expired"),
            AuthError::InsufficientPermissions => write!(f, "Insufficient permissions"),
            AuthError::InvalidApiKey => write!(f, "Invalid API key"),
            AuthError::MalformedHeader => write!(f, "Malformed authorization header"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<AuthError> for StatusCode {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::MissingToken | AuthError::InvalidToken | AuthError::InvalidApiKey => {
                StatusCode::UNAUTHORIZED
            }
            AuthError::ExpiredToken => StatusCode::UNAUTHORIZED,
            AuthError::InsufficientPermissions => StatusCode::FORBIDDEN,
            AuthError::MalformedHeader => StatusCode::BAD_REQUEST,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub permissions: Vec<Permission>,
    pub auth_type: AuthType,
}

#[derive(Debug, Clone)]
pub enum AuthType {
    ApiKey(String),
    Jwt(Claims),
}

impl AuthenticatedUser {
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission) || self.permissions.contains(&Permission::Admin)
    }

    pub fn require_permission(&self, permission: Permission) -> Result<(), AuthError> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(AuthError::InsufficientPermissions)
        }
    }
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expiration_hours: i64,
}

impl JwtService {
    pub fn new(secret: &str, expiration_hours: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_ref()),
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
            expiration_hours,
        }
    }

    pub fn generate_token(
        &self,
        user_id: &str,
        permissions: &[Permission],
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.expiration_hours);

        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            permissions: permissions.iter().map(|p| format!("{p:?}")).collect(),
            user_id: user_id.to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)?;
        Ok(token_data.claims)
    }
}

pub struct ApiKeyService {
    keys: HashMap<String, ApiKeyInfo>,
}

impl ApiKeyService {
    pub fn new(keys: HashMap<String, ApiKeyInfo>) -> Self {
        Self { keys }
    }

    pub fn validate_api_key(&self, api_key: &str) -> Result<&ApiKeyInfo, AuthError> {
        let hashed_key = self.hash_api_key(api_key);
        self.keys
            .get(&hashed_key)
            .filter(|info| info.is_active)
            .ok_or(AuthError::InvalidApiKey)
    }

    fn hash_api_key(&self, api_key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        general_purpose::STANDARD.encode(hasher.finalize())
    }

    pub fn generate_api_key() -> String {
        use rand::Rng;
        let mut rng = rand::rng();
        let key: [u8; 32] = rng.random();
        general_purpose::STANDARD.encode(key)
    }
}

pub async fn auth_middleware(
    State(state): State<crate::routes::AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    match extract_auth_info(&req, &state.auth_config) {
        Ok(user) => {
            req.extensions_mut().insert(user);
            Ok(next.run(req).await)
        }
        Err(err) => {
            warn!("Authentication failed: {}", err);
            Err(err.into())
        }
    }
}

pub async fn optional_auth_middleware(
    State(state): State<crate::routes::AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Ok(user) = extract_auth_info(&req, &state.auth_config) {
        req.extensions_mut().insert(user);
    } else if !state.auth_config.enabled {
        // When auth is disabled, insert a default admin user
        let default_user = AuthenticatedUser {
            user_id: "test-user".to_string(),
            permissions: vec![Permission::Admin], // Grant all permissions
            auth_type: AuthType::ApiKey("test-key".to_string()),
        };
        req.extensions_mut().insert(default_user);
    }
    Ok(next.run(req).await)
}

fn extract_auth_info(req: &Request, config: &AuthConfig) -> Result<AuthenticatedUser, AuthError> {
    if let Some(api_key) = extract_api_key(req) {
        return validate_api_key(&api_key, config);
    }

    if let Some(token) = extract_jwt_token(req) {
        return validate_jwt_token(&token, config);
    }

    Err(AuthError::MissingToken)
}

fn extract_api_key(req: &Request) -> Option<String> {
    req.headers()
        .get(API_KEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn extract_jwt_token(req: &Request) -> Option<String> {
    req.headers()
        .typed_get::<Authorization<Bearer>>()
        .map(|auth| auth.token().to_string())
        .or_else(|| {
            req.headers()
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .filter(|s| s.starts_with(BEARER_PREFIX))
                .map(|s| s[BEARER_PREFIX.len()..].to_string())
        })
}

fn validate_api_key(api_key: &str, config: &AuthConfig) -> Result<AuthenticatedUser, AuthError> {
    let api_service = ApiKeyService::new(config.api_keys.clone());
    let key_info = api_service.validate_api_key(api_key)?;

    Ok(AuthenticatedUser {
        user_id: key_info.name.clone(),
        permissions: key_info.permissions.clone(),
        auth_type: AuthType::ApiKey(api_key.to_string()),
    })
}

fn validate_jwt_token(token: &str, config: &AuthConfig) -> Result<AuthenticatedUser, AuthError> {
    let jwt_service = JwtService::new(&config.jwt_secret, config.jwt_expiration_hours);
    let claims = jwt_service.validate_token(token).map_err(|err| {
        error!("JWT validation failed: {}", err);
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
            _ => AuthError::InvalidToken,
        }
    })?;

    let permissions = claims
        .permissions
        .iter()
        .filter_map(|p| parse_permission(p))
        .collect();

    Ok(AuthenticatedUser {
        user_id: claims.user_id.clone(),
        permissions,
        auth_type: AuthType::Jwt(claims),
    })
}

pub fn parse_permission(permission_str: &str) -> Option<Permission> {
    match permission_str {
        "TaskRead" => Some(Permission::TaskRead),
        "TaskWrite" => Some(Permission::TaskWrite),
        "TaskDelete" => Some(Permission::TaskDelete),
        "WorkerRead" => Some(Permission::WorkerRead),
        "WorkerWrite" => Some(Permission::WorkerWrite),
        "SystemRead" => Some(Permission::SystemRead),
        "SystemWrite" => Some(Permission::SystemWrite),
        "Admin" => Some(Permission::Admin),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jwt_service() {
        let jwt_service = JwtService::new("test-secret", 24);
        let permissions = vec![Permission::TaskRead, Permission::TaskWrite];

        let token = jwt_service
            .generate_token("test-user", &permissions)
            .unwrap();
        let claims = jwt_service.validate_token(&token).unwrap();

        assert_eq!(claims.user_id, "test-user");
        assert_eq!(claims.permissions.len(), 2);
    }

    #[tokio::test]
    async fn test_api_key_service() {
        let mut keys = HashMap::new();
        keys.insert(
            "hashed-key".to_string(),
            ApiKeyInfo {
                name: "test-key".to_string(),
                permissions: vec![Permission::TaskRead],
                is_active: true,
            },
        );

        let _api_service = ApiKeyService::new(keys);
    }

    #[test]
    fn test_permission_parsing() {
        assert_eq!(parse_permission("TaskRead"), Some(Permission::TaskRead));
        assert_eq!(parse_permission("Admin"), Some(Permission::Admin));
        assert_eq!(parse_permission("Invalid"), None);
    }

    #[test]
    fn test_authenticated_user_permissions() {
        let user = AuthenticatedUser {
            user_id: "test".to_string(),
            permissions: vec![Permission::TaskRead, Permission::TaskWrite],
            auth_type: AuthType::ApiKey("test-key".to_string()),
        };

        assert!(user.has_permission(Permission::TaskRead));
        assert!(!user.has_permission(Permission::TaskDelete));
        assert!(user.require_permission(Permission::TaskWrite).is_ok());
        assert!(user.require_permission(Permission::TaskDelete).is_err());
    }

    #[test]
    fn test_admin_permission() {
        let admin_user = AuthenticatedUser {
            user_id: "admin".to_string(),
            permissions: vec![Permission::Admin],
            auth_type: AuthType::ApiKey("admin-key".to_string()),
        };

        assert!(admin_user.has_permission(Permission::TaskRead));
        assert!(admin_user.has_permission(Permission::TaskDelete));
        assert!(admin_user.has_permission(Permission::SystemWrite));
    }
}
