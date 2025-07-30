# Unified Database Layer

A clean, SOLID-principles-based database abstraction layer that supports both PostgreSQL and SQLite with automatic switching based on URL.

## Features

- **URL-Based Database Detection**: Automatically detects and switches between PostgreSQL and SQLite based on connection URL
- **Zero Configuration Switching**: Change database backend by just changing the URL - no code changes required
- **SOLID Principles Compliance**:
  - **S**ingle Responsibility: Each class has one responsibility
  - **O**pen/Closed: Extensible for new database types without modification
  - **L**iskov Substitution: Database implementations are interchangeable
  - **I**nterface Segregation: Clean, focused interfaces
  - **D**ependency Inversion: Depends on abstractions, not concrete implementations
- **KISS Principle**: Simple, clean API
- **YAGNI Principle**: Only implements what's needed

## Usage

### Basic Usage

```rust
use scheduler_infrastructure::database::{DatabaseManager, ApplicationService};

// PostgreSQL
let db_manager = DatabaseManager::new("postgres://user:pass@localhost/db", 10).await?;

// SQLite
let db_manager = DatabaseManager::new("sqlite:database.db", 10).await?;

// In-memory SQLite
let db_manager = DatabaseManager::new("sqlite::memory:", 10).await?;

// Get repositories (same code works for both databases)
let task_repo = db_manager.task_repository();
let worker_repo = db_manager.worker_repository();
let task_run_repo = db_manager.task_run_repository();
```

### Application Service Pattern

```rust
use scheduler_infrastructure::database::ApplicationService;

// Create application service with automatic database detection
let app = ApplicationService::new("sqlite::memory:", 10).await?;

// Check what database backend is being used
println!("Using database: {:?}", app.database_info());

// Use repositories - same code regardless of database backend
let tasks = app.tasks();
let workers = app.workers();
let task_runs = app.task_runs();

// Health check
app.health_check().await?;

// Clean shutdown
app.close().await;
```

### Database Switching

The same application code works with different database backends:

```rust
// Development with SQLite
let app = ApplicationService::new("sqlite:dev.db", 5).await?;

// Production with PostgreSQL
let app = ApplicationService::new("postgres://user:pass@prod-db:5432/scheduler", 20).await?;

// Testing with in-memory SQLite
let app = ApplicationService::new("sqlite::memory:", 1).await?;
```

## Architecture

### Database Manager (`DatabaseManager`)

Central coordinator that:
- Detects database type from URL
- Manages connection pools
- Provides repository factories
- Handles health checks and cleanup

### Database Pool (`DatabasePool`)

Enum-based pool wrapper that:
- Abstracts over PostgreSQL and SQLite pools
- Provides unified interface for connection management
- Handles database-specific optimizations

### Repository Pattern

Each repository (Task, TaskRun, Worker) has:
- A trait interface defining operations
- PostgreSQL implementation
- SQLite implementation
- Automatic selection via DatabaseManager

## SOLID Principles Implementation

### Single Responsibility Principle (SRP)
- `DatabaseManager`: Manages database connections and repository creation
- `DatabasePool`: Handles connection pooling
- `DatabaseType`: URL detection and type representation
- Each repository: Handles one entity type's data operations

### Open/Closed Principle (OCP)
- Extensible for new database types (MySQL, etc.) without modifying existing code
- New repositories can be added without changing the manager
- Database-specific optimizations can be added per implementation

### Liskov Substitution Principle (LSP)
- PostgreSQL and SQLite repositories are completely interchangeable
- Same interface, same behavior contracts
- Application code doesn't need to know which database is being used

### Interface Segregation Principle (ISP)
- Clean, focused repository traits
- Database manager provides only needed methods
- No forced dependencies on unused functionality

### Dependency Inversion Principle (DIP)
- Application depends on repository traits, not concrete implementations
- Database manager returns trait objects
- Easy to mock for testing

## Testing

The layer includes comprehensive tests:

```bash
# Test database manager
cargo test database::manager::tests

# Test application service
cargo test database::example::tests

# Test all database functionality
cargo test database --package scheduler-infrastructure
```

## Database Support

### PostgreSQL
- Connection string: `postgres://` or `postgresql://`
- Full feature support
- Production-ready with connection pooling

### SQLite
- File database: `sqlite:path/to/database.db`
- In-memory: `sqlite::memory:`
- Perfect for development and testing
- Supports all the same operations as PostgreSQL

## Migration from Old Code

Old code:
```rust
// Tightly coupled to PostgreSQL
let pool = PgPool::connect(&config.database_url).await?;
let task_repo = PostgresTaskRepository::new(pool.clone());
```

New code:
```rust
// Database-agnostic
let db_manager = DatabaseManager::new(&config.database_url, config.max_connections).await?;
let task_repo = db_manager.task_repository();
```

## Configuration

No configuration files needed! Just change your database URL:

```bash
# Development
DATABASE_URL=sqlite:dev.db

# Testing
DATABASE_URL=sqlite::memory:

# Production
DATABASE_URL=postgres://user:pass@localhost:5432/scheduler
```

The application automatically adapts to the database backend.