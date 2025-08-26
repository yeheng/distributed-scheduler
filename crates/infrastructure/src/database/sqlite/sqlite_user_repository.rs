use async_trait::async_trait;
use chrono::Utc;
use scheduler_domain::repositories::{User, UserRepository, CreateUserRequest, UserRole};
use scheduler_errors::{SchedulerError, SchedulerResult};
use sqlx::{SqlitePool, Row};
use uuid::Uuid;

/// SQLite implementation of UserRepository
pub struct SqliteUserRepository {
    pool: SqlitePool,
}

impl SqliteUserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Hash password using bcrypt
    fn hash_password(password: &str) -> SchedulerResult<String> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| SchedulerError::Internal(format!("Failed to hash password: {e}")))
    }

    /// Verify password using bcrypt
    fn verify_password(password: &str, hash: &str) -> bool {
        bcrypt::verify(password, hash).unwrap_or(false)
    }

    /// Convert UserRole to string for SQLite storage
    fn role_to_string(role: UserRole) -> &'static str {
        match role {
            UserRole::Admin => "Admin",
            UserRole::Operator => "Operator",
            UserRole::Viewer => "Viewer",
        }
    }

    /// Convert string to UserRole from SQLite storage
    fn string_to_role(role_str: &str) -> SchedulerResult<UserRole> {
        match role_str {
            "Admin" => Ok(UserRole::Admin),
            "Operator" => Ok(UserRole::Operator),
            "Viewer" => Ok(UserRole::Viewer),
            _ => Err(SchedulerError::validation_error("Invalid user role")),
        }
    }
}

#[async_trait]
impl UserRepository for SqliteUserRepository {
    async fn create(&self, request: &CreateUserRequest) -> SchedulerResult<User> {
        let password_hash = Self::hash_password(&request.password)?;
        let user_id = Uuid::new_v4();
        let now = Utc::now();
        let role_str = Self::role_to_string(request.role);

        // Insert user
        let result = sqlx::query!(
            r#"
            INSERT INTO users (id, username, email, password_hash, role, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            user_id.to_string(),
            request.username,
            request.email,
            password_hash,
            role_str,
            true,
            now,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.message().contains("UNIQUE constraint failed") => {
                if db_err.message().contains("username") {
                    SchedulerError::validation_error("Username already exists")
                } else if db_err.message().contains("email") {
                    SchedulerError::validation_error("Email already exists")
                } else {
                    SchedulerError::Database(e)
                }
            }
            _ => SchedulerError::Database(e),
        })?;

        // Return the created user
        Ok(User {
            id: user_id,
            username: request.username.clone(),
            email: request.email.clone(),
            password_hash,
            role: request.role,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, user_id: Uuid) -> SchedulerResult<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT id, username, email, password_hash, role, is_active, created_at, updated_at
            FROM users 
            WHERE id = $1
            "#,
            user_id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match row {
            Some(row) => {
                let role = Self::string_to_role(&row.role)?;
                let id = Uuid::parse_str(&row.id)
                    .map_err(|e| SchedulerError::Internal(format!("Invalid UUID: {e}")))?;

                Ok(Some(User {
                    id,
                    username: row.username,
                    email: row.email,
                    password_hash: row.password_hash,
                    role,
                    is_active: row.is_active,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_username(&self, username: &str) -> SchedulerResult<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT id, username, email, password_hash, role, is_active, created_at, updated_at
            FROM users 
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match row {
            Some(row) => {
                let role = Self::string_to_role(&row.role)?;
                let id = Uuid::parse_str(&row.id)
                    .map_err(|e| SchedulerError::Internal(format!("Invalid UUID: {e}")))?;

                Ok(Some(User {
                    id,
                    username: row.username,
                    email: row.email,
                    password_hash: row.password_hash,
                    role,
                    is_active: row.is_active,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_email(&self, email: &str) -> SchedulerResult<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT id, username, email, password_hash, role, is_active, created_at, updated_at
            FROM users 
            WHERE email = $1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        match row {
            Some(row) => {
                let role = Self::string_to_role(&row.role)?;
                let id = Uuid::parse_str(&row.id)
                    .map_err(|e| SchedulerError::Internal(format!("Invalid UUID: {e}")))?;

                Ok(Some(User {
                    id,
                    username: row.username,
                    email: row.email,
                    password_hash: row.password_hash,
                    role,
                    is_active: row.is_active,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, user: &User) -> SchedulerResult<()> {
        let role_str = Self::role_to_string(user.role);
        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET username = $2, email = $3, password_hash = $4, role = $5, is_active = $6, updated_at = $7
            WHERE id = $1
            "#,
            user.id.to_string(),
            user.username,
            user.email,
            user.password_hash,
            role_str,
            user.is_active,
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::ValidationError("User not found".to_string()));
        }

        Ok(())
    }

    async fn delete(&self, user_id: Uuid) -> SchedulerResult<()> {
        let result = sqlx::query!(
            "DELETE FROM users WHERE id = $1",
            user_id.to_string()
        )
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::ValidationError("User not found".to_string()));
        }

        Ok(())
    }

    async fn list_users(&self, limit: Option<i64>, offset: Option<i64>) -> SchedulerResult<Vec<User>> {
        let limit = limit.unwrap_or(50).min(1000); // Maximum 1000 users per request
        let offset = offset.unwrap_or(0);

        let rows = sqlx::query!(
            r#"
            SELECT id, username, email, password_hash, role, is_active, created_at, updated_at
            FROM users 
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        let mut users = Vec::new();
        for row in rows {
            let role = Self::string_to_role(&row.role)?;
            let id = Uuid::parse_str(&row.id)
                .map_err(|e| SchedulerError::Internal(format!("Invalid UUID: {e}")))?;

            users.push(User {
                id,
                username: row.username,
                email: row.email,
                password_hash: row.password_hash,
                role,
                is_active: row.is_active,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }

        Ok(users)
    }

    async fn update_password(&self, user_id: Uuid, password_hash: &str) -> SchedulerResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET password_hash = $2, updated_at = $3
            WHERE id = $1
            "#,
            user_id.to_string(),
            password_hash,
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::ValidationError("User not found".to_string()));
        }

        Ok(())
    }

    async fn update_role(&self, user_id: Uuid, role: UserRole) -> SchedulerResult<()> {
        let role_str = Self::role_to_string(role);
        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET role = $2, updated_at = $3
            WHERE id = $1
            "#,
            user_id.to_string(),
            role_str,
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::ValidationError("User not found".to_string()));
        }

        Ok(())
    }

    async fn activate_user(&self, user_id: Uuid) -> SchedulerResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET is_active = true, updated_at = $2
            WHERE id = $1
            "#,
            user_id.to_string(),
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::ValidationError("User not found".to_string()));
        }

        Ok(())
    }

    async fn deactivate_user(&self, user_id: Uuid) -> SchedulerResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET is_active = false, updated_at = $2
            WHERE id = $1
            "#,
            user_id.to_string(),
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::ValidationError("User not found".to_string()));
        }

        Ok(())
    }

    async fn authenticate_user(&self, username: &str, password: &str) -> SchedulerResult<Option<User>> {
        // Get user by username
        let user = match self.get_by_username(username).await? {
            Some(user) => user,
            None => return Ok(None),
        };

        // Check if user is active
        if !user.is_active {
            return Ok(None);
        }

        // Verify password
        if Self::verify_password(password, &user.password_hash) {
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    // Helper function to setup test database
    async fn setup_test_db() -> SqlitePool {
        // This would typically be a test database
        // For now, we'll skip actual database tests
        unimplemented!("Database tests require test database setup")
    }

    #[tokio::test]
    #[ignore] // Ignored until test database is setup
    async fn test_create_user() {
        let pool = setup_test_db().await;
        let repo = SqliteUserRepository::new(pool);

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "TestPassword123".to_string(),
            role: UserRole::Operator,
        };

        let result = repo.create(&request).await;
        assert!(result.is_ok());

        let user = result.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.role, UserRole::Operator);
        assert!(user.is_active);
    }

    #[tokio::test]
    #[ignore] // Ignored until test database is setup
    async fn test_authenticate_user() {
        let pool = setup_test_db().await;
        let repo = SqliteUserRepository::new(pool);

        // Create user first
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "TestPassword123".to_string(),
            role: UserRole::Operator,
        };
        repo.create(&request).await.unwrap();

        // Test authentication
        let result = repo.authenticate_user("testuser", "TestPassword123").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        // Test wrong password
        let result = repo.authenticate_user("testuser", "WrongPassword").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}