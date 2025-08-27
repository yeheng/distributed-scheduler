# Complete API Reference

**Version**: v1.0  
**Base URL**: `http://localhost:8080`  
**Content-Type**: `application/json`

## 📋 Overview

This document provides a complete reference for the distributed task scheduler REST API, covering authentication, task management, worker management, and system monitoring endpoints.

## 🔐 Authentication

The API supports two authentication methods:

- **JWT Tokens**: For user sessions with automatic expiration
- **API Keys**: For programmatic access and long-lived integrations

### Authentication Header Format

```http
Authorization: Bearer <jwt_token_or_api_key>
```

---

## 🔑 Authentication Endpoints

### Login

**POST** `/api/auth/login`

Authenticate user and receive JWT tokens.

**Request Body:**

```json
{
  "username": "admin",
  "password": "admin123"
}
```

**Default User Accounts:**

- `admin` / `admin123` - Full administrative access
- `operator` / `op123` - Task and worker management
- `viewer` / `view123` - Read-only access

**Response:**

```json
{
  "success": true,
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
    "token_type": "Bearer",
    "expires_in": 3600,
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "username": "admin",
      "email": "admin@example.com",
      "role": "Admin",
      "permissions": ["Admin"]
    }
  }
}
```

### Refresh Token

**POST** `/api/auth/refresh`

Refresh expired JWT token using refresh token.

**Request Body:**

```json
{
  "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9..."
}
```

**Response:** Same format as login response with new tokens.

### Validate Token

**GET** `/api/auth/validate`

Validate current token and get user information.

**Authentication Required:** Yes

**Response:**

```json
{
  "success": true,
  "data": {
    "valid": true,
    "user_id": "admin",
    "permissions": ["Admin"],
    "expires_at": 1704067200
  }
}
```

### Create API Key

**POST** `/api/auth/api-keys`

Create a new API key for programmatic access.

**Authentication Required:** Yes (Admin permission)

**Request Body:**

```json
{
  "name": "deployment-service",
  "permissions": ["TaskRead", "TaskWrite", "WorkerRead"],
  "expires_in_days": 365
}
```

**Available Permissions:**

- `TaskRead` - View tasks and task runs
- `TaskWrite` - Create and modify tasks
- `TaskDelete` - Delete tasks
- `WorkerRead` - View worker information
- `WorkerWrite` - Modify worker settings
- `SystemRead` - View system statistics
- `SystemWrite` - Modify system configuration
- `Admin` - Full administrative access

**Response:**

```json
{
  "success": true,
  "data": {
    "api_key": "sk_live_abcd1234efgh5678ijkl9012mnop3456",
    "name": "deployment-service",
    "permissions": ["TaskRead", "TaskWrite", "WorkerRead"],
    "created_at": "2024-01-01T00:00:00Z",
    "expires_at": "2025-01-01T00:00:00Z"
  }
}
```

### Logout

**POST** `/api/auth/logout`

Invalidate current session.

**Authentication Required:** Yes

**Response:**

```json
{
  "success": true,
  "data": null
}
```

---

## 📋 Task Management

### Create Task

**POST** `/api/tasks`

Create a new scheduled task.

**Authentication Required:** Yes (TaskWrite permission)

**Request Body:**

```json
{
  "name": "daily-backup",
  "task_type": "shell",
  "schedule": "0 2 * * *",
  "parameters": {
    "command": "pg_dump mydb > /backup/$(date +%Y%m%d).sql",
    "working_directory": "/backup"
  },
  "timeout_seconds": 3600,
  "max_retries": 3,
  "dependencies": [1, 2]
}
```

**Field Validations:**

- `name`: 1-255 characters
- `task_type`: 1-100 characters  
- `schedule`: Valid cron expression (1-255 characters)
- `timeout_seconds`: 1-86400 seconds (optional)
- `max_retries`: 0-10 retries (optional)
- `dependencies`: Array of task IDs (optional)

**Response:**

```json
{
  "success": true,
  "data": {
    "id": 123,
    "name": "daily-backup",
    "task_type": "shell",
    "schedule": "0 2 * * *",
    "parameters": {
      "command": "pg_dump mydb > /backup/$(date +%Y%m%d).sql",
      "working_directory": "/backup"
    },
    "timeout_seconds": 3600,
    "max_retries": 3,
    "status": "Active",
    "dependencies": [1, 2],
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
  }
}
```

### List Tasks

**GET** `/api/tasks`

Retrieve list of tasks with optional filtering and pagination.

**Authentication Required:** Yes (TaskRead permission)

**Query Parameters:**

- `page` - Page number (default: 1)
- `page_size` - Items per page (default: 20, max: 100)
- `status` - Filter by status: `Active`, `Inactive`
- `name` - Filter by name (partial match)
- `task_type` - Filter by task type

**Response:**

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": 123,
        "name": "daily-backup",
        "task_type": "shell",
        "schedule": "0 2 * * *",
        "parameters": {...},
        "timeout_seconds": 3600,
        "max_retries": 3,
        "status": "Active",
        "dependencies": [1, 2],
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
      }
    ],
    "total": 150,
    "page": 1,
    "page_size": 20,
    "total_pages": 8
  }
}
```

### Get Task Details

**GET** `/api/tasks/{id}`

Get detailed information about a specific task.

**Authentication Required:** Yes (TaskRead permission)

**Response:**

```json
{
  "success": true,
  "data": {
    "id": 123,
    "name": "daily-backup",
    "task_type": "shell",
    "schedule": "0 2 * * *",
    "parameters": {...},
    "timeout_seconds": 3600,
    "max_retries": 3,
    "status": "Active",
    "dependencies": [1, 2],
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z",
    "recent_runs": [
      {
        "id": 456,
        "task_id": 123,
        "status": "Completed",
        "worker_id": "worker-001",
        "retry_count": 0,
        "scheduled_at": "2024-01-01T02:00:00Z",
        "started_at": "2024-01-01T02:00:05Z",
        "completed_at": "2024-01-01T02:15:30Z",
        "result": "Backup completed successfully",
        "error_message": null,
        "execution_duration_ms": 925000,
        "created_at": "2024-01-01T02:00:00Z"
      }
    ],
    "execution_stats": {
      "total_runs": 30,
      "successful_runs": 28,
      "failed_runs": 2,
      "success_rate": 93.33,
      "average_duration_ms": 900000,
      "last_success": "2024-01-01T02:15:30Z",
      "last_failure": "2023-12-28T02:00:00Z"
    }
  }
}
```

### Update Task

**PUT** `/api/tasks/{id}`

Update task configuration (full replace).

**Authentication Required:** Yes (TaskWrite permission)

**Request Body:** Same format as create task request

### Partial Update Task

**PATCH** `/api/tasks/{id}`

Partially update specific task fields.

**Authentication Required:** Yes (TaskWrite permission)

**Request Body:**

```json
{
  "name": "updated-backup-name",
  "status": "Inactive",
  "timeout_seconds": 7200
}
```

### Delete Task

**DELETE** `/api/tasks/{id}`

Delete a task and all its run history.

**Authentication Required:** Yes (TaskDelete permission)

**Response:**

```json
{
  "success": true,
  "data": null
}
```

### Trigger Task

**POST** `/api/tasks/{id}/trigger`

Manually trigger immediate task execution.

**Authentication Required:** Yes (TaskWrite permission)

**Response:**

```json
{
  "success": true,
  "data": {
    "task_run_id": 789,
    "message": "Task triggered successfully"
  }
}
```

### Get Task Runs

**GET** `/api/tasks/{id}/runs`

Get execution history for a specific task.

**Authentication Required:** Yes (TaskRead permission)

**Query Parameters:**

- `page` - Page number (default: 1)
- `page_size` - Items per page (default: 20, max: 100)
- `status` - Filter by status: `Pending`, `Dispatched`, `Running`, `Completed`, `Failed`, `Timeout`

**Response:**

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": 456,
        "task_id": 123,
        "status": "Completed",
        "worker_id": "worker-001",
        "retry_count": 0,
        "scheduled_at": "2024-01-01T02:00:00Z",
        "started_at": "2024-01-01T02:00:05Z",
        "completed_at": "2024-01-01T02:15:30Z",
        "result": "Backup completed successfully",
        "error_message": null,
        "execution_duration_ms": 925000,
        "created_at": "2024-01-01T02:00:00Z"
      }
    ],
    "total": 30,
    "page": 1,
    "page_size": 20,
    "total_pages": 2
  }
}
```

### Get Task Execution Statistics

**GET** `/api/tasks/{id}/stats`

Get detailed execution statistics for a task.

**Authentication Required:** Yes (TaskRead permission)

**Response:**

```json
{
  "success": true,
  "data": {
    "total_runs": 30,
    "successful_runs": 28,
    "failed_runs": 2,
    "success_rate": 93.33,
    "average_duration_ms": 900000,
    "min_duration_ms": 780000,
    "max_duration_ms": 1200000,
    "last_success": "2024-01-01T02:15:30Z",
    "last_failure": "2023-12-28T02:00:00Z",
    "runs_last_24h": 1,
    "runs_last_7d": 7,
    "runs_last_30d": 30
  }
}
```

### Get Task Run Details

**GET** `/api/task-runs/{id}`

Get detailed information about a specific task run.

**Authentication Required:** Yes (TaskRead permission)

**Response:**

```json
{
  "success": true,
  "data": {
    "id": 456,
    "task_id": 123,
    "status": "Completed",
    "worker_id": "worker-001",
    "retry_count": 0,
    "scheduled_at": "2024-01-01T02:00:00Z",
    "started_at": "2024-01-01T02:00:05Z",
    "completed_at": "2024-01-01T02:15:30Z",
    "result": "Backup completed successfully",
    "error_message": null,
    "execution_duration_ms": 925000,
    "created_at": "2024-01-01T02:00:00Z"
  }
}
```

---

## 👷 Worker Management

### List Workers

**GET** `/api/workers`

Get list of all registered workers.

**Authentication Required:** Yes (WorkerRead permission)

**Query Parameters:**

- `status` - Filter by status: `Alive`, `Down`
- `page` - Page number (default: 1)
- `page_size` - Items per page (default: 20, max: 100)

**Response:**

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": "worker-001",
        "hostname": "worker-node-1",
        "ip_address": "192.168.1.100",
        "supported_task_types": ["shell", "http", "python"],
        "max_concurrent_tasks": 10,
        "current_task_count": 3,
        "status": "Alive",
        "load_percentage": 30.0,
        "last_heartbeat": "2024-01-01T12:00:00Z",
        "registered_at": "2024-01-01T00:00:00Z"
      }
    ],
    "total": 5,
    "page": 1,
    "page_size": 20,
    "total_pages": 1
  }
}
```

### Get Worker Details

**GET** `/api/workers/{id}`

Get detailed information about a specific worker.

**Authentication Required:** Yes (WorkerRead permission)

**Response:**

```json
{
  "success": true,
  "data": {
    "id": "worker-001",
    "hostname": "worker-node-1",
    "ip_address": "192.168.1.100",
    "supported_task_types": ["shell", "http", "python"],
    "max_concurrent_tasks": 10,
    "current_task_count": 3,
    "status": "Alive",
    "load_percentage": 30.0,
    "last_heartbeat": "2024-01-01T12:00:00Z",
    "registered_at": "2024-01-01T00:00:00Z"
  }
}
```

### Get Worker Statistics

**GET** `/api/workers/{id}/stats`

Get performance statistics for a specific worker.

**Authentication Required:** Yes (WorkerRead permission)

**Response:**

```json
{
  "success": true,
  "data": {
    "total_tasks_executed": 150,
    "successful_tasks": 145,
    "failed_tasks": 5,
    "success_rate": 96.67,
    "average_execution_time_ms": 45000,
    "current_load_percentage": 30.0,
    "uptime_seconds": 86400,
    "tasks_last_24h": 24,
    "tasks_last_7d": 168,
    "last_task_completed": "2024-01-01T11:45:00Z"
  }
}
```

---

## 📊 System Monitoring

### Health Check

**GET** `/health`

Basic health check endpoint (no authentication required).

**Response:**

```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "timestamp": "2024-01-01T12:00:00Z"
  }
}
```

### System Health

**GET** `/api/system/health`

Detailed system health information.

**Authentication Required:** Yes (SystemRead permission)

**Response:**

```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "database_status": "connected",
    "message_queue_status": "connected",
    "uptime_seconds": 86400,
    "timestamp": "2024-01-01T12:00:00Z"
  }
}
```

### System Statistics

**GET** `/api/system/stats`

Get comprehensive system statistics.

**Authentication Required:** Yes (SystemRead permission)

**Response:**

```json
{
  "success": true,
  "data": {
    "total_tasks": 150,
    "active_tasks": 120,
    "total_workers": 5,
    "active_workers": 4,
    "running_task_runs": 8,
    "completed_task_runs_today": 45,
    "failed_task_runs_today": 2
  }
}
```

### Prometheus Metrics

**GET** `/metrics`

Prometheus-compatible metrics endpoint (no authentication required).

**Response Format:** Prometheus text format

```
# HELP scheduler_tasks_total Total number of tasks
# TYPE scheduler_tasks_total counter
scheduler_tasks_total{status="active"} 120
scheduler_tasks_total{status="inactive"} 30

# HELP scheduler_task_runs_total Total number of task runs
# TYPE scheduler_task_runs_total counter
scheduler_task_runs_total{status="completed"} 1250
scheduler_task_runs_total{status="failed"} 85

# HELP scheduler_workers_total Total number of workers
# TYPE scheduler_workers_total gauge
scheduler_workers_total{status="alive"} 4
scheduler_workers_total{status="down"} 1
```

---

## 🏷️ Task Types and Parameters

### Shell Executor

```json
{
  "task_type": "shell",
  "parameters": {
    "command": "ls -la /tmp",
    "working_directory": "/tmp",
    "environment": {
      "PATH": "/usr/bin:/bin",
      "HOME": "/root"
    },
    "shell": "/bin/bash"
  }
}
```

### HTTP Executor

```json
{
  "task_type": "http",
  "parameters": {
    "url": "https://api.example.com/webhook",
    "method": "POST",
    "headers": {
      "Content-Type": "application/json",
      "Authorization": "Bearer token123"
    },
    "body": "{\"message\": \"Task completed\"}",
    "timeout_ms": 30000,
    "follow_redirects": true
  }
}
```

### Python Executor

```json
{
  "task_type": "python",
  "parameters": {
    "script_path": "/scripts/data_processor.py",
    "arguments": ["--input", "data.csv", "--output", "results.json"],
    "python_executable": "/usr/bin/python3",
    "working_directory": "/scripts",
    "environment": {
      "PYTHONPATH": "/scripts/lib"
    },
    "virtual_env": "/venv/data-processing"
  }
}
```

---

## ⚠️ Error Handling

### Standard Error Response Format

```json
{
  "success": false,
  "error": {
    "code": "TASK_NOT_FOUND",
    "message": "Task with ID 123 not found",
    "details": null
  },
  "timestamp": "2024-01-01T12:00:00Z"
}
```

### HTTP Status Codes

| Status | Code | Description |
|--------|------|-------------|
| 200 | OK | Request successful |
| 201 | Created | Resource created |
| 204 | No Content | Request successful, no content |
| 400 | Bad Request | Invalid request parameters |
| 401 | Unauthorized | Authentication required |
| 403 | Forbidden | Insufficient permissions |
| 404 | Not Found | Resource not found |
| 409 | Conflict | Resource conflict |
| 422 | Unprocessable Entity | Validation error |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Server error |
| 503 | Service Unavailable | Service temporarily unavailable |

### Common Error Codes

| Error Code | Description |
|------------|-------------|
| `AUTHENTICATION_REQUIRED` | Valid authentication token required |
| `INVALID_TOKEN` | Authentication token is invalid or expired |
| `INSUFFICIENT_PERMISSIONS` | User lacks required permissions |
| `TASK_NOT_FOUND` | Specified task does not exist |
| `WORKER_NOT_FOUND` | Specified worker does not exist |
| `VALIDATION_ERROR` | Request validation failed |
| `DUPLICATE_TASK_NAME` | Task name already exists |
| `INVALID_SCHEDULE` | Cron expression is invalid |
| `DEPENDENCY_CYCLE` | Task dependencies create a cycle |
| `DATABASE_ERROR` | Database operation failed |
| `MESSAGE_QUEUE_ERROR` | Message queue operation failed |
| `RATE_LIMIT_EXCEEDED` | Request rate limit exceeded |

---

## 🔧 Rate Limiting

The API implements rate limiting to prevent abuse:

- **Authentication endpoints**: 10 requests/minute per IP
- **Task operations**: 100 requests/minute per user
- **System monitoring**: 60 requests/minute per user

Rate limit headers are included in responses:

```http
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1704067260
```

When rate limit is exceeded:

```json
{
  "success": false,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Try again later.",
    "details": {
      "limit": 100,
      "reset_at": "2024-01-01T12:01:00Z"
    }
  },
  "timestamp": "2024-01-01T12:00:00Z"
}
```

---

## 📝 Usage Examples

### Complete Task Lifecycle

```bash
# 1. Authenticate
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}'

# 2. Create a task
curl -X POST http://localhost:8080/api/tasks \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "daily-report",
    "task_type": "python",
    "schedule": "0 9 * * *",
    "parameters": {
      "script_path": "/scripts/daily_report.py",
      "arguments": ["--format", "pdf"]
    },
    "timeout_seconds": 1800,
    "max_retries": 2
  }'

# 3. Trigger immediate execution
curl -X POST http://localhost:8080/api/tasks/123/trigger \
  -H "Authorization: Bearer YOUR_TOKEN"

# 4. Monitor execution
curl -X GET http://localhost:8080/api/tasks/123/runs \
  -H "Authorization: Bearer YOUR_TOKEN"

# 5. Get statistics
curl -X GET http://localhost:8080/api/tasks/123/stats \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### Worker Monitoring

```bash
# List all workers
curl -X GET http://localhost:8080/api/workers \
  -H "Authorization: Bearer YOUR_TOKEN"

# Get worker details
curl -X GET http://localhost:8080/api/workers/worker-001 \
  -H "Authorization: Bearer YOUR_TOKEN"

# Get worker performance stats
curl -X GET http://localhost:8080/api/workers/worker-001/stats \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### System Monitoring

```bash
# Check system health
curl -X GET http://localhost:8080/api/system/health \
  -H "Authorization: Bearer YOUR_TOKEN"

# Get system statistics
curl -X GET http://localhost:8080/api/system/stats \
  -H "Authorization: Bearer YOUR_TOKEN"

# Get Prometheus metrics
curl -X GET http://localhost:8080/metrics
```

---

## 🔒 Security Considerations

### Authentication Best Practices

- Use HTTPS in production environments
- Store tokens securely and never expose them in logs
- Implement proper token rotation for long-lived applications
- Use API keys for service-to-service communication
- Regular audit of API key usage and permissions

### Input Validation

- All request parameters are validated server-side
- SQL injection protection through parameterized queries
- XSS prevention through proper input sanitization
- File path validation to prevent directory traversal
- Command injection prevention in shell executors

### Rate Limiting

- Implement client-side rate limiting awareness
- Use exponential backoff for retry strategies
- Monitor rate limit headers in responses
- Consider implementing distributed rate limiting for multiple instances

---

This comprehensive API reference provides complete coverage of all available endpoints, authentication methods, request/response formats, and usage examples for the distributed task scheduler system.
