# Merged Todo List - Scheduler System Comprehensive Improvements

This prioritized todo list combines critical bug fixes with architectural improvements based on comprehensive code analysis and second-round tuning recommendations.

## 🔴 Critical Issues (Must Fix First)

### 1. Complete Application Service Layer

- **File**: `crates/application/src/services/scheduler_service.rs`
- **Issue**: Service layer contains only `todo!()` placeholders
- **Action**: Implement missing methods in `SchedulerService`
- **Priority**: Critical
- **Status**: Completed

### 2. Fix MockMessageQueue Async Safety

- **File**: `crates/core/src/traits/scheduler.rs:242-297`
- **Issue**: Using `std::sync::Mutex` in async context causes potential deadlocks
- **Action**: Replace with `tokio::sync::Mutex`
- **Priority**: Critical
- **Status**: Completed

### 3. Complete TaskScheduler Trait Implementation

- **File**: `crates/dispatcher/src/scheduler.rs:307-363`
- **Issue**: Missing trait method implementations marked with `#[allow(dead_code)]`
- **Action**: Properly implement `start`, `stop`, `is_running`, `get_stats`, `reload_config` methods
- **Priority**: Critical
- **Status**: Completed

## 🟡 High Priority Issues

### 4. Add Input Validation and Sanitization

- **File**: `crates/api/src/handlers/tasks.rs`
- **Issue**: Missing input sanitization for task parameters
- **Action**: Implement proper validation for task parameters in API handlers
- **Priority**: High
- **Status**: Completed

### 5. Parallelize Task Scheduling

- **File**: `crates/dispatcher/src/scheduler.rs:112`
- **Issue**: Sequential task processing in `scan_and_schedule`
- **Action**: Use `futures::stream` for concurrent processing with configurable parallelism
- **Priority**: High
- **Status**: Completed

### 6. Fix Error Swallowing in Message Serialization

- **File**: `crates/domain/entities.rs:541,554,563,573`
- **Issue**: Using `.unwrap_or()` on serialization hides errors
- **Action**: Replace with proper error handling and propagation
- **Priority**: High
- **Status**: Completed

### 7. Add Authentication/Authorization Layer

- **File**: `crates/api/src/handlers/*.rs`
- **Issue**: No authentication/authorization visible in API handlers
- **Action**: Implement JWT-based authentication and role-based authorization
- **Priority**: High
- **Status**: Completed

### 8. Enhance Error Handling

- **File**: All Repository implementations
- **Issue**: Missing contextual error information for debugging
- **Action**: Apply `anyhow::Context` to enhance all Repository implementations with detailed error context
- **Priority**: High
- **Status**: Completed

## 🟢 Medium Priority Issues

### 9. Refactor Database Mapping Logic

- **File**: `crates/infrastructure/src/database/*/repositories.rs`
- **Issue**: Code duplication in `row_to_*` functions across PostgreSQL and SQLite
- **Action**: Use `sqlx::FromRow` or shared mapping logic to reduce duplication
- **Priority**: Medium
- **Status**: Completed

### 10. Implement Circuit Breaker Pattern

- **File**: `crates/core/src/circuit_breaker.rs`
- **Issue**: No circuit breakers for external service calls
- **Action**: Add circuit breaker implementation for database and message queue calls
- **Priority**: Medium
- **Status**: Completed

### 11. Add Rate Limiting

- **File**: `crates/api/src/middleware.rs`
- **Issue**: No rate limiting to prevent resource exhaustion
- **Action**: Implement rate limiting middleware using token bucket algorithm
- **Priority**: Medium
- **Status**: Completed

### 12. Optimize Database Queries

- **File**: `crates/infrastructure/src/database/postgres/postgres_task_repository.rs:51`
- **Issue**: N+1 query pattern for recent runs
- **Action**: Use JOIN queries or batch loading to reduce database calls
- **Priority**: Medium
- **Status**: Completed

### 13. Add Proper Timeout Handling

- **File**: Multiple async operations across codebase
- **Issue**: Missing timeout handling in async operations
- **Action**: Add `tokio::time::timeout` wrapper for all external calls
- **Priority**: Medium
- **Status**: Completed

### 14. Implement Caching Layer

- **File**: New file - `crates/infrastructure/src/cache/`
- **Issue**: No caching for frequently accessed data
- **Action**: Add Redis-based caching for task definitions and worker status
- **Priority**: Medium
- **Status**: Completed

## 🔵 Architectural Refactoring (Phase 2)

### 15. Create Scheduler-Config Crate

- **File**: New crate - `crates/scheduler-config/`
- **Issue**: Configuration logic mixed in core crate
- **Action**: Move `core/config` module to dedicated configuration crate
- **Priority**: Architectural
- **Status**: Completed

### 16. Reorganize Service Interfaces

- **File**: `crates/core/src/traits/` → `crates/application/src/interfaces/`
- **Issue**: Application service interfaces in wrong crate violates dependency rules
- **Action**: Move application service interfaces from core/traits to application/interfaces
- **Priority**: Architectural
- **Status**: Completed

### 17. Rename Core Crate

- **File**: `crates/core/` → `crates/scheduler-foundation/`
- **Issue**: Core crate has unclear responsibilities
- **Action**: Rename scheduler-core to scheduler-foundation and clarify responsibilities
- **Priority**: Architectural
- **Status**: Completed

### 18. Create Observability Crate

- **File**: New crate - `crates/scheduler-observability/`
- **Issue**: Observability code scattered across multiple crates
- **Action**: Merge core/logging and infrastructure/observability into dedicated crate
- **Priority**: Architectural
- **Status**: Completed

### 19. Configuration-Driven Executor Loading

- **File**: `crates/worker/src/executors.rs`
- **Issue**: Hardcoded executor registration
- **Action**: Implement ExecutorFactory with configuration-driven loading mechanism
- **Priority**: Architectural
- **Status**: Pending

## 🔵 Enhancement Features (Phase 3)

### 20. Enhance API PATCH Operations

- **File**: `crates/api/src/handlers/tasks.rs`
- **Issue**: Cannot distinguish between null and undefined in PATCH requests
- **Action**: Use `UpdateValue<T>` enum to support precise PATCH semantics
- **Priority**: Enhancement
- **Status**: Pending

### 21. Implement Distributed Tracing

- **File**: All service layers
- **Issue**: Limited tracing capabilities across service boundaries
- **Action**: Implement distributed tracing with context propagation through message queues
- **Priority**: Enhancement
- **Status**: Pending

## 🔵 Low Priority Issues

### 22. Remove Dead Code

- **File**: `crates/dispatcher/src/scheduler.rs`
- **Issue**: Methods marked with `#[allow(dead_code)]`
- **Action**: Clean up unused methods or properly integrate them
- **Priority**: Low
- **Status**: Pending

### 23. Add Comprehensive Integration Tests

- **File**: `crates/*/tests/`
- **Issue**: Limited integration test coverage
- **Action**: Add end-to-end workflow tests for core functionality
- **Priority**: Low
- **Status**: Pending

### 24. Implement Proper Resource Cleanup

- **File**: `crates/worker/src/executors.rs`
- **Issue**: Missing resource cleanup in task executors
- **Action**: Add proper cleanup methods and resource management
- **Priority**: Low
- **Status**: Pending

## 📝 Documentation & Testing

### 25. Update Architecture Documentation

- **File**: Documentation files and diagrams
- **Issue**: Architecture diagrams don't reflect new crate structure
- **Action**: Update dependency diagrams to reflect new crate structure and boundaries
- **Priority**: Documentation
- **Status**: Pending

### 26. Add Performance Benchmarks

- **File**: `benches/` directory
- **Issue**: No performance validation for optimizations
- **Action**: Add benchmark tests for parallelized task scanning to validate improvements
- **Priority**: Testing
- **Status**: Pending

## Implementation Roadmap

### Phase 1: Critical Fixes (Week 1-2)

- Complete items 1-8 (Critical and High Priority)
- Focus on core functionality and safety issues

### Phase 2: Quality Improvements (Week 3-4)

- Complete items 9-14 (Medium Priority)
- Focus on performance and reliability enhancements

### Phase 3: Architecture Refactoring (Week 5-8)

- Complete items 15-19 (Architectural)
- Major structural changes requiring careful dependency management

### Phase 4: Features & Polish (Week 9-10)

- Complete items 20-26 (Enhancement, Low Priority, Documentation)
- Final improvements and comprehensive testing

## Progress Summary

- **Total Issues**: 26
- **Critical**: 3 (12%)
- **High Priority**: 5 (19%)
- **Medium Priority**: 6 (23%)
- **Architectural**: 5 (19%)
- **Enhancement**: 2 (8%)
- **Low Priority**: 3 (12%)
- **Documentation/Testing**: 2 (8%)
- **Completed**: 18 (69%)
- **In Progress**: 0 (0%)
- **Pending**: 8 (31%)

## Notes

- **Risk Management**: Critical issues must be resolved before any architectural changes
- **Dependency Awareness**: Architectural changes (Phase 3) will require updating multiple modules
- **Testing Strategy**: Each phase should include comprehensive testing before moving to the next
- **Branch Strategy**: Consider feature branches for each phase to enable parallel development
- **Performance Validation**: Benchmark critical paths before and after optimizations
