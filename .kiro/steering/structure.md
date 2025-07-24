# Project Structure

## Cargo Workspace Layout

```
scheduler/
├── Cargo.toml              # Workspace configuration
├── Cargo.lock              # Dependency lock file
├── README.md               # Project documentation
├── docker-compose.yml      # Production services
├── docker-compose.dev.yml  # Development services
├── Dockerfile              # Multi-stage build
├── .env.example            # Environment template
├── migrations/             # Database migrations
│   ├── 001_initial.sql
│   ├── 002_add_indexes.sql
│   └── ...
├── config/                 # Configuration files
│   ├── dispatcher.toml
│   ├── worker.toml
│   └── development.toml
├── core/                   # Shared types and traits
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── models/         # Data models
│       ├── traits/         # Service traits
│       ├── errors.rs       # Error definitions
│       └── config.rs       # Configuration structs
├── dispatcher/             # Dispatcher service
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── scheduler/      # Task scheduling logic
│       ├── state_listener/ # Status update handling
│       ├── controller/     # Task control operations
│       └── strategies/     # Dispatch strategies
├── worker/                 # Worker service
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── service.rs      # Worker main service
│       ├── executors/      # Task executors
│       │   ├── mod.rs
│       │   ├── shell.rs
│       │   └── http.rs
│       └── heartbeat.rs    # Heartbeat management
├── api/                    # REST API server
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── handlers/       # HTTP handlers
│       ├── middleware/     # HTTP middleware
│       └── routes.rs       # Route definitions
├── infrastructure/         # Infrastructure abstractions
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── database/       # Database repositories
│       ├── message_queue/  # Message queue implementations
│       └── observability/  # Metrics and tracing
└── tests/                  # Integration tests
    ├── integration/
    ├── e2e/
    └── fixtures/
```

## Module Organization

### Core Module (`core/`)

Contains shared types, traits, and utilities used across all components.

**Key Files:**

- `models/` - Domain models (Task, TaskRun, Worker, etc.)
- `traits/` - Service trait definitions
- `errors.rs` - Centralized error types
- `config.rs` - Configuration structures

### Dispatcher Module (`dispatcher/`)

Implements the task scheduling and coordination logic.

**Key Components:**

- `scheduler/` - CRON-based task scheduling
- `state_listener/` - Worker status update processing
- `controller/` - Task lifecycle management
- `strategies/` - Worker selection strategies

### Worker Module (`worker/`)

Implements the task execution nodes.

**Key Components:**

- `service.rs` - Main worker service loop
- `executors/` - Pluggable task executors
- `heartbeat.rs` - Health reporting to dispatcher

### API Module (`api/`)

Provides REST API endpoints for task management.

**Key Components:**

- `handlers/` - HTTP request handlers
- `middleware/` - Authentication, logging, etc.
- `routes.rs` - Route configuration

### Infrastructure Module (`infrastructure/`)

Contains concrete implementations of infrastructure abstractions.

**Key Components:**

- `database/` - PostgreSQL repository implementations
- `message_queue/` - RabbitMQ client implementations
- `observability/` - Metrics and tracing setup

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

- **Unit Tests**: apart from source code, the same as source code folder, `{rust_source_name}_test.rs`
- **Integration Tests**: In `tests/integration/` directory
- **End-to-End Tests**: In `tests/e2e/` directory
- **Test Fixtures**: In `tests/fixtures/` directory

### Test Naming

- **Unit Tests**: `test_{function_name}_{scenario}`
- **Integration Tests**: `test_{component}_integration_{scenario}`
- **E2E Tests**: `test_{workflow}_{scenario}`

## Documentation Standards

### Code Documentation

- **Public APIs**: Comprehensive rustdoc comments, all use chinese
- **Complex Logic**: Inline comments explaining the "why"
- **Examples**: Include usage examples in doc comments
- **Error Cases**: Document error conditions and handling

### Architecture Documentation

- **ADRs**: Architecture Decision Records in `docs/adr/`
- **API Docs**: OpenAPI specifications in `docs/api/`
- **Deployment**: Deployment guides in `docs/deployment/`
