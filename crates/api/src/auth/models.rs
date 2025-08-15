use crate::auth::{AuthType, AuthenticatedUser, Permission};
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
