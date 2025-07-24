# Technology Stack

## Core Technologies

### Programming Language

- **Rust**: Primary language for all components
- **Edition**: 2024
- **MSRV**: 1.70+ (minimum supported Rust version)

### Database

- **PostgreSQL**: Primary data store for task definitions and execution history
- **SQLx**: Async database driver with compile-time query verification
- **Migrations**: Database schema versioning and migration management

### Message Queue

- **RabbitMQ**: Message broker for task distribution and status updates
- **AMQP Protocol**: Standard messaging protocol implementation

### Web Framework

- **Axum**: Modern async web framework for REST API
- **Tower**: Middleware and service abstractions
- **Serde**: JSON serialization/deserialization

### Async Runtime

- **Tokio**: Async runtime for all async operations
- **Async-trait**: Trait definitions for async methods

### Configuration

- **TOML**: Configuration file format
- **Environment Variables**: Runtime configuration overrides
- **Config Validation**: Startup-time configuration validation

### Observability

- **Tracing**: Structured logging and distributed tracing
- **OpenTelemetry**: Metrics and tracing export (Post-MVP)
- **Prometheus**: Metrics collection endpoint

### Testing

- **Tokio-test**: Async testing utilities
- **Testcontainers**: Integration testing with real databases
- **Mockall**: Mock generation for trait testing

## Build System

### Cargo Workspace

```toml
[workspace]
members = [
    "core",
    "dispatcher", 
    "worker",
    "api"
]
```

### Common Commands

#### Development

```bash
# Build all components
cargo build

# Run tests
cargo test

# Run with logs
RUST_LOG=debug cargo run

# Format code
cargo fmt

# Lint code
cargo clippy
```

#### Database Operations

```bash
# Run migrations
sqlx migrate run

# Create new migration
sqlx migrate add <migration_name>

# Reset database
sqlx database reset
```

#### Docker Operations

```bash
# Build image
docker build -t scheduler .

# Run with compose
docker-compose up -d

# View logs
docker-compose logs -f
```

#### Testing

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test integration

# End-to-end tests
cargo test --test e2e
```

## Dependencies

### Core Dependencies

- `tokio` - Async runtime
- `sqlx` - Database operations
- `serde` - Serialization
- `tracing` - Logging
- `anyhow` - Error handling
- `chrono` - Date/time handling
- `uuid` - Unique identifiers

### Dispatcher Dependencies

- `cron` - CRON expression parsing
- `axum` - HTTP server
- `lapin` - RabbitMQ client

### Worker Dependencies

- `lapin` - RabbitMQ client
- `reqwest` - HTTP client for HTTP tasks
- `tokio-process` - Process execution

## Environment Setup

### Required Services

- PostgreSQL 14+
- RabbitMQ 3.8+

### Development Environment

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install SQLx CLI
cargo install sqlx-cli

# Start services
docker-compose -f docker-compose.dev.yml up -d
```
