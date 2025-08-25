# JWT Authentication and Authorization System

This document describes the comprehensive JWT authentication and role-based authorization system implemented for the Rust task scheduler project.

## Overview

The authentication system provides:

- JWT token-based authentication
- Role-based access control (RBAC)
- API key authentication
- Password hashing with bcrypt
- Refresh token support
- Comprehensive authorization guards

## Architecture

### Core Components

1. **Authentication Models** (`crates/api/src/auth/models.rs`)
   - User entity with roles and permissions
   - User service for credential management
   - Role definitions: Admin, Operator, Viewer

2. **Authentication Service** (`crates/api/src/auth/service.rs`)
   - JWT token generation and validation
   - User authentication
   - Refresh token handling

3. **Authentication Middleware** (`crates/api/src/auth.rs`)
   - JWT extraction from Authorization header
   - API key extraction from X-API-Key header
   - Token validation and user context setting

4. **Authorization Guards** (`crates/api/src/auth/guards.rs`)
   - Route-level permission checks
   - Role-based access control
   - Middleware for fine-grained authorization

## User Roles and Permissions

### Role Hierarchy

1. **Admin** - Full system access
   - All permissions including system administration
   - Can manage users and API keys

2. **Operator** - Operational access
   - Task management (read/write)
   - Worker management (read/write)
   - System monitoring (read-only)

3. **Viewer** - Read-only access
   - Task viewing (read-only)
   - Worker monitoring (read-only)
   - System statistics (read-only)

### Permission Matrix

| Permission | Admin | Operator | Viewer |
|------------|-------|----------|--------|
| TaskRead | ✅ | ✅ | ✅ |
| TaskWrite | ✅ | ✅ | ❌ |
| TaskDelete | ✅ | ❌ | ❌ |
| WorkerRead | ✅ | ✅ | ✅ |
| WorkerWrite | ✅ | ✅ | ❌ |
| SystemRead | ✅ | ✅ | ✅ |
| SystemWrite | ✅ | ❌ | ❌ |
| Admin | ✅ | ❌ | ❌ |

## Authentication Flow

### 1. User Login

```bash
POST /api/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "admin123"
}
```

**Response:**

```json
{
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
    "token_type": "Bearer",
    "expires_in": 86400,
    "user": {
      "id": "uuid-here",
      "username": "admin",
      "email": "admin@example.com",
      "role": "Admin",
      "permissions": ["Admin", "TaskRead", "TaskWrite", ...]
    }
  }
}
```

### 2. Token Refresh

```bash
POST /api/auth/refresh
Content-Type: application/json

{
  "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9..."
}
```

### 3. Protected API Access

```bash
GET /api/tasks
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...
```

## API Key Authentication

### Generate API Key

```bash
POST /api/auth/api-keys
Authorization: Bearer admin-token
```

### Use API Key

```bash
GET /api/tasks
X-API-Key: your-api-key-here
```

## Configuration

### JWT Configuration

```toml
[api.auth]
enabled = true
jwt_secret = "your-super-secret-jwt-key-min-32-chars"
jwt_expiration_hours = 24
refresh_token_expiration_days = 30
password_hash_cost = 12
```

### API Key Configuration

```toml
[api.auth.api_keys."hashed-key-here"]
name = "admin-key"
permissions = ["Admin"]
is_active = true
```

## Usage Examples

### Basic Authentication

```rust
use axum::{extract::State, Json};
use scheduler_api::auth::AuthenticatedUser;

pub async fn protected_endpoint(
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    // User is authenticated
    Ok(Json(json!({
        "message": "Access granted",
        "user": user.user_id
    })))
}
```

### Permission-Based Authorization

```rust
use scheduler_api::auth::{AuthenticatedUser, Permission, guards::RequirePermission};

pub async fn admin_only_endpoint(
    _permission: guards::RequirePermission<Permission::Admin>,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Only users with Admin permission can access
    Ok(Json(json!({
        "message": "Admin access granted"
    })))
}
```

### Role-Based Authorization

```rust
use scheduler_api::auth::{AuthenticatedUser, guards::RequireRole};

pub async fn operator_endpoint(
    _role: RequireRole,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Only Admin and Operator roles can access
    Ok(Json(json!({
        "message": "Operator access granted"
    })))
}
```

## Security Features

### Password Security

- bcrypt password hashing with configurable cost
- Secure password comparison
- No plaintext password storage

### Token Security

- JWT tokens with HS256 algorithm
- Configurable expiration times
- Refresh token rotation
- Secure key management

### Request Security

- CORS configuration
- Request size limits
- Timeout controls
- Input validation

### API Security

- API key hashing with SHA256
- Permission-based access control
- Rate limiting ready
- Audit logging

## Testing

The authentication system includes comprehensive tests:

```bash
# Run authentication tests
cargo test -p scheduler-api auth_tests

# Run all API tests
cargo test -p scheduler-api

# Run integration tests
cargo test --test integration_tests
```

## Error Handling

The system provides clear error responses:

```json
{
  "error": {
    "code": "UNAUTHORIZED",
    "message": "Missing authentication token"
  }
}
```

Common error codes:

- `UNAUTHORIZED` - Missing or invalid authentication
- `FORBIDDEN` - Insufficient permissions
- `BAD_REQUEST` - Invalid request format
- `INTERNAL` - Server error

## Migration Guide

### From API Key Only

1. Update dependencies in `Cargo.toml`
2. Add new authentication modules
3. Update route handlers
4. Add user management
5. Update configuration

### From Basic Auth

1. Replace basic auth with JWT
2. Add permission system
3. Update middleware
4. Migrate user credentials

## Best Practices

1. **Environment Variables**: Use environment variables for secrets
2. **Token Expiration**: Set appropriate expiration times
3. **Permission Principle**: Follow least privilege principle
4. **Key Rotation**: Regularly rotate JWT secrets
5. **Monitoring**: Monitor authentication attempts
6. **Logging**: Log authentication events securely

## Troubleshooting

### Common Issues

1. **Token Validation Failed**
   - Check JWT secret configuration
   - Verify token format
   - Check expiration time

2. **Permission Denied**
   - Verify user permissions
   - Check role assignments
   - Review route guards

3. **API Key Issues**
   - Verify key hashing
   - Check key activation status
   - Review permission assignments

### Debug Mode

Enable debug logging for authentication:

```rust
use tracing::info;

info!("Auth debug: {:?}", user.permissions);
```

## Future Enhancements

1. **Multi-factor Authentication**
2. **OAuth2 Integration**
3. **Rate Limiting**
4. **Session Management**
5. **Audit Logging**
6. **Password Policies**
7. **Account Lockout**
8. **CORS Enhancement**
