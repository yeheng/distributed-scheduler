use async_trait::async_trait;
use chrono::Utc;
use scheduler_domain::repositories::{User, UserRepository, CreateUserRequest, UserRole};
use scheduler_errors::{SchedulerError, SchedulerResult};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// PostgreSQL implementation of UserRepository
pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
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
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn create(&self, request: &CreateUserRequest) -> SchedulerResult<User> {
        let password_hash = Self::hash_password(&request.password)?;
        let user_id = Uuid::new_v4();
        let now = Utc::now();

        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (id, username, email, password_hash, role, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, username, email, password_hash, role AS "role: UserRole", is_active, created_at, updated_at
            "#,
            user_id,
            request.username,
            request.email,
            password_hash,
            request.role as UserRole,
            true,
            now,
            now
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.constraint().is_some() => {
                let constraint = db_err.constraint().unwrap_or("");
                if constraint.contains("username") {
                    SchedulerError::validation_error("Username already exists")
                } else if constraint.contains("email") {
                    SchedulerError::validation_error("Email already exists")
                } else {
                    SchedulerError::Database(e)
                }
            }
            _ => SchedulerError::Database(e),
        })?;

        Ok(user)
    }

    async fn get_by_id(&self, user_id: Uuid) -> SchedulerResult<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, password_hash, role AS "role: UserRole", is_active, created_at, updated_at
            FROM users 
            WHERE id = $1
            "#,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        Ok(user)
    }

    async fn get_by_username(&self, username: &str) -> SchedulerResult<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, password_hash, role AS "role: UserRole", is_active, created_at, updated_at
            FROM users 
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        Ok(user)
    }

    async fn get_by_email(&self, email: &str) -> SchedulerResult<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, password_hash, role AS "role: UserRole", is_active, created_at, updated_at
            FROM users 
            WHERE email = $1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(SchedulerError::Database)?;

        Ok(user)
    }

    async fn update(&self, user: &User) -> SchedulerResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET username = $2, email = $3, password_hash = $4, role = $5, is_active = $6, updated_at = $7
            WHERE id = $1
            "#,
            user.id,
            user.username,
            user.email,
            user.password_hash,
            user.role as UserRole,
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
            user_id
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

        let users = sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, password_hash, role AS "role: UserRole", is_active, created_at, updated_at
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

        Ok(users)
    }

    async fn update_password(&self, user_id: Uuid, password_hash: &str) -> SchedulerResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET password_hash = $2, updated_at = $3
            WHERE id = $1
            "#,
            user_id,
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
        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET role = $2, updated_at = $3
            WHERE id = $1
            "#,
            user_id,
            role as UserRole,
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
            user_id,
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
            user_id,
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
    use sqlx::PgPool;

    // Helper function to setup test database
    async fn setup_test_db() -> PgPool {
        // This would typically be a test database
        // For now, we'll skip actual database tests
        unimplemented!("Database tests require test database setup")
    }

    #[tokio::test]
    #[ignore] // Ignored until test database is setup
    async fn test_create_user() {
        let pool = setup_test_db().await;
        let repo = PostgresUserRepository::new(pool);

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
        let repo = PostgresUserRepository::new(pool);

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