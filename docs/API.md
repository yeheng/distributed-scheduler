# API 参考

## 📋 API 概述

REST API接口文档，提供任务管理、Worker管理、系统监控等功能。

**基础信息**
- **Base URL**: `http://localhost:8080/api`  
- **数据格式**: JSON
- **认证方式**: API Key 或 JWT Token

**统一响应格式**
```json
{
  "success": true,
  "data": { /* 响应数据 */ },
  "message": "操作信息",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

## 🔐 认证

### API Key 认证
```http
Authorization: Bearer your-api-key
```

### JWT Token 认证  
```http
Authorization: Bearer your-jwt-token
```

## 📋 任务管理

### 创建任务
**POST** `/api/tasks`

```json
{
  "name": "task_name",
  "executor": "shell|http|python",
  "command": "echo hello",
  "schedule": "0 */6 * * *",    // 可选：Cron表达式
  "priority": "normal",         // low|normal|high|critical
  "retry_count": 3,
  "timeout_seconds": 3600
}
```

### 获取任务列表
**GET** `/api/tasks`

查询参数：
- `page`: 页码 (默认1)
- `size`: 页大小 (默认20) 
- `status`: 状态过滤
- `search`: 搜索关键词

### 获取任务详情
**GET** `/api/tasks/{task_id}`

### 更新任务
**PUT** `/api/tasks/{task_id}`

```json
{
  "name": "updated_name",
  "status": "active|paused|disabled",
  "schedule": "0 12 * * *"
}
```

### 删除任务
**DELETE** `/api/tasks/{task_id}`

### 手动触发任务
**POST** `/api/tasks/{task_id}/trigger`

### 获取任务运行记录
**GET** `/api/tasks/{task_id}/runs`

## 👷 Worker管理

### 获取Worker列表  
**GET** `/api/workers`

响应：
```json
{
  "success": true,
  "data": [
    {
      "id": "worker-001",
      "name": "worker-node-1", 
      "status": "online|offline|busy",
      "last_heartbeat": "2024-01-01T00:00:00Z",
      "current_tasks": 2,
      "max_tasks": 10,
      "tags": ["gpu", "python"]
    }
  ]
}
```

### 获取Worker详情
**GET** `/api/workers/{worker_id}`

### 更新Worker配置
**PUT** `/api/workers/{worker_id}`

```json
{
  "max_tasks": 20,
  "tags": ["cpu", "shell"],
  "enabled": true
}
```

### Worker心跳上报
**POST** `/api/workers/{worker_id}/heartbeat`

```json
{
  "status": "online",
  "current_tasks": 3,
  "system_info": {
    "cpu_usage": 45.2,
    "memory_usage": 60.8,
    "disk_usage": 35.1
  }
}
```

## 📊 系统监控

### 系统健康检查
**GET** `/api/health`

响应：
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "database": "connected",
    "message_queue": "connected", 
    "uptime_seconds": 86400,
    "version": "1.0.0"
  }
}
```

### 系统状态概览
**GET** `/api/system/status`

响应：
```json
{
  "success": true,
  "data": {
    "tasks": {
      "total": 150,
      "active": 120,
      "paused": 20,
      "running": 10
    },
    "workers": {
      "total": 5,
      "online": 4,
      "offline": 1
    },
    "runs_today": 450,
    "success_rate": 98.5
  }
}
```

### Prometheus指标
**GET** `/metrics`

返回Prometheus格式的指标数据。

## 🔍 任务执行器

### Shell执行器
```json
{
  "executor": "shell",
  "command": "ls -la",
  "working_directory": "/tmp",
  "environment": {
    "PATH": "/usr/bin:/bin"
  }
}
```

### HTTP执行器  
```json
{
  "executor": "http",
  "url": "https://api.example.com/webhook",
  "method": "POST", 
  "headers": {
    "Content-Type": "application/json"
  },
  "body": "{\"data\": \"value\"}"
}
```

### Python执行器
```json
{
  "executor": "python",
  "command": "data_processor.py",
  "arguments": ["--input", "data.csv"],
  "python_path": "/usr/bin/python3"
}
```

## 📖 状态码参考

| 状态码 | 含义 | 说明 |
|--------|------|------|
| 200 | OK | 请求成功 |
| 201 | Created | 资源创建成功 |
| 204 | No Content | 请求成功，无返回内容 |
| 400 | Bad Request | 请求参数错误 |
| 401 | Unauthorized | 认证失败 |
| 403 | Forbidden | 权限不足 |
| 404 | Not Found | 资源未找到 |
| 409 | Conflict | 资源冲突 |
| 500 | Internal Server Error | 服务器内部错误 |

## 📝 使用示例

### 完整任务创建示例

```bash
# 创建定时备份任务
curl -X POST http://localhost:8080/api/tasks \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "database_backup",
    "executor": "shell", 
    "command": "pg_dump mydb > /backup/$(date +%Y%m%d).sql",
    "schedule": "0 2 * * *",
    "priority": "high",
    "retry_count": 2,
    "timeout_seconds": 1800
  }'

# 创建HTTP webhook任务  
curl -X POST http://localhost:8080/api/tasks \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "status_webhook",
    "executor": "http",
    "url": "https://hooks.slack.com/services/xxx",
    "method": "POST",
    "headers": {"Content-Type": "application/json"},
    "body": "{\"text\": \"Daily report completed\"}"
  }'

# 手动触发任务执行
curl -X POST http://localhost:8080/api/tasks/1/trigger \
  -H "Authorization: Bearer your-api-key"

# 查看任务执行结果
curl -X GET http://localhost:8080/api/tasks/1/runs \
  -H "Authorization: Bearer your-api-key"
```