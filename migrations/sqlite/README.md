# SQLite Database Migrations

This directory contains SQLite-specific database migrations for the embedded scheduler system.

## Migration Files

### 001_initial_schema.sql

- Creates all core tables: `tasks`, `task_runs`, `workers`, `task_dependencies`, `task_locks`, `jwt_token_blacklist`, `system_state`
- Defines constraints and foreign key relationships
- Adapts PostgreSQL schema to SQLite syntax
- Includes the `system_state` table for embedded system management

### 002_indexes_optimization.sql

- Creates indexes for optimal query performance
- Includes partial indexes for common query patterns
- Optimizes for typical scheduler workloads

### 003_triggers_and_constraints.sql

- Creates triggers for automatic `updated_at` timestamp updates
- Defines useful views for common queries:
  - `active_tasks`: Currently active tasks
  - `pending_task_runs`: Tasks waiting to be executed
  - `active_workers`: Available workers
  - `task_execution_stats`: Task performance statistics

### 004_maintenance_procedures.sql

- Defines maintenance queries for data cleanup
- Creates system health monitoring views
- Includes performance metrics views
- Sets up initial system state records

## Key Differences from PostgreSQL

### Data Types

- `BIGSERIAL` → `INTEGER PRIMARY KEY AUTOINCREMENT`
- `JSONB` → `TEXT` (JSON stored as text)
- `TIMESTAMPTZ` → `DATETIME`
- `INET` → `TEXT`
- `TEXT[]` → `TEXT` (JSON array as text)
- `BIGINT[]` → `TEXT` (JSON array as text)

### Features

- Uses triggers instead of automatic timestamp updates
- Views replace some stored procedure functionality
- Check constraints for data validation
- Foreign key constraints with CASCADE options

### Constraints

- All PostgreSQL check constraints preserved
- Foreign key relationships maintained
- Unique constraints preserved
- NOT NULL constraints preserved

## Usage

These migrations are designed to be run by SQLx migrate in order:

```rust
// In your embedded application
sqlx::migrate!("./migrations/sqlite").run(&pool).await?;
```

## Compatibility

The SQLite schema maintains full compatibility with the PostgreSQL version:

- Same table names and column names
- Same data validation rules
- Same relationships and constraints
- Compatible with existing repository implementations

## Performance Considerations

- Indexes are optimized for typical scheduler queries
- Partial indexes reduce storage overhead
- Views provide pre-computed common queries
- Foreign key constraints ensure data integrity

## Maintenance

Regular maintenance queries are provided in `004_maintenance_procedures.sql`:

- Clean up expired JWT tokens
- Remove expired task locks
- Archive old task execution records
- Update worker status based on heartbeat

These can be run periodically by the embedded scheduler system.
