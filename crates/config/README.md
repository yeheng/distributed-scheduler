# Scheduler Config

Configuration management crate for the distributed task scheduler system.

## Overview

This crate provides configuration management functionality for the scheduler system, including:

- **Configuration Models**: Structured configuration types for all system components
- **Environment Management**: Support for different deployment environments
- **Validation**: Comprehensive validation for all configuration parameters
- **Serialization**: Support for TOML, JSON, and other formats
- **Circuit Breaker Configuration**: Resilience patterns and circuit breaker settings

## Features

### Configuration Models
- `AppConfig`: Root configuration structure
- `DatabaseConfig`: Database connection settings
- `MessageQueueConfig`: Message queue (RabbitMQ/Redis) settings
- `DispatcherConfig`: Task dispatcher configuration
- `WorkerConfig`: Worker node configuration
- `ApiConfig`: HTTP API server configuration
- `ObservabilityConfig`: Logging, metrics, and tracing settings
- `ResilienceConfig`: Circuit breaker and resilience patterns

### Environment Support
- `Environment`: Development, Testing, Staging, Production
- `ConfigProfile`: Environment-specific configuration profiles
- `ProfileRegistry`: Management of multiple configuration profiles

### Validation
- Comprehensive validation for all configuration parameters
- Type-specific validation (ports, URLs, timeouts, etc.)
- Business logic validation (e.g., circuit breaker thresholds)
- Custom validation traits for extensibility

## Usage

### Basic Configuration Loading

```rust
use scheduler_config::AppConfig;

// Load configuration from file
let config = AppConfig::load(Some("config/scheduler.toml"))?;

// Or load from TOML string
let toml_str = r#"
[database]
url = "postgresql://localhost/scheduler"
max_connections = 10

[dispatcher]
enabled = true
schedule_interval_seconds = 10
"#;

let config = AppConfig::from_toml(toml_str)?;
```

### Environment Management

```rust
use scheduler_config::{Environment, ConfigProfile};

// Get current environment
let env = Environment::current()?;
println!("Current environment: {}", env);

// Create a custom profile
let profile = ConfigProfile::new("custom".to_string(), Environment::Production)
    .with_feature("monitoring".to_string(), true)
    .with_override("log_level".to_string(), serde_json::Value::String("debug".to_string()));
```

### Configuration Validation

```rust
use scheduler_config::AppConfig;

let config = AppConfig::default();
if let Err(e) = config.validate() {
    eprintln!("Configuration validation failed: {}", e);
    // Handle validation errors
}
```

### Circuit Breaker Configuration

```rust
use scheduler_config::CircuitBreakerConfig;
use std::time::Duration;

let circuit_breaker_config = CircuitBreakerConfig {
    failure_threshold: 5,
    recovery_timeout: Duration::from_secs(60),
    success_threshold: 3,
    call_timeout: Duration::from_secs(30),
    backoff_multiplier: 2.0,
    max_recovery_timeout: Duration::from_secs(300),
};

circuit_breaker_config.validate()?;
```

## Configuration Structure

The configuration is organized hierarchically:

```
AppConfig
├── database
├── message_queue
├── dispatcher
├── worker
├── api
│   ├── auth
│   └── rate_limiting
├── observability
└── resilience
    ├── database_circuit_breaker
    ├── message_queue_circuit_breaker
    └── external_service_circuit_breaker
```

## Error Handling

The crate uses `ConfigResult<T>` as a result type, which is an alias for `Result<T, ConfigError>`. The `ConfigError` enum provides detailed error information for configuration-related issues.

## Testing

Run tests with:

```bash
cargo test -p scheduler-config
```

## Dependencies

- `serde`: Serialization and deserialization
- `toml`: TOML format support
- `config`: Configuration file loading
- `anyhow`: Error handling
- `thiserror`: Custom error types
- `chrono`: Time handling
- `url`: URL parsing
- `uuid`: UUID generation