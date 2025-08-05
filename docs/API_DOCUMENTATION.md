# API 文档

## 📋 API 概述

分布式任务调度系统提供完整的REST API接口，支持任务管理、Worker管理、系统监控等功能。

### 基础信息

- **Base URL**: `http://localhost:8080/api`
- **认证方式**: API Key / JWT Token
- **数据格式**: JSON
- **字符编码**: UTF-8

### 响应格式

所有API响应都遵循统一格式：

#### 成功响应
```json
{
  "success": true,
  "data": {
    // 响应数据
  },
  "metadata": {
    "timestamp": "2024-01-01T00:00:00Z",
    "request_id": "req_123456789",
    "version": "1.0.0"
  }
}
```

#### 错误响应
```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "请求参数验证失败",
    "details": {
      "field": "name",
      "rule": "required",
      "value": null
    }
  },
  "metadata": {
    "timestamp": "2024-01-01T00:00:00Z",
    "request_id": "req_123456789",
    "version": "1.0.0"
  }
}
```

## 🔐 认证和授权

### API Key 认证

在请求头中添加API Key：

```http
Authorization: Bearer your-api-key
```

或者在查询参数中：

```
?api_key=your-api-key
```

### JWT Token 认证

在请求头中添加JWT Token：

```http
Authorization: Bearer your-jwt-token
```

## 📋 任务管理 API

### 创建任务

**POST** `/tasks`

创建新的任务。

#### 请求参数

```json
{
  "name": "string",
  "description": "string",
  "task_type": "string",
  "executor": "string",
  "command": "string",
  "arguments": ["string"],
  "schedule": "string",
  "priority": "low|normal|high|critical",
  "retry_count": "integer",
  "timeout_seconds": "integer",
  "dependencies": ["string"],
  "tags": ["string"],
  "metadata": {}
}
```

#### 字段说明

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| name | string | 是 | 任务名称，唯一标识 |
| description | string | 否 | 任务描述 |
| task_type | string | 是 | 任务类型 (python, shell, http, etc.) |
| executor | string | 是 | 执行器类型 |
| command | string | 是 | 执行命令 |
| arguments | array | 否 | 命令参数 |
| schedule | string | 否 | Cron表达式，用于定时任务 |
| priority | string | 否 | 优先级 (low, normal, high, critical) |
| retry_count | integer | 否 | 重试次数，默认3 |
| timeout_seconds | integer | 否 | 超时时间(秒)，默认3600 |
| dependencies | array | 否 | 依赖任务ID列表 |
| tags | array | 否 | 标签列表 |
| metadata | object | 否 | 自定义元数据 |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": 1,
    "name": "data_processing",
    "description": "处理每日数据",
    "task_type": "python",
    "executor": "python",
    "command": "process_data.py",
    "arguments": ["--input", "data.csv"],
    "schedule": "0 2 * * *",
    "priority": "normal",
    "retry_count": 3,
    "timeout_seconds": 3600,
    "status": "active",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z",
    "last_run_at": null,
    "next_run_at": "2024-01-01T02:00:00Z",
    "dependencies": [],
    "tags": ["data", "daily"],
    "metadata": {}
  }
}
```

### 获取任务列表

**GET** `/tasks`

获取任务列表，支持分页和过滤。

#### 查询参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| page | integer | 否 | 页码，默认1 |
| size | integer | 否 | 页大小，默认20 |
| status | string | 否 | 任务状态过滤 |
| task_type | string | 否 | 任务类型过滤 |
| priority | string | 否 | 优先级过滤 |
| search | string | 否 | 搜索关键词 |
| sort | string | 否 | 排序字段，默认created_at |
| order | string | 否 | 排序方向 (asc/desc)，默认desc |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": 1,
        "name": "data_processing",
        "description": "处理每日数据",
        "task_type": "python",
        "status": "active",
        "priority": "normal",
        "created_at": "2024-01-01T00:00:00Z",
        "next_run_at": "2024-01-01T02:00:00Z"
      }
    ],
    "total": 1,
    "page": 1,
    "size": 20,
    "total_pages": 1
  }
}
```

### 获取任务详情

**GET** `/tasks/{id}`

获取指定任务的详细信息。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 任务ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": 1,
    "name": "data_processing",
    "description": "处理每日数据",
    "task_type": "python",
    "executor": "python",
    "command": "process_data.py",
    "arguments": ["--input", "data.csv"],
    "schedule": "0 2 * * *",
    "priority": "normal",
    "retry_count": 3,
    "timeout_seconds": 3600,
    "status": "active",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z",
    "last_run_at": "2024-01-01T00:00:00Z",
    "next_run_at": "2024-01-01T02:00:00Z",
    "dependencies": [],
    "tags": ["data", "daily"],
    "metadata": {},
    "statistics": {
      "total_runs": 100,
      "successful_runs": 95,
      "failed_runs": 5,
      "average_execution_time": 30.5,
      "last_execution_status": "success"
    }
  }
}
```

### 更新任务

**PUT** `/tasks/{id}`

更新指定任务的信息。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 任务ID |

#### 请求参数

与创建任务相同的参数，所有字段都是可选的。

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": 1,
    "name": "data_processing_updated",
    "description": "处理每日数据",
    "task_type": "python",
    "executor": "python",
    "command": "process_data.py",
    "arguments": ["--input", "data.csv"],
    "schedule": "0 2 * * *",
    "priority": "high",
    "retry_count": 5,
    "timeout_seconds": 3600,
    "status": "active",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:01:00Z",
    "last_run_at": "2024-01-01T00:00:00Z",
    "next_run_at": "2024-01-01T02:00:00Z",
    "dependencies": [],
    "tags": ["data", "daily", "critical"],
    "metadata": {}
  }
}
```

### 删除任务

**DELETE** `/tasks/{id}`

删除指定任务。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 任务ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "message": "任务删除成功",
    "deleted_id": 1
  }
}
```

### 触发任务

**POST** `/tasks/{id}/trigger`

手动触发指定任务的执行。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 任务ID |

#### 请求参数

```json
{
  "parameters": {},
  "priority": "normal",
  "timeout_seconds": 3600
}
```

#### 响应示例

```json
{
  "success": true,
  "data": {
    "task_id": 1,
    "run_id": 123,
    "status": "pending",
    "worker_id": null,
    "started_at": null,
    "estimated_completion": "2024-01-01T00:05:00Z"
  }
}
```

### 暂停任务

**POST** `/tasks/{id}/pause`

暂停指定任务的执行。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 任务ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": 1,
    "status": "paused",
    "updated_at": "2024-01-01T00:01:00Z"
  }
}
```

### 恢复任务

**POST** `/tasks/{id}/resume`

恢复已暂停的任务。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 任务ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": 1,
    "status": "active",
    "updated_at": "2024-01-01T00:01:00Z"
  }
}
```

## 🏃 任务运行管理 API

### 获取任务运行列表

**GET** `/runs`

获取任务运行列表，支持分页和过滤。

#### 查询参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| page | integer | 否 | 页码，默认1 |
| size | integer | 否 | 页大小，默认20 |
| task_id | integer | 否 | 任务ID过滤 |
| status | string | 否 | 运行状态过滤 |
| worker_id | string | 否 | Worker ID过滤 |
| start_date | string | 否 | 开始日期 (YYYY-MM-DD) |
| end_date | string | 否 | 结束日期 (YYYY-MM-DD) |
| sort | string | 否 | 排序字段，默认created_at |
| order | string | 否 | 排序方向 (asc/desc)，默认desc |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": 123,
        "task_id": 1,
        "task_name": "data_processing",
        "worker_id": "worker-001",
        "status": "completed",
        "started_at": "2024-01-01T00:00:00Z",
        "completed_at": "2024-01-01T00:00:30Z",
        "result": "数据处理完成",
        "error_message": null,
        "retry_count": 0,
        "created_at": "2024-01-01T00:00:00Z"
      }
    ],
    "total": 1,
    "page": 1,
    "size": 20,
    "total_pages": 1
  }
}
```

### 获取任务运行详情

**GET** `/runs/{id}`

获取指定任务运行的详细信息。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 运行ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": 123,
    "task_id": 1,
    "task_name": "data_processing",
    "worker_id": "worker-001",
    "worker_hostname": "server-01",
    "status": "completed",
    "started_at": "2024-01-01T00:00:00Z",
    "completed_at": "2024-01-01T00:00:30Z",
    "execution_time": 30.5,
    "result": "数据处理完成",
    "error_message": null,
    "retry_count": 0,
    "parameters": {
      "input": "data.csv",
      "output": "processed_data.csv"
    },
    "logs": [
      {
        "timestamp": "2024-01-01T00:00:00Z",
        "level": "info",
        "message": "开始处理数据"
      },
      {
        "timestamp": "2024-01-01T00:00:30Z",
        "level": "info",
        "message": "数据处理完成"
      }
    ],
    "created_at": "2024-01-01T00:00:00Z"
  }
}
```

### 重启任务运行

**POST** `/runs/{id}/restart`

重启指定的任务运行。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 运行ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "original_run_id": 123,
    "new_run_id": 124,
    "status": "pending",
    "started_at": "2024-01-01T00:01:00Z"
  }
}
```

### 中止任务运行

**POST** `/runs/{id}/abort`

中止指定的任务运行。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | integer | 是 | 运行ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": 123,
    "status": "aborted",
    "completed_at": "2024-01-01T00:01:00Z",
    "message": "任务运行已中止"
  }
}
```

## 👷 Worker 管理 API

### 获取 Worker 列表

**GET** `/workers`

获取 Worker 列表，支持分页和过滤。

#### 查询参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| page | integer | 否 | 页码，默认1 |
| size | integer | 否 | 页大小，默认20 |
| status | string | 否 | Worker 状态过滤 |
| task_type | string | 否 | 任务类型过滤 |
| hostname | string | 否 | 主机名过滤 |
| sort | string | 否 | 排序字段，默认created_at |
| order | string | 否 | 排序方向 (asc/desc)，默认desc |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": "worker-001",
        "hostname": "server-01",
        "ip_address": "192.168.1.100",
        "port": 8080,
        "task_types": ["python", "shell"],
        "max_concurrent_tasks": 5,
        "current_task_count": 2,
        "status": "active",
        "last_heartbeat": "2024-01-01T00:00:00Z",
        "registered_at": "2024-01-01T00:00:00Z",
        "load_average": 0.4,
        "memory_usage": 512,
        "cpu_usage": 25.5
      }
    ],
    "total": 1,
    "page": 1,
    "size": 20,
    "total_pages": 1
  }
}
```

### 注册 Worker

**POST** `/workers`

注册新的 Worker。

#### 请求参数

```json
{
  "hostname": "string",
  "ip_address": "string",
  "port": "integer",
  "task_types": ["string"],
  "max_concurrent_tasks": "integer",
  "metadata": {}
}
```

#### 字段说明

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| hostname | string | 是 | 主机名 |
| ip_address | string | 是 | IP地址 |
| port | integer | 是 | 端口号 |
| task_types | array | 是 | 支持的任务类型列表 |
| max_concurrent_tasks | integer | 否 | 最大并发任务数，默认5 |
| metadata | object | 否 | 自定义元数据 |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": "worker-001",
    "hostname": "server-01",
    "ip_address": "192.168.1.100",
    "port": 8080,
    "task_types": ["python", "shell"],
    "max_concurrent_tasks": 5,
    "current_task_count": 0,
    "status": "active",
    "last_heartbeat": "2024-01-01T00:00:00Z",
    "registered_at": "2024-01-01T00:00:00Z",
    "metadata": {}
  }
}
```

### 获取 Worker 详情

**GET** `/workers/{id}`

获取指定 Worker 的详细信息。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | string | 是 | Worker ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": "worker-001",
    "hostname": "server-01",
    "ip_address": "192.168.1.100",
    "port": 8080,
    "task_types": ["python", "shell"],
    "max_concurrent_tasks": 5,
    "current_task_count": 2,
    "status": "active",
    "last_heartbeat": "2024-01-01T00:00:00Z",
    "registered_at": "2024-01-01T00:00:00Z",
    "load_average": 0.4,
    "memory_usage": 512,
    "cpu_usage": 25.5,
    "current_tasks": [
      {
        "run_id": 123,
        "task_id": 1,
        "task_name": "data_processing",
        "started_at": "2024-01-01T00:00:00Z",
        "status": "running"
      }
    ],
    "statistics": {
      "total_tasks_executed": 1000,
      "successful_tasks": 950,
      "failed_tasks": 50,
      "average_execution_time": 30.5,
      "uptime": "7d 12h 30m"
    }
  }
}
```

### 更新 Worker

**PUT** `/workers/{id}`

更新指定 Worker 的信息。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | string | 是 | Worker ID |

#### 请求参数

```json
{
  "max_concurrent_tasks": "integer",
  "task_types": ["string"],
  "status": "active|draining|offline",
  "metadata": {}
}
```

#### 响应示例

```json
{
  "success": true,
  "data": {
    "id": "worker-001",
    "hostname": "server-01",
    "ip_address": "192.168.1.100",
    "port": 8080,
    "task_types": ["python", "shell", "http"],
    "max_concurrent_tasks": 10,
    "current_task_count": 2,
    "status": "active",
    "last_heartbeat": "2024-01-01T00:00:00Z",
    "registered_at": "2024-01-01T00:00:00Z",
    "metadata": {}
  }
}
```

### 注销 Worker

**DELETE** `/workers/{id}`

注销指定的 Worker。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | string | 是 | Worker ID |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "message": "Worker 注销成功",
    "worker_id": "worker-001"
  }
}
```

### Worker 心跳

**POST** `/workers/{id}/heartbeat`

发送 Worker 心跳。

#### 路径参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | string | 是 | Worker ID |

#### 请求参数

```json
{
  "current_task_count": "integer",
  "load_average": "number",
  "memory_usage": "integer",
  "cpu_usage": "number",
  "current_tasks": ["integer"],
  "metadata": {}
}
```

#### 响应示例

```json
{
  "success": true,
  "data": {
    "worker_id": "worker-001",
    "status": "active",
    "last_heartbeat": "2024-01-01T00:00:00Z",
    "message": "心跳接收成功"
  }
}
```

## 🖥️ 系统管理 API

### 系统健康检查

**GET** `/system/health`

系统健康检查。

#### 响应示例

```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "timestamp": "2024-01-01T00:00:00Z",
    "version": "1.0.0",
    "components": {
      "database": {
        "status": "healthy",
        "response_time": 0.005,
        "connection_pool": {
          "total": 10,
          "active": 2,
          "idle": 8
        }
      },
      "message_queue": {
        "status": "healthy",
        "response_time": 0.002,
        "queue_size": 0
      },
      "scheduler": {
        "status": "healthy",
        "active_workers": 5,
        "pending_tasks": 0
      },
      "api": {
        "status": "healthy",
        "request_rate": 10.5,
        "response_time": 0.001
      }
    }
  }
}
```

### 系统统计信息

**GET** `/system/stats`

获取系统统计信息。

#### 响应示例

```json
{
  "success": true,
  "data": {
    "tasks": {
      "total": 100,
      "active": 80,
      "paused": 10,
      "completed": 10,
      "failed": 0
    },
    "task_runs": {
      "total": 1000,
      "pending": 5,
      "running": 10,
      "completed": 980,
      "failed": 5,
      "aborted": 0
    },
    "workers": {
      "total": 10,
      "active": 8,
      "offline": 1,
      "draining": 1,
      "average_load": 0.3
    },
    "performance": {
      "average_execution_time": 30.5,
      "tasks_per_minute": 50.2,
      "success_rate": 98.0,
      "system_uptime": "7d 12h 30m"
    },
    "resource_usage": {
      "cpu_usage": 25.5,
      "memory_usage": 1024,
      "disk_usage": 85.5,
      "network_io": {
        "bytes_in": 1024000,
        "bytes_out": 2048000
      }
    }
  }
}
```

### 系统指标

**GET** `/system/metrics`

获取系统性能指标（Prometheus 格式）。

#### 响应示例

```
# HELP scheduler_tasks_total Total number of tasks
# TYPE scheduler_tasks_total gauge
scheduler_tasks_total 100

# HELP scheduler_tasks_active Number of active tasks
# TYPE scheduler_tasks_active gauge
scheduler_tasks_active 80

# HELP scheduler_workers_total Total number of workers
# TYPE scheduler_workers_total gauge
scheduler_workers_total 10

# HELP scheduler_task_execution_duration_seconds Task execution duration
# TYPE scheduler_task_execution_duration_seconds histogram
scheduler_task_execution_duration_seconds_bucket{le="0.1"} 0
scheduler_task_execution_duration_seconds_bucket{le="0.5"} 5
scheduler_task_execution_duration_seconds_bucket{le="1.0"} 20
scheduler_task_execution_duration_seconds_bucket{le="5.0"} 100
scheduler_task_execution_duration_seconds_bucket{le="10.0"} 200
scheduler_task_execution_duration_seconds_bucket{le="+Inf"} 250
scheduler_task_execution_duration_seconds_sum 7500
scheduler_task_execution_duration_seconds_count 250
```

### 系统日志

**GET** `/system/logs`

获取系统日志。

#### 查询参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| level | string | 否 | 日志级别 (debug, info, warn, error) |
| service | string | 否 | 服务名称 |
| start_time | string | 否 | 开始时间 (ISO 8601) |
| end_time | string | 否 | 结束时间 (ISO 8601) |
| page | integer | 否 | 页码，默认1 |
| size | integer | 否 | 页大小，默认50 |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "items": [
      {
        "timestamp": "2024-01-01T00:00:00Z",
        "level": "info",
        "service": "scheduler",
        "message": "任务调度完成",
        "metadata": {
          "task_id": 1,
          "worker_id": "worker-001",
          "execution_time": 30.5
        }
      }
    ],
    "total": 1,
    "page": 1,
    "size": 50,
    "total_pages": 1
  }
}
```

## 📊 状态码说明

### HTTP 状态码

| 状态码 | 说明 |
|--------|------|
| 200 | 请求成功 |
| 201 | 创建成功 |
| 400 | 请求参数错误 |
| 401 | 认证失败 |
| 403 | 权限不足 |
| 404 | 资源不存在 |
| 409 | 资源冲突 |
| 422 | 数据验证失败 |
| 429 | 请求频率过高 |
| 500 | 服务器内部错误 |
| 503 | 服务暂时不可用 |

### 错误代码

| 错误代码 | 说明 |
|----------|------|
| VALIDATION_ERROR | 参数验证失败 |
| AUTHENTICATION_ERROR | 认证失败 |
| AUTHORIZATION_ERROR | 权限不足 |
| RESOURCE_NOT_FOUND | 资源不存在 |
| RESOURCE_CONFLICT | 资源冲突 |
| DATABASE_ERROR | 数据库错误 |
| MESSAGE_QUEUE_ERROR | 消息队列错误 |
| WORKER_ERROR | Worker 错误 |
| TASK_ERROR | 任务错误 |
| SYSTEM_ERROR | 系统错误 |

## 🔄 Webhook 事件

系统支持通过 Webhook 发送事件通知。

### 事件类型

| 事件类型 | 说明 |
|----------|------|
| task.created | 任务创建 |
| task.updated | 任务更新 |
| task.deleted | 任务删除 |
| task.triggered | 任务触发 |
| task.completed | 任务完成 |
| task.failed | 任务失败 |
| worker.registered | Worker 注册 |
| worker.unregistered | Worker 注销 |
| worker.heartbeat | Worker 心跳 |
| system.health_check | 系统健康检查 |

### 事件格式

```json
{
  "event": "task.created",
  "timestamp": "2024-01-01T00:00:00Z",
  "data": {
    "task_id": 1,
    "task_name": "data_processing",
    // ... 其他相关数据
  }
}
```

## 📚 使用示例

### cURL 示例

```bash
# 创建任务
curl -X POST http://localhost:8080/api/tasks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '{
    "name": "data_processing",
    "description": "处理每日数据",
    "task_type": "python",
    "executor": "python",
    "command": "process_data.py",
    "schedule": "0 2 * * *",
    "priority": "normal"
  }'

# 获取任务列表
curl -X GET http://localhost:8080/api/tasks \
  -H "Authorization: Bearer your-api-key"

# 获取任务详情
curl -X GET http://localhost:8080/api/tasks/1 \
  -H "Authorization: Bearer your-api-key"

# 触发任务
curl -X POST http://localhost:8080/api/tasks/1/trigger \
  -H "Authorization: Bearer your-api-key"

# 获取系统健康状态
curl -X GET http://localhost:8080/api/system/health \
  -H "Authorization: Bearer your-api-key"
```

### Python 示例

```python
import requests
import json

# API 配置
BASE_URL = "http://localhost:8080/api"
API_KEY = "your-api-key"

headers = {
    "Content-Type": "application/json",
    "Authorization": f"Bearer {API_KEY}"
}

# 创建任务
def create_task():
    task_data = {
        "name": "data_processing",
        "description": "处理每日数据",
        "task_type": "python",
        "executor": "python",
        "command": "process_data.py",
        "schedule": "0 2 * * *",
        "priority": "normal"
    }
    
    response = requests.post(
        f"{BASE_URL}/tasks",
        headers=headers,
        json=task_data
    )
    
    return response.json()

# 获取任务列表
def get_tasks():
    response = requests.get(
        f"{BASE_URL}/tasks",
        headers=headers
    )
    
    return response.json()

# 触发任务
def trigger_task(task_id):
    response = requests.post(
        f"{BASE_URL}/tasks/{task_id}/trigger",
        headers=headers
    )
    
    return response.json()

# 使用示例
if __name__ == "__main__":
    # 创建任务
    result = create_task()
    print(f"创建任务结果: {result}")
    
    # 获取任务列表
    tasks = get_tasks()
    print(f"任务列表: {tasks}")
    
    # 触发任务
    task_id = 1
    result = trigger_task(task_id)
    print(f"触发任务结果: {result}")
```

### JavaScript 示例

```javascript
// API 配置
const BASE_URL = 'http://localhost:8080/api';
const API_KEY = 'your-api-key';

const headers = {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${API_KEY}`
};

// 创建任务
async function createTask() {
    const taskData = {
        name: 'data_processing',
        description: '处理每日数据',
        task_type: 'python',
        executor: 'python',
        command: 'process_data.py',
        schedule: '0 2 * * *',
        priority: 'normal'
    };
    
    const response = await fetch(`${BASE_URL}/tasks`, {
        method: 'POST',
        headers: headers,
        body: JSON.stringify(taskData)
    });
    
    return await response.json();
}

// 获取任务列表
async function getTasks() {
    const response = await fetch(`${BASE_URL}/tasks`, {
        headers: headers
    });
    
    return await response.json();
}

// 触发任务
async function triggerTask(taskId) {
    const response = await fetch(`${BASE_URL}/tasks/${taskId}/trigger`, {
        method: 'POST',
        headers: headers
    });
    
    return await response.json();
}

// 使用示例
(async () => {
    try {
        // 创建任务
        const result = await createTask();
        console.log('创建任务结果:', result);
        
        // 获取任务列表
        const tasks = await getTasks();
        console.log('任务列表:', tasks);
        
        // 触发任务
        const taskId = 1;
        const triggerResult = await triggerTask(taskId);
        console.log('触发任务结果:', triggerResult);
    } catch (error) {
        console.error('API 调用失败:', error);
    }
})();
```

---

本文档提供了分布式任务调度系统 API 的完整说明，包括所有可用的端点、参数、响应格式和使用示例。如需更多帮助，请参考项目文档或联系技术支持。