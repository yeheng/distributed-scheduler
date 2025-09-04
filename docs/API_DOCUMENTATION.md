# 分布式任务调度系统 API 文档

## 概览

本文档描述了分布式任务调度系统的 REST API 接口。系统提供任务管理、工作节点管理、认证授权等功能。

**Base URL:** `http://localhost:8080/api` (开发环境)

## 认证

系统支持两种认证方式：

### 1. JWT Token 认证
- 通过 `/auth/login` 获取访问令牌
- 在请求头中添加：`Authorization: Bearer <access_token>`
- 访问令牌会过期，可通过刷新令牌获取新的访问令牌

### 2. API Key 认证
- 通过 `/auth/api-keys` 创建 API 密钥
- 在请求头中添加：`Authorization: Bearer <api_key>`

### 用户角色和权限

| 角色 | 用户名 | 密码 | 权限 |
|-----|--------|------|------|
| 管理员 | admin | admin123 | Admin（所有权限） |
| 操作员 | operator | op123 | TaskRead, TaskWrite, WorkerRead, SystemRead |
| 查看者 | viewer | view123 | TaskRead, WorkerRead, SystemRead |

## 认证 API

### 用户登录
```http
POST /auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "admin123"
}
```

**响应:**
```json
{
  "success": true,
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "token_type": "Bearer",
    "expires_in": 3600,
    "user": {
      "id": "123e4567-e89b-12d3-a456-426614174000",
      "username": "admin",
      "email": "admin@example.com",
      "role": "Admin",
      "permissions": ["Admin"]
    }
  }
}
```

### 刷新令牌
```http
POST /auth/refresh
Content-Type: application/json

{
  "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGc..."
}
```

### 创建 API 密钥
```http
POST /auth/api-keys
Authorization: Bearer <admin_token>
Content-Type: application/json

{
  "name": "My API Key",
  "permissions": ["TaskRead", "TaskWrite"],
  "expires_in_days": 30
}
```

**响应:**
```json
{
  "success": true,
  "data": {
    "api_key": "sk-1234567890abcdef",
    "name": "My API Key", 
    "permissions": ["TaskRead", "TaskWrite"],
    "created_at": "2024-01-01T00:00:00Z",
    "expires_at": "2024-01-31T00:00:00Z"
  }
}
```

### 验证令牌
```http
GET /auth/validate
Authorization: Bearer <token>
```

### 用户退出
```http
POST /auth/logout
Authorization: Bearer <token>
```

## 任务管理 API

### 创建任务
```http
POST /api/tasks
Authorization: Bearer <token>
Content-Type: application/json

{
  "name": "数据备份任务",
  "task_type": "backup",
  "schedule": "0 2 * * *",
  "parameters": {
    "source": "/data",
    "destination": "/backup"
  },
  "timeout_seconds": 3600,
  "max_retries": 3,
  "dependencies": [1, 2]
}
```

**字段说明:**
- `name` (必需): 任务名称，1-255字符
- `task_type` (必需): 任务类型，1-100字符
- `schedule` (必需): Cron 调度表达式，1-255字符
- `parameters` (必需): 任务参数 JSON 对象
- `timeout_seconds` (可选): 超时时间，1-86400秒，默认3600
- `max_retries` (可选): 最大重试次数，0-10次，默认3
- `dependencies` (可选): 依赖任务ID数组

**响应:**
```json
{
  "success": true,
  "data": {
    "id": 1,
    "name": "数据备份任务",
    "task_type": "backup",
    "schedule": "0 2 * * *",
    "parameters": {
      "source": "/data",
      "destination": "/backup"
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

### 获取任务列表
```http
GET /api/tasks?page=1&page_size=20&task_type=backup&status=ACTIVE&name=数据
Authorization: Bearer <token>
```

**查询参数:**
- `page` (可选): 页码，默认1，范围1-1000
- `page_size` (可选): 每页大小，默认20，范围1-100
- `task_type` (可选): 任务类型过滤，最大100字符
- `status` (可选): 任务状态过滤 (ACTIVE, INACTIVE)
- `name` (可选): 任务名称模糊匹配，最大255字符

**响应:**
```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": 1,
        "name": "数据备份任务",
        "task_type": "backup",
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
    "pagination": {
      "total": 50,
      "page": 1,
      "page_size": 20,
      "total_pages": 3
    }
  }
}
```

### 获取任务详情
```http
GET /api/tasks/{id}
Authorization: Bearer <token>
```

**响应包含额外字段:**
```json
{
  "success": true,
  "data": {
    "id": 1,
    "name": "数据备份任务",
    "task_type": "backup",
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
        "id": 101,
        "task_id": 1,
        "status": "Completed",
        "worker_id": "worker-001",
        "retry_count": 0,
        "scheduled_at": "2024-01-01T02:00:00Z",
        "started_at": "2024-01-01T02:00:05Z",
        "completed_at": "2024-01-01T02:30:00Z",
        "result": "Backup completed successfully",
        "error_message": null,
        "execution_duration_ms": 1795000,
        "created_at": "2024-01-01T02:00:00Z"
      }
    ],
    "execution_stats": {
      "task_id": 1,
      "total_runs": 30,
      "successful_runs": 28,
      "failed_runs": 2,
      "timeout_runs": 0,
      "average_execution_time_ms": 1800000,
      "success_rate": 93.33,
      "last_execution": "2024-01-01T02:00:00Z"
    }
  }
}
```

### 更新任务 (PUT)
```http
PUT /api/tasks/{id}
Authorization: Bearer <token>
Content-Type: application/json

{
  "name": "更新的任务名",
  "schedule": "0 3 * * *",
  "parameters": {...},
  "timeout_seconds": 7200,
  "max_retries": 5,
  "dependencies": [],
  "status": "Inactive"
}
```

### 部分更新任务 (PATCH)
```http
PATCH /api/tasks/{id}
Authorization: Bearer <token>
Content-Type: application/json

{
  "name": "新任务名",
  "timeout_seconds": 7200
}
```

### 删除任务
```http
DELETE /api/tasks/{id}
Authorization: Bearer <token>
```

**注意:** 删除操作是软删除，将任务状态设置为 Inactive。

### 手动触发任务
```http
POST /api/tasks/{id}/trigger
Authorization: Bearer <token>
```

**响应:**
```json
{
  "success": true,
  "data": {
    "id": 201,
    "task_id": 1,
    "status": "Dispatched",
    "worker_id": null,
    "retry_count": 0,
    "scheduled_at": "2024-01-01T10:00:00Z",
    "started_at": null,
    "completed_at": null,
    "result": null,
    "error_message": null,
    "execution_duration_ms": null,
    "created_at": "2024-01-01T10:00:00Z"
  }
}
```

### 获取任务执行记录
```http
GET /api/tasks/{task_id}/runs?page=1&page_size=10&status=COMPLETED
Authorization: Bearer <token>
```

**查询参数:**
- `page` (可选): 页码，默认1，范围1-1000
- `page_size` (可选): 每页大小，默认20，范围1-100
- `status` (可选): 执行状态过滤 (PENDING, DISPATCHED, RUNNING, COMPLETED, FAILED, TIMEOUT)

### 获取任务执行统计
```http
GET /api/tasks/{id}/stats?days=30
Authorization: Bearer <token>
```

**查询参数:**
- `days` (可选): 统计天数，默认30，范围1-365

**响应:**
```json
{
  "success": true,
  "data": {
    "task_id": 1,
    "total_runs": 30,
    "successful_runs": 28,
    "failed_runs": 2,
    "timeout_runs": 0,
    "average_execution_time_ms": 1800000,
    "success_rate": 93.33,
    "last_execution": "2024-01-01T02:00:00Z"
  }
}
```

## 工作节点管理 API

### 获取工作节点列表
```http
GET /api/workers?page=1&page_size=20&status=ALIVE
Authorization: Bearer <token>
```

**查询参数:**
- `page` (可选): 页码，默认1
- `page_size` (可选): 每页大小，默认20
- `status` (可选): 工作节点状态过滤 (ALIVE, DOWN)

**响应:**
```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": "worker-001",
        "hostname": "worker-server-01",
        "ip_address": "192.168.1.10",
        "supported_task_types": ["backup", "cleanup", "report"],
        "max_concurrent_tasks": 10,
        "current_task_count": 3,
        "status": "Alive",
        "load_percentage": 30.0,
        "last_heartbeat": "2024-01-01T10:00:00Z",
        "registered_at": "2024-01-01T00:00:00Z"
      }
    ],
    "pagination": {
      "total": 5,
      "page": 1,
      "page_size": 20,
      "total_pages": 1
    }
  }
}
```

### 获取工作节点详情
```http
GET /api/workers/{worker_id}
Authorization: Bearer <token>
```

### 获取工作节点负载统计
```http
GET /api/workers/{worker_id}/stats
Authorization: Bearer <token>
```

**响应:**
```json
{
  "success": true,
  "data": {
    "worker_id": "worker-001",
    "current_task_count": 3,
    "max_concurrent_tasks": 10,
    "load_percentage": 30.0,
    "total_completed_tasks": 1250,
    "total_failed_tasks": 15,
    "average_task_duration_ms": 2500000,
    "last_heartbeat": "2024-01-01T10:00:00Z"
  }
}
```

## 系统管理 API

### 系统健康检查
```http
GET /health
```

**响应:**
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "timestamp": "2024-01-01T10:00:00Z",
    "services": {
      "database": "healthy",
      "message_queue": "healthy",
      "cache": "healthy"
    },
    "uptime": 86400,
    "version": "1.0.0"
  }
}
```

### 系统指标
```http
GET /metrics
```

返回 Prometheus 格式的监控指标。

## 错误处理

### 错误响应格式
```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "输入验证失败",
    "details": {
      "field": "name",
      "reason": "任务名称长度必须在1-255个字符之间"
    }
  },
  "timestamp": "2024-01-01T10:00:00Z"
}
```

### HTTP 状态码

| 状态码 | 含义 | 错误类型 |
|--------|------|----------|
| 200 | 成功 | - |
| 201 | 创建成功 | - |
| 400 | 请求错误 | VALIDATION_ERROR, BAD_REQUEST |
| 401 | 未认证 | AUTHENTICATION_ERROR |
| 403 | 权限不足 | AUTHORIZATION_ERROR |
| 404 | 资源不存在 | NOT_FOUND |
| 409 | 资源冲突 | CONFLICT |
| 422 | 验证失败 | VALIDATION_ERROR |
| 500 | 服务器内部错误 | INTERNAL_ERROR |

### 常见错误类型

#### 验证错误
```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "输入验证失败",
    "details": [
      {
        "field": "name",
        "code": "length",
        "message": "任务名称长度必须在1-255个字符之间"
      }
    ]
  }
}
```

#### 权限错误
```json
{
  "success": false,
  "error": {
    "code": "AUTHORIZATION_ERROR", 
    "message": "权限不足，需要 TaskWrite 权限"
  }
}
```

#### 资源冲突
```json
{
  "success": false,
  "error": {
    "code": "CONFLICT",
    "message": "任务名称 '数据备份任务' 已存在"
  }
}
```

## 数据模型

### Task (任务)
```typescript
interface Task {
  id: number;
  name: string;               // 任务名称
  task_type: string;          // 任务类型
  schedule: string;           // Cron 调度表达式
  parameters: object;         // 任务参数
  timeout_seconds: number;    // 超时时间（秒）
  max_retries: number;        // 最大重试次数
  status: "Active" | "Inactive"; // 任务状态
  dependencies: number[];     // 依赖任务ID列表
  created_at: string;         // 创建时间 (ISO 8601)
  updated_at: string;         // 更新时间 (ISO 8601)
}
```

### TaskRun (任务执行记录)
```typescript
interface TaskRun {
  id: number;
  task_id: number;
  status: "Pending" | "Dispatched" | "Running" | "Completed" | "Failed" | "Timeout";
  worker_id?: string;         // 执行工作节点ID
  retry_count: number;        // 重试次数
  scheduled_at: string;       // 计划执行时间
  started_at?: string;        // 实际开始时间
  completed_at?: string;      // 完成时间
  result?: string;            // 执行结果
  error_message?: string;     // 错误信息
  execution_duration_ms?: number; // 执行时长（毫秒）
  created_at: string;         // 创建时间
}
```

### WorkerInfo (工作节点信息)
```typescript
interface WorkerInfo {
  id: string;                 // 工作节点ID
  hostname: string;           // 主机名
  ip_address: string;         // IP地址
  supported_task_types: string[]; // 支持的任务类型
  max_concurrent_tasks: number;   // 最大并发任务数
  current_task_count: number;     // 当前任务数
  status: "Alive" | "Down";       // 节点状态
  load_percentage: number;        // 负载百分比
  last_heartbeat: string;         // 最后心跳时间
  registered_at: string;          // 注册时间
}
```

### TaskExecutionStats (任务执行统计)
```typescript
interface TaskExecutionStats {
  task_id: number;
  total_runs: number;         // 总执行次数
  successful_runs: number;    // 成功次数
  failed_runs: number;        // 失败次数
  timeout_runs: number;       // 超时次数
  average_execution_time_ms?: number; // 平均执行时间
  success_rate: number;       // 成功率（百分比）
  last_execution?: string;    // 最后执行时间
}
```

## API 客户端示例

### JavaScript/Node.js
```javascript
const API_BASE_URL = 'http://localhost:8080';

class SchedulerAPI {
  constructor(token) {
    this.token = token;
    this.headers = {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`
    };
  }

  async createTask(task) {
    const response = await fetch(`${API_BASE_URL}/api/tasks`, {
      method: 'POST',
      headers: this.headers,
      body: JSON.stringify(task)
    });
    return response.json();
  }

  async listTasks(params = {}) {
    const query = new URLSearchParams(params);
    const response = await fetch(`${API_BASE_URL}/api/tasks?${query}`, {
      headers: this.headers
    });
    return response.json();
  }

  async triggerTask(taskId) {
    const response = await fetch(`${API_BASE_URL}/api/tasks/${taskId}/trigger`, {
      method: 'POST',
      headers: this.headers
    });
    return response.json();
  }
}

// 使用示例
const api = new SchedulerAPI('your-jwt-token');
const tasks = await api.listTasks({ status: 'ACTIVE' });
```

### Python
```python
import requests
import json

class SchedulerAPI:
    def __init__(self, base_url='http://localhost:8080', token=None):
        self.base_url = base_url
        self.headers = {
            'Content-Type': 'application/json'
        }
        if token:
            self.headers['Authorization'] = f'Bearer {token}'
    
    def login(self, username, password):
        response = requests.post(f'{self.base_url}/auth/login', 
                               json={'username': username, 'password': password})
        if response.status_code == 200:
            data = response.json()['data']
            self.headers['Authorization'] = f"Bearer {data['access_token']}"
            return data
        raise Exception(f"Login failed: {response.text}")
    
    def create_task(self, task_data):
        response = requests.post(f'{self.base_url}/api/tasks',
                               headers=self.headers, json=task_data)
        return response.json()
    
    def list_tasks(self, **params):
        response = requests.get(f'{self.base_url}/api/tasks',
                              headers=self.headers, params=params)
        return response.json()
    
    def trigger_task(self, task_id):
        response = requests.post(f'{self.base_url}/api/tasks/{task_id}/trigger',
                               headers=self.headers)
        return response.json()

# 使用示例
api = SchedulerAPI()
api.login('admin', 'admin123')

# 创建任务
task = {
    'name': '测试任务',
    'task_type': 'test',
    'schedule': '0 */5 * * *',
    'parameters': {'test_param': 'value'}
}
result = api.create_task(task)
```

### cURL 示例
```bash
# 登录获取令牌
curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}'

# 创建任务
curl -X POST http://localhost:8080/api/tasks \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "测试任务",
    "task_type": "test", 
    "schedule": "0 */5 * * *",
    "parameters": {"key": "value"}
  }'

# 获取任务列表
curl -X GET "http://localhost:8080/api/tasks?status=ACTIVE" \
  -H "Authorization: Bearer YOUR_TOKEN"

# 触发任务执行
curl -X POST http://localhost:8080/api/tasks/1/trigger \
  -H "Authorization: Bearer YOUR_TOKEN"
```

## 最佳实践

1. **认证令牌管理**
   - 安全存储访问令牌和刷新令牌
   - 在令牌过期前主动刷新
   - 实现自动重试机制处理 401 错误

2. **错误处理**
   - 检查 `success` 字段确定请求是否成功
   - 根据错误代码实现相应的错误处理逻辑
   - 对于网络错误实现重试机制

3. **分页处理**
   - 合理设置 `page_size` 避免单次请求数据过多
   - 实现分页导航功能
   - 缓存列表数据提升用户体验

4. **任务调度**
   - 使用标准 Cron 表达式格式
   - 合理设置超时时间和重试次数
   - 考虑任务依赖关系避免循环依赖

5. **监控和日志**
   - 定期检查系统健康状态
   - 监控任务执行统计和工作节点负载
   - 实现告警机制处理异常情况