# Project Structure Documentation

## Overview

This project is a Rust workspace containing multiple interconnected crates that implement a task scheduling and execution system.

## Main Components

### Core Component (core)

- **Functionality**: Provides base models, configuration loading and core traits
- **Key Files**:
  - `models/`: Core data structures (Task, TaskRun, Worker etc.)
  - `config/`: Configuration loading and management
  - `traits/`: Core interfaces (MessageQueue, Repository, Scheduler etc.)

### API Component (api)

- **Functionality**: Provides HTTP API interfaces
- **Key Files**:
  - `routes.rs`: API route definitions
  - `handlers.rs`: Request handlers
  - `middleware.rs`: Middleware implementations

### Dispatcher Component (dispatcher)

- **Functionality**: Task scheduling and dependency management
- **Key Files**:
  - `scheduler.rs`: Scheduler implementation
  - `strategies.rs`: Scheduling strategies
  - `dependency_checker.rs`: Dependency checking

### Infrastructure Component (infrastructure)

- **Functionality**: Database and message queue implementations
- **Key Files**:
  - `database/`: PostgreSQL repository implementations
  - `message_queue.rs`: Message queue implementation
  - `observability.rs`: Observability tools

### Worker Component (worker)

- **Functionality**: Task execution
- **Key Files**:
  - `executors.rs`: Task executors
  - `heartbeat.rs`: Heartbeat monitoring
  - `service.rs`: Worker service

## Complete Directory Structure

```
.
├── config/                  # Configuration files
│   ├── development.toml
│   ├── production.toml
│   └── scheduler.toml
├── crates/
│   ├── api/                 # API component
│   ├── core/                # Core component
│   ├── dispatcher/          # Dispatcher component
│   ├── infrastructure/      # Infrastructure component
│   └── worker/              # Worker component
├── docs/                    # Documentation
│   └── configuration.md
├── examples/                # Example code
└── migrations/              # Database migrations
```

## Key Dependencies

1. `core` is the base component, depended on by all other components
2. `api` depends on `core` and `infrastructure`
3. `dispatcher` depends on `core` and `infrastructure`
4. `worker` depends on `core` and `infrastructure`
5. `infrastructure` depends on `core`

## Naming Conventions

### Rust Conventions

- **Modules**: `snake_case` (e.g., `task_scheduler`)
- **Structs/Enums**: `PascalCase` (e.g., `TaskScheduler`)
- **Functions/Variables**: `snake_case` (e.g., `create_task`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `MAX_RETRIES`)
- **Traits**: `PascalCase` with descriptive suffixes (e.g., `TaskRepository`)

### File Organization

- **Service implementations**: `{service_name}_service.rs`
- **Repository implementations**: `{entity}_repository.rs`
- **Error types**: `{module}_error.rs`
- **Configuration**: `{component}_config.rs`

### Database Conventions

- **Tables**: `snake_case` plural (e.g., `tasks`, `task_runs`)
- **Columns**: `snake_case` (e.g., `created_at`, `max_retries`)
- **Indexes**: `idx_{table}_{columns}` (e.g., `idx_tasks_status_schedule`)
- **Foreign Keys**: `fk_{table}_{referenced_table}`

## Configuration Structure

### Environment-based Configuration

```toml
# config/development.toml
[database]
url = "postgresql://localhost/scheduler_dev"
max_connections = 10

[message_queue]
url = "amqp://localhost:5672"
task_queue = "tasks"
status_queue = "status_updates"

[dispatcher]
enabled = true
schedule_interval_seconds = 10
max_concurrent_dispatches = 100

[worker]
enabled = false
worker_id = "dev-worker-001"
max_concurrent_tasks = 5
heartbeat_interval_seconds = 30

[api]
bind_address = "127.0.0.1:8080"
cors_enabled = true
```

## Testing Structure

### Test Organization

- **Unit Tests**: apart from source code, `tests/{rust_source_name}_test.rs`
- **Integration Tests**: In `tests/integration/` directory
- **End-to-End Tests**: In `tests/e2e/` directory
- **Test Fixtures**: In `tests/fixtures/` directory

### Test Naming

- **Unit Tests**: `test_{function_name}_{scenario}`
- **Integration Tests**: `test_{component}_integration_{scenario}`
- **E2E Tests**: `test_{workflow}_{scenario}`

## Documentation Standards

### Code Documentation

- **Public APIs**: Comprehensive rustdoc comments
- **Complex Logic**: Inline comments explaining the "why"
- **Examples**: Include usage examples in doc comments
- **Error Cases**: Document error conditions and handling

### Architecture Documentation

- **ADRs**: Architecture Decision Records in `docs/adr/`
- **API Docs**: OpenAPI specifications in `docs/api/`
- **Deployment**: Deployment guides in `docs/deployment/`
