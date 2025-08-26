use scheduler_domain::repositories::{User, UserRepository, CreateUserRequest, UserRole};
use scheduler_errors::{SchedulerError, SchedulerResult};
use std::sync::Arc;
use uuid::Uuid;

/// Authentication service for user management and authentication
/// This service orchestrates authentication-related business logic
pub struct AuthenticationService {
    user_repository: Arc<dyn UserRepository>,
}

impl AuthenticationService {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }

    /// Create a new user account
    pub async fn create_user(&self, request: CreateUserRequest) -> SchedulerResult<User> {
        // Check if user already exists
        if let Some(_) = self.user_repository.get_by_username(&request.username).await? {
            return Err(SchedulerError::validation_error(
                "Username already exists",
            ));
        }

        if let Some(_) = self.user_repository.get_by_email(&request.email).await? {
            return Err(SchedulerError::validation_error(
                "Email already exists",
            ));
        }

        // Validate password strength
        self.validate_password(&request.password)?;

        // Create user through repository
        self.user_repository.create(&request).await
    }

    /// Authenticate user with username and password
    pub async fn authenticate_user(&self, username: &str, password: &str) -> SchedulerResult<Option<User>> {
        // Validate input
        if username.trim().is_empty() {
            return Err(SchedulerError::validation_error("Username cannot be empty"));
        }

        if password.is_empty() {
            return Err(SchedulerError::validation_error("Password cannot be empty"));
        }

        // Delegate to repository for authentication
        self.user_repository.authenticate_user(username, password).await
    }

    /// Get user by ID
    pub async fn get_user_by_id(&self, user_id: Uuid) -> SchedulerResult<Option<User>> {
        self.user_repository.get_by_id(user_id).await
    }

    /// Get user by username
    pub async fn get_user_by_username(&self, username: &str) -> SchedulerResult<Option<User>> {
        self.user_repository.get_by_username(username).await
    }

    /// Update user password
    pub async fn change_password(&self, user_id: Uuid, old_password: &str, new_password: &str) -> SchedulerResult<()> {
        // Get user
        let user = self.user_repository.get_by_id(user_id).await?
            .ok_or_else(|| SchedulerError::ValidationError("User not found".to_string()))?;

        // Verify old password through authentication
        let auth_result = self.user_repository.authenticate_user(&user.username, old_password).await?;
        if auth_result.is_none() {
            return Err(SchedulerError::Permission("Invalid current password".to_string()));
        }

        // Validate new password
        self.validate_password(new_password)?;

        // Hash new password and update
        let password_hash = self.hash_password(new_password)?;
        self.user_repository.update_password(user_id, &password_hash).await
    }

    /// Update user role (admin only)
    pub async fn update_user_role(&self, user_id: Uuid, new_role: UserRole) -> SchedulerResult<()> {
        // Check if user exists
        let _user = self.user_repository.get_by_id(user_id).await?
            .ok_or_else(|| SchedulerError::ValidationError("User not found".to_string()))?;

        self.user_repository.update_role(user_id, new_role).await
    }

    /// Activate user account
    pub async fn activate_user(&self, user_id: Uuid) -> SchedulerResult<()> {
        self.user_repository.activate_user(user_id).await
    }

    /// Deactivate user account
    pub async fn deactivate_user(&self, user_id: Uuid) -> SchedulerResult<()> {
        self.user_repository.deactivate_user(user_id).await
    }

    /// List users with pagination
    pub async fn list_users(&self, limit: Option<i64>, offset: Option<i64>) -> SchedulerResult<Vec<User>> {
        self.user_repository.list_users(limit, offset).await
    }

    /// Delete user account
    pub async fn delete_user(&self, user_id: Uuid) -> SchedulerResult<()> {
        // Check if user exists
        let _user = self.user_repository.get_by_id(user_id).await?
            .ok_or_else(|| SchedulerError::ValidationError("User not found".to_string()))?;

        self.user_repository.delete(user_id).await
    }

    /// Validate password strength
    fn validate_password(&self, password: &str) -> SchedulerResult<()> {
        if password.len() < 8 {
            return Err(SchedulerError::validation_error(
                "Password must be at least 8 characters long",
            ));
        }

        if !password.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(SchedulerError::validation_error(
                "Password must contain at least one uppercase letter",
            ));
        }

        if !password.chars().any(|c| c.is_ascii_lowercase()) {
            return Err(SchedulerError::validation_error(
                "Password must contain at least one lowercase letter",
            ));
        }

        if !password.chars().any(|c| c.is_ascii_digit()) {
            return Err(SchedulerError::validation_error(
                "Password must contain at least one digit",
            ));
        }

        Ok(())
    }

    /// Hash password using bcrypt
    fn hash_password(&self, password: &str) -> SchedulerResult<String> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| SchedulerError::Internal(format!("Failed to hash password: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use tokio::sync::RwLock;
    use chrono::Utc;

    // Mock UserRepository for testing
    struct MockUserRepository {
        users: Arc<RwLock<HashMap<Uuid, User>>>,
        username_index: Arc<RwLock<HashMap<String, Uuid>>>,
        email_index: Arc<RwLock<HashMap<String, Uuid>>>,
    }

    impl MockUserRepository {
        fn new() -> Self {
            Self {
                users: Arc::new(RwLock::new(HashMap::new())),
                username_index: Arc::new(RwLock::new(HashMap::new())),
                email_index: Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create(&self, request: &CreateUserRequest) -> SchedulerResult<User> {
            let user_id = Uuid::new_v4();
            let password_hash = bcrypt::hash(&request.password, bcrypt::DEFAULT_COST)
                .map_err(|e| SchedulerError::Internal(format!("Hash error: {e}")))?;

            let now = Utc::now();
            let user = User {
                id: user_id,
                username: request.username.clone(),
                email: request.email.clone(),
                password_hash,
                role: request.role,
                is_active: true,
                created_at: now,
                updated_at: now,
            };

            let mut users = self.users.write().await;
            let mut username_index = self.username_index.write().await;
            let mut email_index = self.email_index.write().await;

            users.insert(user_id, user.clone());
            username_index.insert(request.username.clone(), user_id);
            email_index.insert(request.email.clone(), user_id);

            Ok(user)
        }

        async fn get_by_id(&self, user_id: Uuid) -> SchedulerResult<Option<User>> {
            let users = self.users.read().await;
            Ok(users.get(&user_id).cloned())
        }

        async fn get_by_username(&self, username: &str) -> SchedulerResult<Option<User>> {
            let username_index = self.username_index.read().await;
            let users = self.users.read().await;
            
            if let Some(user_id) = username_index.get(username) {
                Ok(users.get(user_id).cloned())
            } else {
                Ok(None)
            }
        }

        async fn get_by_email(&self, email: &str) -> SchedulerResult<Option<User>> {
            let email_index = self.email_index.read().await;
            let users = self.users.read().await;
            
            if let Some(user_id) = email_index.get(email) {
                Ok(users.get(user_id).cloned())
            } else {
                Ok(None)
            }
        }

        async fn authenticate_user(&self, username: &str, password: &str) -> SchedulerResult<Option<User>> {
            if let Some(user) = self.get_by_username(username).await? {
                if user.is_active && bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
                    Ok(Some(user))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }

        // Implement other required methods with minimal functionality for testing
        async fn update(&self, _user: &User) -> SchedulerResult<()> { Ok(()) }
        async fn delete(&self, _user_id: Uuid) -> SchedulerResult<()> { Ok(()) }
        async fn list_users(&self, _limit: Option<i64>, _offset: Option<i64>) -> SchedulerResult<Vec<User>> { Ok(vec![]) }
        async fn update_password(&self, _user_id: Uuid, _password_hash: &str) -> SchedulerResult<()> { Ok(()) }
        async fn update_role(&self, _user_id: Uuid, _role: UserRole) -> SchedulerResult<()> { Ok(()) }
        async fn activate_user(&self, _user_id: Uuid) -> SchedulerResult<()> { Ok(()) }
        async fn deactivate_user(&self, _user_id: Uuid) -> SchedulerResult<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_create_user_success() {
        let repo = Arc::new(MockUserRepository::new());
        let service = AuthenticationService::new(repo);

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "TestPassword123".to_string(),
            role: UserRole::Operator,
        };

        let result = service.create_user(request).await;
        assert!(result.is_ok());

        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.role, UserRole::Operator);
        assert!(user.is_active);
    }

    #[tokio::test]
    async fn test_authenticate_user_success() {
        let repo = Arc::new(MockUserRepository::new());
        let service = AuthenticationService::new(repo.clone());

        // Create a user first
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "TestPassword123".to_string(),
            role: UserRole::Operator,
        };
        service.create_user(request).await.unwrap();

        // Test authentication
        let result = service.authenticate_user("testuser", "TestPassword123").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_validate_password() {
        let repo = Arc::new(MockUserRepository::new());
        let service = AuthenticationService::new(repo);

        // Test weak password
        assert!(service.validate_password("weak").is_err());
        assert!(service.validate_password("nouppercasenumber").is_err());
        assert!(service.validate_password("NOLOWERCASENUMBER").is_err());
        assert!(service.validate_password("NoNumber").is_err());

        // Test strong password
        assert!(service.validate_password("StrongPassword123").is_ok());
    }
}