# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository. 

## Project Overview

This is a distributed task scheduler system built in Rust, designed for high-performance, scalable task scheduling and worker management. The system follows a microservices architecture with message queue-based communication and clean architecture principles. All components are designed to be highly scalable and fault-tolerant, and **MUST** follow DRY, SLIOD, YANGI principles.

## Architecture

The project uses a multi-crate workspace structure with 11 crates following domain-driven design:

### Core Domain & Business Logic

- **`crates/domain/`** - Domain models and business entities (Task, TaskRun, WorkerInfo, Message)
- **`crates/application/`** - Application services and use cases, service interfaces (ports)
- **`crates/errors/`** - Unified error handling with `SchedulerError` enum and `SchedulerResult<T>` type

### Infrastructure & Implementation

- **`crates/infrastructure/`** - Infrastructure implementations (repositories, message queues, factories)
- **`crates/config/`** - Configuration management with TOML support, security, validation
- **`crates/database/`** - Database abstractions and connection management
- **`crates/api/`** - REST API service built with Axum framework for HTTP interfaces

### Services & Applications

- **`crates/dispatcher/`** - Task scheduler core responsible for scheduling strategies, dependency management, and cron jobs
- **`crates/worker/`** - Worker node service for task execution, heartbeat management, and lifecycle control
- **`crates/cli/`** - Command-line interface and utilities
- **`crates/core/`** - Core models and shared abstractions

### Key Design Patterns

- **Clean Architecture**: Domain-driven design with clear separation of concerns
- **Dependency Injection**: Uses `Arc<dyn Trait>` for service abstraction
- **Repository Pattern**: Database operations abstracted through repository interfaces
- **Service Layer**: Business logic encapsulated in service implementations
- **Message Queue Pattern**: Async communication between components using RabbitMQ/Redis Stream
- **Factory Pattern**: Message queue and database connection factories

## Development Commands

### Building and Testing

```bash
# Build entire workspace
cargo build

# Build in release mode
cargo build --release

# Run all tests
cargo test

# Run tests for specific crate
cargo test -p scheduler-domain

# Run integration tests
cargo test --test integration_tests

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### Code Quality

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Run linter
cargo clippy

# Run linter with strict warnings
cargo clippy -- -D warnings

# Check compilation without building
cargo check --all-targets

# Generate documentation
cargo doc --no-deps

# Open documentation
cargo doc --open

# Architecture compliance check
./scripts/architecture_check.sh

# Performance benchmarks
./scripts/run_benchmarks.sh
```

### Database Operations

```bash
# Run database migrations (SQLx-based)
sqlx migrate run

# Create new migration
sqlx migrate add migration_name

# Check database schema
sqlx database reset
```

### Development Services

```bash
# Start development dependencies (PostgreSQL, RabbitMQ, Redis)
docker-compose up -d

# Start combined scheduler service (All modes)
cargo run --bin scheduler

# Start specific services
cargo run --bin api       # API service only
cargo run --bin dispatcher # Dispatcher service only
cargo run --bin worker    # Worker service only

# CLI tools
cargo run --bin cli -- --help
```

### Service Binaries

The project provides 4 main binaries:

- **`scheduler`** - Combined service that can run all modes (API, Dispatcher, Worker)
- **`api`** - Standalone REST API server
- **`dispatcher`** - Task scheduling and dispatch service
- **`worker`** - Task execution worker node
- **`cli`** - Command-line utilities and tools

## Code Standards

### Error Handling

- Use the unified `SchedulerResult<T>` type for all fallible operations
- Use `SchedulerError` enum for comprehensive error types
- Prefer `?` operator over `unwrap()` or `expect()`
- Create meaningful error messages with context
- Implement proper error conversion with `From` traits

```rust
use scheduler_errors::{SchedulerError, SchedulerResult};

async fn operation() -> SchedulerResult<T> {
    some_fallible_operation()
        .await
        .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;
    Ok(result)
}
```

### Async Programming

- All async functions should return `SchedulerResult<T>`
- Use `tokio::join!` for concurrent operations
- Use `Arc<T>` for sharing data between async tasks
- Implement timeout controls for network operations
- Follow async/await patterns consistently

### Service Interfaces

Use `#[async_trait]` for service definitions:

```rust
#[async_trait]
pub trait TaskControlService: Send + Sync {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;
}
```

### Documentation

- All public APIs must have comprehensive documentation comments
- Include usage examples in documentation
- Document error conditions and return values
- Use `//!` for module-level documentation

### Testing

- Write unit tests for all business logic
- Use integration tests for cross-module functionality
- Test both success and error scenarios
- Use descriptive test names: `test_function_scenario_expected_result`

## Configuration

### Configuration Files

The system uses TOML configuration with environment-specific files:

- `config/development.toml` - Development environment
- `config/production.toml` - Production environment
- `config/scheduler.toml` - Default scheduler settings

### Configuration Structure

```rust
use scheduler_config::AppConfig;

// Load configuration
let config = AppConfig::load(Some("config/development.toml"))?;

// Access configuration sections
config.database.url;
config.message_queue.r#type;
config.api.enabled;
```

### Message Queue Support

The system supports multiple message queue backends:

- **RabbitMQ** - Production-ready message queue
- **Redis Stream** - High-performance streaming

Configuration is handled through `MessageQueueFactory`:

```rust
let queue = MessageQueueFactory::create(&config.message_queue).await?;
```

## Database Schema

### Core Entities

- **`tasks`** - Task definitions and metadata
- **`task_runs`** - Task execution instances  
- **`workers`** - Worker node information
- **`task_dependencies`** - Task dependency relationships

### Database Support

- **PostgreSQL** - Primary database for production
- **SQLite** - Development and testing database

Database migrations are managed with SQLx in the `migrations/` directory.

## API Reference

Complete API documentation is available in `docs/API_COMPLETE.md`.

### Key Endpoints

- **Authentication**: `/auth/login`, `/auth/refresh`, `/auth/logout`
- **Task Management**: `/api/tasks/*` - CRUD operations, execution control
- **Worker Management**: `/api/workers/*` - Worker registration, health monitoring
- **System Monitoring**: `/api/system/*` - Health, metrics, configuration
- **Health Checks**: `/health` - Service health status

### Authentication

The API supports multiple authentication methods:

- **API Keys** - For service-to-service communication
- **JWT Tokens** - For user authentication
- **Role-based permissions** - Admin, User, Worker roles

## Performance Features

- Connection pooling for database and message queues
- Batch operations for high throughput
- Streaming for large data processing
- Built-in metrics with Prometheus endpoint (`/metrics`)
- Configurable retry mechanisms with exponential backoff
- Circuit breaker patterns for resilience

## Security Features

- JWT token authentication with refresh tokens
- API key authentication for service accounts
- Role-based access control (RBAC)
- Input validation for all endpoints
- SQL injection prevention through parameterized queries
- Sensitive configuration encryption
- Rate limiting and request throttling

## Monitoring and Observability

- **Structured Logging**: Tracing with JSON/pretty output formats
- **Metrics**: Prometheus endpoint at `/metrics`
- **Health Checks**: Service and dependency health monitoring
- **Distributed Tracing**: OpenTelemetry integration
- **Error Tracking**: Comprehensive error context and reporting

## Development Workflow

1. **Setup**: Clone repository, start Docker services (`docker-compose up -d`)
2. **Code Standards**: Run `cargo fmt` and ensure `cargo clippy` passes
3. **Testing**: Run full test suite with `cargo test`
4. **Architecture**: Validate with `./scripts/architecture_check.sh`
5. **Performance**: Benchmark with `./scripts/run_benchmarks.sh`
6. **Documentation**: Update API docs for changes
7. **Versioning**: Use semantic versioning for releases

## Common Patterns

### Service Factory Pattern

```rust
use scheduler_infrastructure::ServiceFactory;

let factory = ServiceFactory::new(config).await?;
let task_service = factory.create_task_control_service().await?;
```

### Message Queue Usage

```rust
use scheduler_infrastructure::MessageQueueFactory;

let queue = MessageQueueFactory::create(&config.message_queue).await?;
queue.publish_message("tasks", &message).await?;
```

### Configuration Loading with Startup

```rust
use scheduler::common::{StartupConfig, start_application};
use scheduler::app::AppMode;

let startup_config = StartupConfig {
    config_path: "config/development.toml".to_string(),
    log_level: "info".to_string(),
    log_format: "pretty".to_string(),
    worker_id: None,
};

start_application(startup_config, AppMode::All, "Scheduler").await?;
```

This distributed task scheduler emphasizes reliability, scalability, and maintainability through modern Rust practices, clean architecture, and enterprise-grade patterns.