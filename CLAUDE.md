# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.
please follow these [guidelines](./rules.md) strictly when working with the codebase.

## Project Overview

This is a distributed task scheduler system built in Rust, designed for high-performance, scalable task scheduling and worker management. The system follows a microservices architecture with message queue-based communication.

## Architecture

The project uses a multi-crate workspace structure with the following key modules:

### Core Modules

- **`crates/foundation/`** - Foundation module providing core abstractions, service interfaces, dependency injection, and essential building blocks for the scheduler ecosystem
- **`crates/api/`** - REST API service built with Axum framework for HTTP interfaces
- **`crates/dispatcher/`** - Task scheduler core responsible for scheduling strategies, dependency management, and cron jobs
- **`crates/worker/`** - Worker node service for task execution, heartbeat management, and lifecycle control
- **`crates/infrastructure/`** - Infrastructure abstraction layer providing database and message queue interfaces
- **`crates/domain/`** - Domain models and business logic

### Key Design Patterns

- **Dependency Injection**: Uses `Arc<dyn Trait>` for service abstraction
- **Repository Pattern**: Database operations abstracted through repository interfaces
- **Service Layer**: Business logic encapsulated in service implementations
- **Message Queue Pattern**: Async communication between components

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
cargo test -p scheduler-foundation

# Run integration tests
cargo test --test integration_tests

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
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
```

### Database Operations

```bash
# Run database migrations
cargo run --bin migrate

# Create new migration
sqlx migrate add migration_name
```

### Development Services

```bash
# Start development dependencies (PostgreSQL, RabbitMQ, Redis)
docker-compose up -d

# Start scheduler service
cargo run --bin scheduler

# Start worker service
cargo run --bin worker

# Start API service
cargo run --bin api
```

## Code Standards

### Error Handling

- Use the unified `SchedulerResult<T>` type for all fallible operations
- Prefer `?` operator over `unwrap()` or `expect()`
- Create meaningful error messages with context
- Use `SchedulerError` enum for all error types

### Async Programming

- All async functions should return `SchedulerResult<T>`
- Use `tokio::join!` for concurrent operations
- Use `Arc<T>` for sharing data between async tasks
- Implement timeout controls for network operations

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

The system uses TOML configuration files:

- `config/development.toml` - Development environment
- `config/production.toml` - Production environment
- `config/scheduler.toml` - Scheduler-specific settings
- `config/rabbitmq.toml` - RabbitMQ configuration
- `config/redis-stream.toml` - Redis Stream configuration

## Database Schema

Key tables:
- `tasks` - Task definitions and metadata
- `task_runs` - Task execution instances
- `workers` - Worker node information

Database migrations are located in `migrations/` directory.

## API Endpoints

The REST API provides:
- Task management: `/api/tasks`
- Worker management: `/api/workers`
- System monitoring: `/api/system`
- Health checks: `/health`

## Performance Considerations

- Use connection pools for database and message queue connections
- Implement batch operations where possible
- Use streaming for large data processing
- Monitor resource usage with built-in metrics

## Security Features

- API key authentication
- JWT token support
- Input validation for all endpoints
- SQL injection prevention through parameterized queries
- Sensitive information encryption in configuration

## Monitoring and Observability

- Structured logging with tracing
- Prometheus metrics endpoint (`/metrics`)
- Health check endpoints
- Distributed tracing support

## Development Workflow

1. Follow SOLID principles in design
2. Run `cargo fmt` before committing
3. Ensure `cargo clippy` passes without warnings
4. Run full test suite with `cargo test`
5. Update documentation for API changes
6. Use semantic versioning for releases

## Common Patterns

### Service Implementation

```rust
#[async_trait]
impl TaskControlService for TaskService {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
        // Implementation
    }
}
```

### Error Handling

```rust
async fn operation() -> SchedulerResult<T> {
    some_fallible_operation()
        .await
        .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;
    Ok(result)
}
```

### Configuration Loading

```rust
let config = AppConfig::from_file("config/development.toml").await?;
```

This distributed task scheduler emphasizes reliability, scalability, and maintainability through modern Rust practices and enterprise-grade architectural patterns.