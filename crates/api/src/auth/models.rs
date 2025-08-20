use crate::auth::{AuthType, AuthenticatedUser, Claims, Permission};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: UserRole,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    Admin,
    Operator,
    Viewer,
}

impl UserRole {
    pub fn permissions(&self) -> Vec<Permission> {
        match self {
            UserRole::Admin => vec![
                Permission::Admin,
                Permission::TaskRead,
                Permission::TaskWrite,
                Permission::TaskDelete,
                Permission::WorkerRead,
                Permission::WorkerWrite,
                Permission::SystemRead,
                Permission::SystemWrite,
            ],
            UserRole::Operator => vec![
                Permission::TaskRead,
                Permission::TaskWrite,
                Permission::WorkerRead,
                Permission::WorkerWrite,
                Permission::SystemRead,
            ],
            UserRole::Viewer => vec![
                Permission::TaskRead,
                Permission::WorkerRead,
                Permission::SystemRead,
            ],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: UserRole,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: UserRole,
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            role: user.role,
            permissions: user.role.permissions(),
        }
    }
}

impl From<AuthenticatedUser> for UserResponse {
    fn from(user: AuthenticatedUser) -> Self {
        let role = match user.auth_type {
            AuthType::Jwt(_) => {
                if user.permissions.contains(&Permission::Admin) {
                    UserRole::Admin
                } else if user.permissions.contains(&Permission::TaskWrite) {
                    UserRole::Operator
                } else {
                    UserRole::Viewer
                }
            }
            AuthType::ApiKey(_) => UserRole::Operator,
        };

        Self {
            id: Uuid::new_v4(),
            username: user.user_id.clone(),
            email: format!("{}@example.com", user.user_id),
            role,
            permissions: user.permissions,
        }
    }
}

pub struct UserService {
    users: HashMap<Uuid, User>,
    username_index: HashMap<String, Uuid>,
}

impl UserService {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            username_index: HashMap::new(),
        }
    }

    pub async fn create_user(&mut self, request: CreateUserRequest) -> Result<User, String> {
        if self.username_index.contains_key(&request.username) {
            return Err("Username already exists".to_string());
        }

        let user_id = Uuid::new_v4();
        let password_hash = self.hash_password(&request.password)?;

        let now = chrono::Utc::now();
        let user = User {
            id: user_id,
            username: request.username.clone(),
            email: request.email,
            password_hash,
            role: request.role,
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        self.users.insert(user_id, user.clone());
        self.username_index.insert(request.username, user_id);

        Ok(user)
    }

    pub async fn authenticate_user(&self, username: &str, password: &str) -> Result<User, String> {
        let user_id = self.username_index.get(username).ok_or("User not found")?;

        let user = self.users.get(user_id).ok_or("User not found")?;

        if !user.is_active {
            return Err("User account is disabled".to_string());
        }

        self.verify_password(password, &user.password_hash)?;

        Ok(user.clone())
    }

    pub async fn get_user_by_id(&self, user_id: Uuid) -> Option<User> {
        self.users.get(&user_id).cloned()
    }

    pub async fn get_user_by_username(&self, username: &str) -> Option<User> {
        self.username_index
            .get(username)
            .and_then(|id| self.users.get(id))
            .cloned()
    }

    fn hash_password(&self, password: &str) -> Result<String, String> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| format!("Failed to hash password: {e}"))
    }

    fn verify_password(&self, password: &str, hash: &str) -> Result<(), String> {
        bcrypt::verify(password, hash)
            .map_err(|e| format!("Failed to verify password: {e}"))?
            .then_some(())
            .ok_or("Invalid password".to_string())
    }
}

impl Default for UserService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthType, AuthenticatedUser};

    #[test]
    fn test_user_role_admin_permissions() {
        let role = UserRole::Admin;
        let permissions = role.permissions();
        
        assert!(permissions.contains(&Permission::Admin));
        assert!(permissions.contains(&Permission::TaskRead));
        assert!(permissions.contains(&Permission::TaskWrite));
        assert!(permissions.contains(&Permission::TaskDelete));
        assert!(permissions.contains(&Permission::WorkerRead));
        assert!(permissions.contains(&Permission::WorkerWrite));
        assert!(permissions.contains(&Permission::SystemRead));
        assert!(permissions.contains(&Permission::SystemWrite));
    }

    #[test]
    fn test_user_role_operator_permissions() {
        let role = UserRole::Operator;
        let permissions = role.permissions();
        
        assert!(permissions.contains(&Permission::TaskRead));
        assert!(permissions.contains(&Permission::TaskWrite));
        assert!(permissions.contains(&Permission::WorkerRead));
        assert!(permissions.contains(&Permission::WorkerWrite));
        assert!(permissions.contains(&Permission::SystemRead));
        
        // Should not have admin permissions
        assert!(!permissions.contains(&Permission::Admin));
        assert!(!permissions.contains(&Permission::TaskDelete));
        assert!(!permissions.contains(&Permission::SystemWrite));
    }

    #[test]
    fn test_user_role_viewer_permissions() {
        let role = UserRole::Viewer;
        let permissions = role.permissions();
        
        assert!(permissions.contains(&Permission::TaskRead));
        assert!(permissions.contains(&Permission::WorkerRead));
        assert!(permissions.contains(&Permission::SystemRead));
        
        // Should only have read permissions
        assert!(!permissions.contains(&Permission::TaskWrite));
        assert!(!permissions.contains(&Permission::TaskDelete));
        assert!(!permissions.contains(&Permission::WorkerWrite));
        assert!(!permissions.contains(&Permission::SystemWrite));
        assert!(!permissions.contains(&Permission::Admin));
    }

    #[test]
    fn test_user_from_authenticated_user_jwt_admin() {
        let user = AuthenticatedUser {
            user_id: "admin_user".to_string(),
            permissions: vec![Permission::Admin],
            auth_type: AuthType::Jwt(crate::auth::Claims {
                sub: "admin_user".to_string(),
                exp: 0,
                iat: 0,
                permissions: vec
!["Admin".to_string()],
                user_id: "admin_user".to_string(),
            }),
        };

        let user_response: UserResponse = user.into();
        assert_eq!(user_response.role, UserRole::Admin);
    }

    #[test]
    fn test_user_from_authenticated_user_jwt_operator() {
        let user = AuthenticatedUser {
            user_id: "operator_user".to_string(),
            permissions: vec![Permission::TaskWrite],
            auth_type: AuthType::Jwt(crate::auth::Claims {
                sub: "operator_user".to_string(),
                exp: 0,
                iat: 0,
                permissions: vec
!["TaskWrite".to_string()],
                user_id: "operator_user".to_string(),
            }),
        };

        let user_response: UserResponse = user.into();
        assert_eq!(user_response.role, UserRole::Operator);
    }

    #[test]
    fn test_user_from_authenticated_user_jwt_viewer() {
        let user = AuthenticatedUser {
            user_id: "viewer_user".to_string(),
            permissions: vec![Permission::TaskRead],
            auth_type: AuthType::Jwt(crate::auth::Claims {
                sub: "viewer_user".to_string(),
                exp: 0,
                iat: 0,
                permissions: vec
!["TaskRead".to_string()],
                user_id: "viewer_user".to_string(),
            }),
        };

        let user_response: UserResponse = user.into();
        assert_eq!(user_response.role, UserRole::Viewer);
    }

    #[test]
    fn test_user_from_authenticated_user_api_key() {
        let user = AuthenticatedUser {
            user_id: "api_user".to_string(),
            permissions: vec![Permission::TaskRead],
            auth_type: AuthType::ApiKey("test_key".to_string()),
        };

        let user_response: UserResponse = user.into();
        assert_eq!(user_response.role, UserRole::Operator);
    }

    #[tokio::test]
    async fn test_user_service_create_user() {
        let mut service = UserService::new();
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            role: UserRole::Viewer,
        };

        let result = service.create_user(request).await;
        assert!(result.is_ok());

        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.role, UserRole::Viewer);
        assert!(user.is_active);
    }

    #[tokio::test]
    async fn test_user_service_create_duplicate_user() {
        let mut service = UserService::new();
        let request1 = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test1@example.com".to_string(),
            password: "password123".to_string(),
            role: UserRole::Viewer,
        };

        let request2 = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test2@example.com".to_string(),
            password: "password456".to_string(),
            role: UserRole::Operator,
        };

        // First creation should succeed
        let result1 = service.create_user(request1).await;
        assert!(result1.is_ok());

        // Second creation with same username should fail
        let result2 = service.create_user(request2).await;
        assert!(result2.is_err());
    }

    #[tokio::test]
    async fn test_user_service_authenticate_user() {
        let mut service = UserService::new();
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            role: UserRole::Viewer,
        };

        // Create user first
        service.create_user(request).await.unwrap();

        // Authentication should succeed
        let result = service.authenticate_user("testuser", "password123").await;
        assert!(result.is_ok());

        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
    }

    #[tokio::test]
    async fn test_user_service_authenticate_wrong_password() {
        let mut service = UserService::new();
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            role: UserRole::Viewer,
        };

        // Create user first
        service.create_user(request).await.unwrap();

        // Authentication with wrong password should fail
        let result = service.authenticate_user("testuser", "wrongpassword").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_user_service_authenticate_nonexistent_user() {
        let service = UserService::new();

        // Authentication with non-existent user should fail
        let result = service.authenticate_user("nonexistent", "password123").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_user_service_get_user_by_id() {
        let mut service = UserService::new();
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            role: UserRole::Viewer,
        };

        // Create user first
        let created_user = service.create_user(request).await.unwrap();

        // Get user by ID should succeed
        let result = service.get_user_by_id(created_user.id).await;
        assert!(result.is_some());

        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
    }

    #[tokio::test]
    async fn test_user_service_get_user_by_username() {
        let mut service = UserService::new();
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            role: UserRole::Viewer,
        };

        // Create user first
        service.create_user(request).await.unwrap();

        // Get user by username should succeed
        let result = service.get_user_by_username("testuser").await;
        assert!(result.is_some());

        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
    }

    #[test]
    fn test_user_response_from_user() {
        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let user = User {
            id: user_id,
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hashed_password".to_string(),
            role: UserRole::Operator,
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        let user_response: UserResponse = user.into();
        assert_eq!(user_response.id, user_id);
        assert_eq!(user_response.username, "testuser");
        assert_eq!(user_response.email, "test@example.com");
        assert_eq!(user_response.role, UserRole::Operator);
        
        // Should contain operator permissions
        assert!(user_response.permissions.contains(&Permission::TaskRead));
        assert!(user_response.permissions.contains(&Permission::TaskWrite));
    }

    #[test]
    fn test_user_role_serialization() {
        let role = UserRole::Admin;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"Admin\"");

        let deserialized: UserRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, UserRole::Admin);
    }

    #[test]
    fn test_create_user_request_serialization() {
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            role: UserRole::Viewer,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"username\":\"testuser\""));
        assert!(json.contains("\"email\":\"test@example.com\""));
        assert!(json.contains("\"role\":\"Viewer\""));

        let deserialized: CreateUserRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.username, "testuser");
        assert_eq!(deserialized.email, "test@example.com");
        assert_eq!(deserialized.role, UserRole::Viewer);
    }

    #[test]
    fn test_login_request_serialization() {
        let request = LoginRequest {
            username: "testuser".to_string(),
            password: "password123".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"username\":\"testuser\""));
        assert!(json.contains("\"password\":\"password123\""));

        let deserialized: LoginRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.username, "testuser");
        assert_eq!(deserialized.password, "password123");
    }
}
