use crate::auth::{AuthConfig, AuthType, AuthenticatedUser, Claims};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Header};
use uuid::Uuid;

pub struct AuthService {
    jwt_service: crate::auth::JwtService,
    user_service: crate::auth::models::UserService,
}

impl AuthService {
    pub fn new(config: &AuthConfig) -> Self {
        Self {
            jwt_service: crate::auth::JwtService::new(
                &config.jwt_secret,
                config.jwt_expiration_hours,
            ),
            user_service: crate::auth::models::UserService::new(),
        }
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<crate::auth::models::LoginResponse, String> {
        let user = self
            .user_service
            .authenticate_user(username, password)
            .await?;

        let permissions = user.role.permissions();
        let access_token = self
            .jwt_service
            .generate_token(&user.id.to_string(), &permissions)
            .map_err(|e| format!("Failed to generate access token: {e}"))?;

        let refresh_token = self.generate_refresh_token(&user.id)?;

        let expires_in = self.jwt_service.expiration_hours * 3600;

        Ok(crate::auth::models::LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
            user: user.into(),
        })
    }

    pub async fn create_user(
        &mut self,
        request: crate::auth::models::CreateUserRequest,
    ) -> Result<crate::auth::models::UserResponse, String> {
        let user = self.user_service.create_user(request).await?;
        Ok(user.into())
    }

    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<crate::auth::models::LoginResponse, String> {
        let user_id = self.validate_refresh_token(refresh_token)?;

        let user = self
            .user_service
            .get_user_by_id(user_id)
            .await
            .ok_or("User not found")?;

        let permissions = user.role.permissions();
        let access_token = self
            .jwt_service
            .generate_token(&user.id.to_string(), &permissions)
            .map_err(|e| format!("Failed to generate access token: {e}"))?;

        let new_refresh_token = self.generate_refresh_token(&user_id)?;

        let expires_in = self.jwt_service.expiration_hours * 3600;

        Ok(crate::auth::models::LoginResponse {
            access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
            user: user.into(),
        })
    }

    pub async fn validate_access_token(&self, token: &str) -> Result<AuthenticatedUser, String> {
        let claims = self
            .jwt_service
            .validate_token(token)
            .map_err(|e| format!("Invalid token: {e}"))?;

        let user_id =
            Uuid::parse_str(&claims.user_id).map_err(|e| format!("Invalid user ID: {e}"))?;

        let user = self
            .user_service
            .get_user_by_id(user_id)
            .await
            .ok_or("User not found")?;

        let permissions = user.role.permissions();

        Ok(AuthenticatedUser {
            user_id: user.username,
            permissions,
            auth_type: AuthType::Jwt(claims),
        })
    }

    fn generate_refresh_token(&self, user_id: &Uuid) -> Result<String, String> {
        let now = Utc::now();
        let exp = now + Duration::days(30);

        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            permissions: vec!["refresh".to_string()],
            user_id: user_id.to_string(),
        };

        encode(&Header::default(), &claims, &self.jwt_service.encoding_key)
            .map_err(|e| format!("Failed to generate refresh token: {e}"))
    }

    fn validate_refresh_token(&self, refresh_token: &str) -> Result<Uuid, String> {
        let claims = self
            .jwt_service
            .validate_token(refresh_token)
            .map_err(|e| format!("Invalid refresh token: {e}"))?;

        if !claims.permissions.contains(&"refresh".to_string()) {
            return Err("Invalid refresh token".to_string());
        }

        Uuid::parse_str(&claims.user_id).map_err(|e| format!("Invalid user ID: {e}"))
    }

    pub fn get_user_service(&self) -> &crate::auth::models::UserService {
        &self.user_service
    }

    pub fn get_user_service_mut(&mut self) -> &mut crate::auth::models::UserService {
        &mut self.user_service
    }
}
