use crate::auth::{AuthType, AuthenticatedUser, Permission};
use serde::{Deserialize, Serialize};
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
    // In-memory storage for demo purposes - in production this would use a database
    users: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<Uuid, User>>>,
}

impl UserService {
    pub fn new() -> Self {
        let users = std::collections::HashMap::new();
        Self {
            users: std::sync::Arc::new(std::sync::RwLock::new(users)),
        }
    }

    pub async fn authenticate_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<User, String> {
        let users = self.users.read().map_err(|e| format!("Lock error: {e}"))?;
        
        // Find user by username
        let user = users
            .values()
            .find(|u| u.username == username && u.is_active)
            .ok_or("Invalid username or password")?;

        // In production, you would use proper password hashing verification
        // For now, just check if the provided password matches a simple hash
        if self.verify_password(password, &user.password_hash)? {
            Ok(user.clone())
        } else {
            Err("Invalid username or password".to_string())
        }
    }

    pub async fn create_user(&mut self, request: CreateUserRequest) -> Result<User, String> {
        let mut users = self.users.write().map_err(|e| format!("Lock error: {e}"))?;
        
        // Check if user already exists
        if users.values().any(|u| u.username == request.username || u.email == request.email) {
            return Err("User already exists".to_string());
        }

        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();
        
        let user = User {
            id: user_id,
            username: request.username,
            email: request.email,
            password_hash: self.hash_password(&request.password)?,
            role: request.role,
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        users.insert(user_id, user.clone());
        Ok(user)
    }

    pub async fn get_user_by_id(&self, user_id: Uuid) -> Option<User> {
        let users = self.users.read().ok()?;
        users.get(&user_id).cloned()
    }

    fn hash_password(&self, password: &str) -> Result<String, String> {
        // In production, use proper password hashing like bcrypt, argon2, etc.
        // For demo purposes, just use a simple hash
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        password.hash(&mut hasher);
        Ok(format!("hash_{}", hasher.finish()))
    }

    fn verify_password(&self, password: &str, hash: &str) -> Result<bool, String> {
        let expected_hash = self.hash_password(password)?;
        Ok(expected_hash == hash)
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
                permissions: vec!["Admin".to_string()],
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
                permissions: vec!["TaskWrite".to_string()],
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
                permissions: vec!["TaskRead".to_string()],
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
