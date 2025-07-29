# Code Examples and Fixes

This document provides specific code examples demonstrating the issues identified in the validation report and their recommended fixes.

## Security Fixes

### 1. Hardcoded Credentials Removal

#### ❌ Current Implementation (Problematic)

**File**: `crates/infrastructure/tests/end_to_end_tests.rs`

```rust
async fn setup_empty_database() -> (testcontainers::ContainerAsync<Postgres>, PgPool) {
    let postgres_image = Postgres::default()
        .with_db_name("scheduler_e2e_test")
        .with_user("test_user")
        .with_password("test_password")  // ❌ Hardcoded password
        .with_tag("16-alpine");
    
    let container = postgres_image.start().await.unwrap();
    let connection_string = format!(
        "postgresql://test_user:test_password@127.0.0.1:{}/scheduler_e2e_test",
        //             ^^^^^^^^^^^^^ ❌ Hardcoded credentials in URL
        container.get_host_port_ipv4(5432).await.unwrap()
    );
}
```

#### ✅ Recommended Fix

**File**: `crates/infrastructure/src/test_config.rs` (New)

```rust
use std::env;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct TestDatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

impl TestDatabaseConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: env::var("TEST_DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("TEST_DB_PORT")
                .unwrap_or_else(|_| "5432".to_string())
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid TEST_DB_PORT"))?,
            database: env::var("TEST_DB_NAME").unwrap_or_else(|_| {
                format!("test_scheduler_{}", uuid::Uuid::new_v4().simple())
            }),
            username: env::var("TEST_DB_USER").unwrap_or_else(|_| "test_user".to_string()),
            password: env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                (0..16).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect()
            }),
        })
    }
    
    pub fn connection_string(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database
        )
    }
}
```

**File**: `crates/infrastructure/tests/end_to_end_tests.rs` (Updated)

```rust
use crate::test_config::TestDatabaseConfig;

async fn setup_empty_database() -> Result<(testcontainers::ContainerAsync<Postgres>, PgPool)> {
    let config = TestDatabaseConfig::from_env()?;
    
    let postgres_image = Postgres::default()
        .with_db_name(&config.database)
        .with_user(&config.username)
        .with_password(&config.password)
        .with_tag("16-alpine");
    
    let container = postgres_image.start().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    
    let connection_string = format!(
        "postgresql://{}:{}@{}:{}/{}",
        config.username, config.password, config.host, port, config.database
    );
    
    let pool = PgPool::connect(&connection_string).await?;
    Ok((container, pool))
}
```

### 2. Input Validation Framework

#### ❌ Current Implementation (Unsafe)

**File**: `crates/worker/src/executors.rs`

```rust
impl TaskExecutor for ShellExecutor {
    async fn execute_task(&self, context: &TaskExecutionContextTrait) -> Result<TaskResult> {
        let shell_params: ShellTaskParams = serde_json::from_value(
            context.parameters.get("shell_params").cloned()
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "command": context.parameters.get("command")
                            .and_then(|v| v.as_str()).unwrap_or("echo"),
                        // ❌ No validation - could be dangerous command
                        "args": context.parameters.get("args")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect::<Vec<_>>()
                            })
                    })
                })
        )?;
        
        // ❌ Direct command execution without validation
        let output = Command::new(&shell_params.command)
            .args(&shell_params.args.unwrap_or_default())
            .output()
            .await?;
    }
}
```

#### ✅ Recommended Fix

**File**: `crates/core/src/validation/command_validator.rs` (New)

```rust
use std::collections::HashSet;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Command not allowed: {0}")]
    CommandNotAllowed(String),
    
    #[error("Dangerous pattern detected in: {0}")]
    DangerousPattern(String),
    
    #[error("Argument too long: max {max}, got {actual}")]
    ArgumentTooLong { max: usize, actual: usize },
    
    #[error("Path traversal attempt detected")]
    PathTraversal,
}

pub struct CommandValidator {
    allowed_commands: HashSet<String>,
    blocked_patterns: Vec<Regex>,
    max_arg_length: usize,
    max_args_count: usize,
}

impl CommandValidator {
    pub fn new() -> Self {
        let mut allowed_commands = HashSet::new();
        allowed_commands.insert("echo".to_string());
        allowed_commands.insert("cat".to_string());
        allowed_commands.insert("ls".to_string());
        allowed_commands.insert("grep".to_string());
        allowed_commands.insert("wc".to_string());
        allowed_commands.insert("sort".to_string());
        
        let blocked_patterns = vec![
            Regex::new(r"[;&|`$()]").unwrap(),           // Shell metacharacters
            Regex::new(r"\.\./").unwrap(),               // Path traversal
            Regex::new(r"rm\s+").unwrap(),               // Remove commands
            Regex::new(r"sudo\s+").unwrap(),             // Privilege escalation
            Regex::new(r"curl\s+").unwrap(),             // Network requests
            Regex::new(r"wget\s+").unwrap(),             // Network requests
            Regex::new(r"/etc/").unwrap(),               // System files
            Regex::new(r"/proc/").unwrap(),              // Process info
        ];
        
        Self {
            allowed_commands,
            blocked_patterns,
            max_arg_length: 256,
            max_args_count: 50,
        }
    }
    
    pub fn validate_command(&self, command: &str, args: &[String]) -> Result<(), SecurityError> {
        // Check command allowlist
        if !self.allowed_commands.contains(command) {
            return Err(SecurityError::CommandNotAllowed(command.to_string()));
        }
        
        // Check argument count
        if args.len() > self.max_args_count {
            return Err(SecurityError::ArgumentTooLong {
                max: self.max_args_count,
                actual: args.len(),
            });
        }
        
        // Validate each argument
        for arg in args {
            // Check argument length
            if arg.len() > self.max_arg_length {
                return Err(SecurityError::ArgumentTooLong {
                    max: self.max_arg_length,
                    actual: arg.len(),
                });
            }
            
            // Check for dangerous patterns
            for pattern in &self.blocked_patterns {
                if pattern.is_match(arg) {
                    return Err(SecurityError::DangerousPattern(arg.clone()));
                }
            }
            
            // Specific path traversal check
            if arg.contains("../") {
                return Err(SecurityError::PathTraversal);
            }
        }
        
        // Check command itself for patterns
        for pattern in &self.blocked_patterns {
            if pattern.is_match(command) {
                return Err(SecurityError::DangerousPattern(command.to_string()));
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ValidatedShellParams {
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
    pub timeout_seconds: Option<u64>,
}

impl ValidatedShellParams {
    pub fn validate(command: String, args: Vec<String>) -> Result<Self, SecurityError> {
        let validator = CommandValidator::new();
        validator.validate_command(&command, &args)?;
        
        // Additional validation for working directory
        if let Some(ref wd) = working_directory {
            if wd.contains("../") || wd.starts_with('/') {
                return Err(SecurityError::PathTraversal);
            }
        }
        
        Ok(Self {
            command,
            args,
            working_directory: None,
            timeout_seconds: Some(300), // Default 5 minutes
        })
    }
}
```

**File**: `crates/worker/src/executors.rs` (Updated)

```rust
use crate::validation::{CommandValidator, ValidatedShellParams, SecurityError};

impl TaskExecutor for ShellExecutor {
    async fn execute_task(&self, context: &TaskExecutionContextTrait) -> Result<TaskResult> {
        // Extract and validate parameters
        let command = context.parameters
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SchedulerError::InvalidTaskParams("Missing command".to_string()))?
            .to_string();
            
        let args = context.parameters
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        
        // ✅ Validate command and arguments
        let validated_params = ValidatedShellParams::validate(command, args)
            .map_err(|e| SchedulerError::SecurityError(e.to_string()))?;
        
        // ✅ Execute with timeout and proper error handling
        let output = tokio::time::timeout(
            Duration::from_secs(validated_params.timeout_seconds.unwrap_or(300)),
            Command::new(&validated_params.command)
                .args(&validated_params.args)
                .output()
        )
        .await
        .map_err(|_| SchedulerError::ExecutionTimeout)?
        .map_err(|e| SchedulerError::ExecutionFailed(e.to_string()))?;
        
        Ok(TaskResult {
            success: output.status.success(),
            exit_code: output.status.code(),
            output: if !output.stdout.is_empty() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            },
            error_message: if !output.stderr.is_empty() {
                Some(String::from_utf8_lossy(&output.stderr).trim().to_string())
            } else {
                None
            },
            execution_time: start_time.elapsed(),
        })
    }
}
```

## Performance Optimizations

### 3. Database Connection with Exponential Backoff

#### ❌ Current Implementation (Inefficient)

**File**: `crates/infrastructure/tests/database_test_utils.rs`

```rust
// ❌ Naive retry loop with fixed delay
let mut retry_count = 0;
let pool = loop {
    match PgPool::connect(&database_url).await {
        Ok(pool) => break pool,
        Err(_) if retry_count < 30 => {
            retry_count += 1;
            sleep(Duration::from_millis(500)).await;  // ❌ Fixed 500ms delay
            continue;
        }
        Err(e) => return Err(e.into()),
    }
};
```

#### ✅ Recommended Fix

**File**: `crates/infrastructure/src/database/connection.rs` (New)

```rust
use tokio::time::{sleep, Duration, Instant};
use sqlx::PgPool;
use rand::Rng;
use tracing::{info, warn, error};

#[derive(Debug, Clone)]
pub struct ConnectionRetryConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub max_retries: u32,
    pub backoff_multiplier: f64,
    pub jitter_factor: f64,
}

impl Default for ConnectionRetryConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            max_retries: 10,
            backoff_multiplier: 1.5,
            jitter_factor: 0.25,
        }
    }
}

pub async fn connect_with_backoff(
    url: &str,
    config: ConnectionRetryConfig,
) -> Result<PgPool, anyhow::Error> {
    let start_time = Instant::now();
    let mut current_delay = config.initial_delay;
    let mut rng = rand::thread_rng();
    
    for attempt in 0..config.max_retries {
        match PgPool::connect(url).await {
            Ok(pool) => {
                let total_time = start_time.elapsed();
                info!(
                    "Database connected successfully after {} attempts in {:?}",
                    attempt + 1,
                    total_time
                );
                return Ok(pool);
            }
            Err(e) if attempt == config.max_retries - 1 => {
                let total_time = start_time.elapsed();
                error!(
                    "Failed to connect to database after {} attempts in {:?}: {}",
                    config.max_retries,
                    total_time,
                    e
                );
                return Err(e.into());
            }
            Err(e) => {
                // Calculate jitter: ±25% of current delay
                let jitter_range = (current_delay.as_millis() as f64 * config.jitter_factor) as u64;
                let jitter = if jitter_range > 0 {
                    rng.gen_range(0..=jitter_range * 2) - jitter_range
                } else {
                    0
                };
                
                let delay_with_jitter = Duration::from_millis(
                    (current_delay.as_millis() as i64 + jitter as i64).max(0) as u64
                );
                
                warn!(
                    "Connection attempt {} failed: {}, retrying in {:?}",
                    attempt + 1,
                    e,
                    delay_with_jitter
                );
                
                sleep(delay_with_jitter).await;
                
                // Exponential backoff with maximum cap
                current_delay = std::cmp::min(
                    Duration::from_millis(
                        (current_delay.as_millis() as f64 * config.backoff_multiplier) as u64
                    ),
                    config.max_delay,
                );
            }
        }
    }
    
    unreachable!("Loop should have returned or failed")
}

// Optimized connection pool configuration
pub fn create_optimized_pool_options() -> sqlx::postgres::PgPoolOptions {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)                          // Reasonable for tests
        .min_connections(2)                           // Keep some warm connections
        .acquire_timeout(Duration::from_secs(5))      // Don't wait too long
        .idle_timeout(Duration::from_secs(30))        // Close idle connections
        .max_lifetime(Duration::from_secs(300))       // Rotate connections
        .test_before_acquire(true)                    // Ensure connection health
}
```

**File**: `crates/infrastructure/tests/database_test_utils.rs` (Updated)

```rust
use crate::database::connection::{connect_with_backoff, create_optimized_pool_options, ConnectionRetryConfig};

impl DatabaseTestContainer {
    pub async fn new() -> Result<Self> {
        let config = TestDatabaseConfig::from_env()?;
        
        let postgres_image = Postgres::default()
            .with_db_name(&config.database)
            .with_user(&config.username)
            .with_password(&config.password)
            .with_tag("16-alpine");

        let container = postgres_image.start().await?;
        let port = container.get_host_port_ipv4(5432).await?;

        let database_url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            config.username, config.password, config.host, port, config.database
        );

        // ✅ Use optimized connection with backoff
        let pool = create_optimized_pool_options()
            .connect_with(|url| connect_with_backoff(url, ConnectionRetryConfig::default()))
            .connect(&database_url)
            .await?;

        Ok(Self { container, pool })
    }
}
```

### 4. Shared Test Infrastructure

#### ❌ Current Implementation (Resource Intensive)

```rust
// ❌ Each test creates its own database container
#[tokio::test]
async fn test_task_repository_basic_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;  // New container each time
    container.run_migrations().await?;
    // ... test logic
}

#[tokio::test] 
async fn test_task_run_repository_basic_operations() -> Result<()> {
    let container = DatabaseTestContainer::new().await?;  // Another new container
    container.run_migrations().await?;
    // ... test logic
}
```

#### ✅ Recommended Fix

**File**: `crates/infrastructure/tests/shared_test_infrastructure.rs` (New)

```rust
use std::sync::Arc;
use tokio::sync::{OnceCell, Mutex};
use sqlx::{PgPool, Transaction, Postgres};

// Global shared test database
static SHARED_TEST_DB: OnceCell<Arc<SharedTestDatabase>> = OnceCell::const_new();

pub struct SharedTestDatabase {
    pub container: testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
    pub pool: PgPool,
    connection_mutex: Mutex<()>, // Ensure migrations run only once
}

impl SharedTestDatabase {
    async fn new() -> Result<Self> {
        let config = TestDatabaseConfig::from_env()?;
        
        let postgres_image = testcontainers_modules::postgres::Postgres::default()
            .with_db_name(&config.database)
            .with_user(&config.username)
            .with_password(&config.password)
            .with_tag("16-alpine");

        let container = postgres_image.start().await?;
        let port = container.get_host_port_ipv4(5432).await?;

        let database_url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            config.username, config.password, config.host, port, config.database
        );

        let pool = create_optimized_pool_options()
            .connect(&database_url)
            .await?;

        // Run migrations once
        sqlx::migrate!("../../migrations").run(&pool).await?;

        Ok(Self {
            container,
            pool,
            connection_mutex: Mutex::new(()),
        })
    }
}

// Get or create shared test database
pub async fn get_shared_test_db() -> Arc<SharedTestDatabase> {
    SHARED_TEST_DB
        .get_or_init(|| async {
            Arc::new(
                SharedTestDatabase::new()
                    .await
                    .expect("Failed to create shared test database")
            )
        })
        .await
        .clone()
}

// Test isolation through transactions
pub struct IsolatedTest<'a> {
    pub tx: Transaction<'a, Postgres>,
}

impl<'a> IsolatedTest<'a> {
    pub async fn new(pool: &'a PgPool) -> Result<Self> {
        let tx = pool.begin().await?;
        Ok(Self { tx })
    }
    
    // Automatic rollback on drop
    pub async fn finish(self) -> Result<()> {
        self.tx.rollback().await?;
        Ok(())
    }
}
```

**File**: Updated test files

```rust
use crate::shared_test_infrastructure::{get_shared_test_db, IsolatedTest};

#[tokio::test]
async fn test_task_repository_basic_operations() -> Result<()> {
    let shared_db = get_shared_test_db().await;
    let isolated_test = IsolatedTest::new(&shared_db.pool).await?;
    
    // Use isolated_test.tx for all database operations
    let repo = PostgresTaskRepository::new_with_transaction(&isolated_test.tx);
    
    let task = Task::new(
        "test_task".to_string(),
        "shell".to_string(),
        "0 0 * * *".to_string(),
        serde_json::json!({"command": "echo hello"}),
    );
    
    let created_task = repo.create(&task).await?;
    assert_eq!(created_task.name, task.name);
    
    // Test automatically rolls back when isolated_test is dropped
    isolated_test.finish().await?;
    Ok(())
}

#[tokio::test]
async fn test_task_run_repository_basic_operations() -> Result<()> {
    let shared_db = get_shared_test_db().await;
    let isolated_test = IsolatedTest::new(&shared_db.pool).await?;
    
    // This test runs in parallel with others but is isolated
    // ... test logic using isolated_test.tx
    
    isolated_test.finish().await?;
    Ok(())
}
```

## Code Quality Improvements

### 5. Constants and Magic Number Elimination

#### ❌ Current Implementation (Magic Numbers)

**File**: `crates/worker/src/executors_test.rs`

```rust
fn create_test_task_run_for_type(params: serde_json::Value, task_type: &str) -> TaskRun {
    TaskRun {
        id: 1,           // ❌ Magic number
        task_id: 1,      // ❌ Magic number
        retry_count: 0,  // ❌ Magic number
        // ... more magic numbers
    }
}
```

#### ✅ Recommended Fix

**File**: `crates/core/src/constants/test_constants.rs` (New)

```rust
//! Test constants used across the test suite
//! 
//! This module provides consistent test data values to avoid magic numbers
//! and improve test maintainability.

/// Default test identifiers
pub mod ids {
    pub const TEST_TASK_ID: i64 = 1;
    pub const TEST_WORKER_ID: &str = "test-worker";
    pub const TEST_TASK_RUN_ID: i64 = 1001;
    pub const TEST_DISPATCHER_ID: &str = "test-dispatcher";
}

/// Default test timing values
pub mod timing {
    use std::time::Duration;
    use chrono::{DateTime, Utc};
    
    pub const DEFAULT_TIMEOUT_SECONDS: i32 = 300;
    pub const DEFAULT_RETRY_COUNT: i32 = 0;
    pub const MAX_RETRY_COUNT: i32 = 3;
    pub const HEARTBEAT_INTERVAL_SECONDS: u64 = 30;
    
    pub fn test_timestamp() -> DateTime<Utc> {
        DateTime::from_timestamp(1640995200, 0).unwrap() // 2022-01-01 00:00:00 UTC
    }
}

/// Default test task configurations
pub mod tasks {
    use serde_json::json;
    
    pub const DEFAULT_TASK_NAME: &str = "test_task";
    pub const DEFAULT_TASK_TYPE: &str = "shell";
    pub const DEFAULT_CRON_SCHEDULE: &str = "0 0 * * *";
    
    pub fn default_shell_params() -> serde_json::Value {
        json!({
            "command": "echo",
            "args": ["Hello, World!"]
        })
    }
    
    pub fn default_http_params() -> serde_json::Value {
        json!({
            "url": "https://httpbin.org/get",
            "method": "GET"
        })
    }
}

/// Test database configuration
pub mod database {
    pub const DEFAULT_CONNECTION_TIMEOUT_SECONDS: u64 = 30;
    pub const DEFAULT_MAX_CONNECTIONS: u32 = 5;
    pub const DEFAULT_MIN_CONNECTIONS: u32 = 1;
}

/// Test worker configurations
pub mod workers {
    pub const DEFAULT_MAX_CONCURRENT_TASKS: i32 = 5;
    pub const DEFAULT_SUPPORTED_TASK_TYPES: &[&str] = &["shell", "http"];
    pub const DEFAULT_HOSTNAME: &str = "test-host";
    pub const DEFAULT_IP_ADDRESS: &str = "127.0.0.1";
}
```

**File**: `crates/worker/src/executors_test.rs` (Updated)

```rust
use scheduler_core::constants::test_constants::{ids, timing, tasks};

fn create_test_task_run_for_type(params: serde_json::Value, task_type: &str) -> TaskRun {
    let task_info = serde_json::json!({
        "parameters": params,
        "task_type": task_type
    });
    
    TaskRun {
        id: ids::TEST_TASK_RUN_ID,
        task_id: ids::TEST_TASK_ID,
        status: TaskRunStatus::Running,
        worker_id: Some(ids::TEST_WORKER_ID.to_string()),
        retry_count: timing::DEFAULT_RETRY_COUNT,
        shard_index: None,
        shard_total: None,
        scheduled_at: timing::test_timestamp(),
        started_at: Some(timing::test_timestamp()),
        completed_at: None,
        result: Some(task_info.to_string()),
        error_message: None,
        created_at: timing::test_timestamp(),
    }
}

#[tokio::test]
async fn test_shell_executor_echo() {
    let executor = ShellExecutor::new();
    let params = tasks::default_shell_params();
    let task_run = create_test_task_run_for_type(params, tasks::DEFAULT_TASK_TYPE);
    
    let result = executor.execute(&task_run).await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.output, Some("Hello, World!".to_string()));
    assert!(result.error_message.is_none());
    assert_eq!(result.exit_code, Some(0));
}
```

### 6. Error Handling Improvements

#### ❌ Current Implementation (Brittle)

```rust
// ❌ Tight coupling to specific error types
assert!(matches!(
    delete_result,
    Err(scheduler_core::SchedulerError::TaskNotFound { id: 999 })
));
```

#### ✅ Recommended Fix

**File**: `crates/core/src/testing/error_matchers.rs` (New)

```rust
//! Error matching utilities for tests
//! 
//! Provides flexible error matching that's less brittle than exact matches

use crate::SchedulerError;

pub trait ErrorMatcher {
    fn matches_error(&self, error: &SchedulerError) -> bool;
    fn error_description(&self) -> String;
}

pub struct NotFoundMatcher {
    pub resource_type: String,
    pub expected_id: Option<i64>,
}

impl ErrorMatcher for NotFoundMatcher {
    fn matches_error(&self, error: &SchedulerError) -> bool {
        match error {
            SchedulerError::TaskNotFound { id } => {
                self.resource_type == "task" && 
                (self.expected_id.is_none() || self.expected_id == Some(*id))
            }
            SchedulerError::TaskRunNotFound { id } => {
                self.resource_type == "task_run" &&
                (self.expected_id.is_none() || self.expected_id == Some(*id))
            }
            SchedulerError::WorkerNotFound { id } => {
                self.resource_type == "worker" && 
                self.expected_id.is_none() // Workers use string IDs
            }
            _ => false,
        }
    }
    
    fn error_description(&self) -> String {
        format!("{} not found", self.resource_type)
    }
}

pub struct ValidationMatcher {
    pub expected_field: Option<String>,
}

impl ErrorMatcher for ValidationMatcher {
    fn matches_error(&self, error: &SchedulerError) -> bool {
        match error {
            SchedulerError::InvalidTaskParams(msg) => {
                self.expected_field.as_ref()
                    .map(|field| msg.contains(field))
                    .unwrap_or(true)
            }
            SchedulerError::Configuration(msg) => {
                self.expected_field.as_ref()
                    .map(|field| msg.contains(field))
                    .unwrap_or(true)
            }
            _ => false,
        }
    }
    
    fn error_description(&self) -> String {
        format!("validation error{}", 
            self.expected_field.as_ref()
                .map(|f| format!(" for field: {}", f))
                .unwrap_or_default()
        )
    }
}

// Convenience functions for common error patterns
pub fn expect_not_found(resource_type: &str) -> NotFoundMatcher {
    NotFoundMatcher {
        resource_type: resource_type.to_string(),
        expected_id: None,
    }
}

pub fn expect_not_found_with_id(resource_type: &str, id: i64) -> NotFoundMatcher {
    NotFoundMatcher {
        resource_type: resource_type.to_string(),
        expected_id: Some(id),
    }
}

pub fn expect_validation_error() -> ValidationMatcher {
    ValidationMatcher {
        expected_field: None,
    }
}

pub fn expect_validation_error_for_field(field: &str) -> ValidationMatcher {
    ValidationMatcher {
        expected_field: Some(field.to_string()),
    }
}

// Macro for cleaner test assertions
#[macro_export]
macro_rules! assert_error_matches {
    ($result:expr, $matcher:expr) => {
        match $result {
            Ok(_) => panic!("Expected error, but got Ok result"),
            Err(ref e) => {
                if !$matcher.matches_error(e) {
                    panic!(
                        "Error doesn't match expected pattern.\nExpected: {}\nActual: {:?}",
                        $matcher.error_description(),
                        e
                    );
                }
            }
        }
    };
}
```

**File**: Updated test files

```rust
use crate::testing::error_matchers::{expect_not_found_with_id, expect_validation_error_for_field};
use crate::assert_error_matches;

#[tokio::test]
async fn test_repository_error_handling() -> Result<()> {
    let shared_db = get_shared_test_db().await;
    let isolated_test = IsolatedTest::new(&shared_db.pool).await?;
    
    let task_repo = PostgresTaskRepository::new_with_transaction(&isolated_test.tx);
    let task_run_repo = PostgresTaskRunRepository::new_with_transaction(&isolated_test.tx);

    // ✅ Flexible error matching - less brittle
    let delete_result = task_repo.delete(999).await;
    assert_error_matches!(delete_result, expect_not_found_with_id("task", 999));

    let delete_result = task_run_repo.delete(999).await;
    assert_error_matches!(delete_result, expect_not_found_with_id("task_run", 999));

    isolated_test.finish().await?;
    Ok(())
}
```

---

*These code examples demonstrate the specific fixes needed to address security, performance, and quality issues identified in the validation report.*