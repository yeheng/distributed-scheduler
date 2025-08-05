# 系统架构文档

## 🏗️ 总体架构

### 架构概述

分布式任务调度系统采用微服务架构，通过消息队列进行服务间通信，支持水平扩展和高可用部署。

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        分布式任务调度系统                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │   REST API      │  │   Scheduler     │  │   Web UI        │     │
│  │   (HTTP接口)     │  │   (任务调度)     │  │   (管理界面)     │     │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘     │
│           │                     │                     │           │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Message Queue                                 │   │
│  │              (消息队列通信层)                                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│           │                     │                     │           │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │   Worker Pool   │  │   Database      │  │   Monitoring    │     │
│  │   (Worker集群)   │  │   (数据存储)     │  │   (监控告警)     │     │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 核心设计原则

1. **SOLID原则**: 单一职责、开闭原则、里氏替换、接口隔离、依赖倒置
2. **微服务架构**: 服务解耦、独立部署、技术异构性
3. **事件驱动**: 通过消息队列实现异步通信
4. **高可用性**: 故障转移、负载均衡、数据备份
5. **可观测性**: 完整的监控、日志、追踪体系

## 📦 模块架构

### Core模块

**职责**: 提供核心数据模型、服务接口和基础设施

```rust
// 核心数据模型
pub struct Task { /* ... */ }
pub struct TaskRun { /* ... */ }
pub struct WorkerInfo { /* ... */ }

// 服务接口
#[async_trait]
pub trait TaskControlService { /* ... */ }

#[async_trait]
pub trait WorkerServiceTrait { /* ... */ }

// 配置管理
pub struct AppConfig { /* ... */ }

// 错误处理
pub enum SchedulerError { /* ... */ }
```

**依赖关系**:
- 无外部依赖，作为基础模块
- 被其他所有模块依赖

### API模块

**职责**: 提供REST API接口，处理HTTP请求

```rust
// 路由定义
pub fn create_routes(state: AppState) -> Router { /* ... */ }

// 请求处理器
pub struct TaskHandler { /* ... */ }
pub struct WorkerHandler { /* ... */ }

// 中间件
pub fn auth_layer() -> Layer { /* ... */ }
pub fn cors_layer() -> Layer { /* ... */ }
```

**依赖关系**:
- 依赖Core模块
- 依赖Infrastructure模块的数据库实现

### Dispatcher模块

**职责**: 任务调度、依赖管理、定时任务

```rust
// 调度器核心
pub struct TaskScheduler {
    task_repo: Arc<dyn TaskRepository>,
    message_queue: Arc<dyn MessageQueue>,
    // ...
}

// 调度策略
pub trait SchedulingStrategy { /* ... */ }

// 依赖检查
pub struct DependencyChecker { /* ... */ }
```

**依赖关系**:
- 依赖Core模块
- 依赖Infrastructure模块

### Worker模块

**职责**: 任务执行、心跳管理、生命周期

```rust
// Worker服务
pub struct WorkerService {
    task_executor: Arc<dyn TaskExecutor>,
    heartbeat_manager: HeartbeatManager,
    // ...
}

// 任务执行器
pub trait TaskExecutor { /* ... */ }

// 心跳管理
pub struct HeartbeatManager { /* ... */ }
```

**依赖关系**:
- 依赖Core模块
- 依赖Infrastructure模块

### Infrastructure模块

**职责**: 提供数据库、消息队列等基础设施的统一接口

```rust
// 数据库抽象
pub trait DatabaseManager { /* ... */ }

// 消息队列抽象
pub trait MessageQueue { /* ... */ }

// 可观测性
pub struct MetricsCollector { /* ... */ }
```

**依赖关系**:
- 依赖Core模块
- 可能依赖外部库（如PostgreSQL驱动、RabbitMQ客户端等）

### Domain模块

**职责**: 领域模型和业务逻辑

```rust
// 领域实体
pub struct TaskEntity { /* ... */ }
pub struct WorkerEntity { /* ... */ }

// 领域服务
pub struct TaskDomainService { /* ... */ }

// 值对象
pub struct TaskPriority { /* ... */ }
pub struct WorkerStatus { /* ... */ }
```

**依赖关系**:
- 依赖Core模块
- 被业务逻辑模块依赖

## 🔄 数据流架构

### 任务调度流程

```
1. 任务创建
   API → Core → Database

2. 任务调度
   Dispatcher → Database (查询待调度任务)
   Dispatcher → Message Queue (发送任务消息)

3. 任务执行
   Worker → Message Queue (接收任务消息)
   Worker → TaskExecutor (执行任务)
   Worker → Database (更新任务状态)

4. 状态更新
   Worker → Message Queue (发送状态更新)
   Dispatcher → Database (更新任务状态)
   API → Database (查询最新状态)
```

### Worker生命周期

```
1. Worker注册
   Worker → API → Database (注册Worker信息)
   Worker → Message Queue (订阅任务队列)

2. 心跳机制
   Worker → API (定期心跳)
   API → Database (更新Worker状态)
   Dispatcher → Database (检查Worker健康度)

3. 任务分配
   Dispatcher → Database (查询可用Worker)
   Dispatcher → Message Queue (发送任务到指定Worker)

4. 故障处理
   Dispatcher → Database (检测超时Worker)
   Dispatcher → Message Queue (重新分配任务)
```

## 🗄️ 数据架构

### 数据模型关系

```
Task (1) ←→ (N) TaskRun
  ↓
Task (1) ←→ (N) Worker (通过TaskRun关联)
  ↓
Task (1) ←→ (N) Dependency (任务依赖)
```

### 数据库表结构

#### tasks 表
```sql
CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    task_type VARCHAR(100) NOT NULL,
    executor VARCHAR(100) NOT NULL,
    command TEXT NOT NULL,
    arguments JSONB,
    schedule VARCHAR(255),
    priority VARCHAR(20) DEFAULT 'normal',
    status VARCHAR(20) DEFAULT 'created',
    retry_count INTEGER DEFAULT 0,
    timeout_seconds INTEGER DEFAULT 3600,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_run_at TIMESTAMP WITH TIME ZONE,
    next_run_at TIMESTAMP WITH TIME ZONE,
    dependencies JSONB,
    tags JSONB,
    metadata JSONB
);
```

#### task_runs 表
```sql
CREATE TABLE task_runs (
    id BIGSERIAL PRIMARY KEY,
    task_id BIGINT NOT NULL REFERENCES tasks(id),
    worker_id VARCHAR(255),
    status VARCHAR(20) DEFAULT 'pending',
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    result TEXT,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

#### workers 表
```sql
CREATE TABLE workers (
    id VARCHAR(255) PRIMARY KEY,
    hostname VARCHAR(255) NOT NULL,
    ip_address INET NOT NULL,
    port INTEGER NOT NULL,
    task_types JSONB NOT NULL,
    max_concurrent_tasks INTEGER DEFAULT 5,
    current_task_count INTEGER DEFAULT 0,
    status VARCHAR(20) DEFAULT 'registering',
    last_heartbeat TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    registered_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    metadata JSONB
);
```

### 索引设计

```sql
-- 任务查询索引
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_schedule ON tasks(schedule) WHERE schedule IS NOT NULL;
CREATE INDEX idx_tasks_next_run ON tasks(next_run_at) WHERE next_run_at IS NOT NULL;
CREATE INDEX idx_tasks_created_at ON tasks(created_at);

-- 任务运行查询索引
CREATE INDEX idx_task_runs_task_id ON task_runs(task_id);
CREATE INDEX idx_task_runs_status ON task_runs(status);
CREATE INDEX idx_task_runs_worker_id ON task_runs(worker_id);
CREATE INDEX idx_task_runs_created_at ON task_runs(created_at);

-- Worker查询索引
CREATE INDEX idx_workers_status ON workers(status);
CREATE INDEX idx_workers_last_heartbeat ON workers(last_heartbeat);
CREATE INDEX idx_workers_task_types ON workers USING GIN(task_types);
```

## 📡 消息队列架构

### 消息类型

```rust
pub enum MessageType {
    TaskExecution,      // 任务执行
    TaskStatusUpdate,   // 任务状态更新
    WorkerHeartbeat,     // Worker心跳
    WorkerRegistration, // Worker注册
    SystemCommand,      // 系统命令
    HealthCheck,        // 健康检查
}
```

### 消息路由

```
Task Execution Queue:
├── task.python (Python任务队列)
├── task.shell (Shell任务队列)
├── task.http (HTTP任务队列)
└── task.custom (自定义任务队列)

Status Update Queue:
├── status.success (成功状态)
├── status.failed (失败状态)
├── status.running (运行状态)
└── status.completed (完成状态)

Worker Queue:
├── worker.heartbeat (心跳队列)
├── worker.registration (注册队列)
└── worker.command (命令队列)
```

### 消息格式

```rust
pub struct Message {
    pub id: String,                    // 消息唯一标识
    pub message_type: MessageType,      // 消息类型
    pub correlation_id: Option<String>, // 关联ID
    pub payload: Value,                // 消息负载
    pub timestamp: DateTime<Utc>,      // 时间戳
    pub retry_count: u32,              // 重试次数
    pub expires_at: Option<DateTime<Utc>>, // 过期时间
}
```

## 🔒 安全架构

### 认证机制

```rust
// API密钥认证
pub struct ApiKey {
    pub key_id: String,
    pub secret: String,
    pub permissions: Vec<Permission>,
    pub expires_at: Option<DateTime<Utc>>,
}

// JWT令牌认证
pub struct JwtToken {
    pub sub: String,        // 主题
    pub exp: usize,         // 过期时间
    pub permissions: Vec<Permission>,
}
```

### 权限控制

```rust
// 权限定义
pub enum Permission {
    TaskRead,      // 任务读取
    TaskWrite,     // 任务写入
    TaskExecute,   // 任务执行
    WorkerRead,    // Worker读取
    WorkerWrite,   // Worker写入
    SystemRead,    // 系统读取
    SystemWrite,   // 系统写入
}

// 角色定义
pub struct Role {
    pub name: String,
    pub permissions: Vec<Permission>,
}
```

### 数据安全

1. **敏感信息加密**: 配置文件中的密码和密钥支持加密
2. **数据库连接**: 支持SSL/TLS加密连接
3. **消息传输**: 支持消息队列加密通信
4. **审计日志**: 完整的操作审计记录

## 📊 监控架构

### 指标收集

```rust
// 系统指标
pub struct SystemMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_io: NetworkIo,
}

// 应用指标
pub struct ApplicationMetrics {
    pub task_count: u64,
    pub worker_count: u64,
    pub message_queue_size: u64,
    pub database_connections: u32,
}

// 业务指标
pub struct BusinessMetrics {
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub average_execution_time: f64,
    pub worker_utilization: f64,
}
```

### 日志架构

```rust
// 结构化日志
pub struct StructuredLog {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub service: String,
    pub message: String,
    pub metadata: Value,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}
```

### 告警机制

```rust
// 告警规则
pub struct AlertRule {
    pub name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub duration: Duration,
    pub severity: AlertSeverity,
    pub notifications: Vec<NotificationChannel>,
}

// 告警通知
pub enum NotificationChannel {
    Email(EmailConfig),
    Slack(SlackConfig),
    Webhook(WebhookConfig),
}
```

## 🚀 部署架构

### 容器化部署

```dockerfile
# Dockerfile示例
FROM rust:1.70-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/scheduler /usr/local/bin/
CMD ["scheduler"]
```

### Kubernetes部署

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: scheduler
spec:
  replicas: 3
  selector:
    matchLabels:
      app: scheduler
  template:
    metadata:
      labels:
        app: scheduler
    spec:
      containers:
      - name: scheduler
        image: scheduler:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: scheduler-secret
              key: database-url
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

### 高可用部署

```
负载均衡器
    ↓
API集群 (3个实例)
    ↓
消息队列集群 (RabbitMQ镜像队列)
    ↓
调度器集群 (3个实例，主备模式)
    ↓
Worker集群 (N个实例)
    ↓
数据库集群 (PostgreSQL主从复制)
```

## 🔧 性能优化

### 数据库优化

1. **连接池配置**: 合理设置连接池大小
2. **索引优化**: 为常用查询字段创建索引
3. **查询优化**: 使用批量操作和预编译语句
4. **读写分离**: 读操作可以走从库

### 缓存策略

```rust
// 多级缓存
pub struct CacheManager {
    pub l1_cache: Arc<RwLock<LruCache<String, Value>>>, // 内存缓存
    pub l2_cache: Arc<dyn DistributedCache>,             // 分布式缓存
    pub l3_cache: Arc<dyn DatabaseCache>,               // 数据库缓存
}
```

### 并发优化

1. **异步I/O**: 使用Tokio异步运行时
2. **线程池**: 合理配置线程池大小
3. **协程**: 使用async/await进行并发处理
4. **锁优化**: 使用读写锁和原子操作

## 🧪 测试架构

### 测试金字塔

```
      /\
     /  \
    /单元测试\
   /----------\
  /    集成测试   \
 /----------------\
/     端到端测试    \
/------------------\
```

### 测试策略

1. **单元测试**: 测试单个函数和模块
2. **集成测试**: 测试模块间交互
3. **端到端测试**: 测试完整业务流程
4. **性能测试**: 测试系统性能指标
5. **安全测试**: 测试安全漏洞

### 测试工具

```rust
// 测试框架
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_task_creation() {
        // 测试代码
    }
}

// Mock工具
pub struct MockTaskRepository {
    // Mock实现
}

// 测试数据工厂
pub struct TestDataFactory {
    // 测试数据生成
}
```

## 🔄 版本管理和兼容性

### 版本号规范

```
主版本号.次版本号.修订号
例如: 1.2.3

- 主版本号: 不兼容的API变更
- 次版本号: 向下兼容的功能性新增
- 修订号: 向下兼容的问题修正
```

### 向后兼容性

1. **API兼容**: 新版本保持API兼容
2. **数据兼容**: 数据库schema变更保持兼容
3. **配置兼容**: 配置文件格式保持兼容
4. **二进制兼容**: 库文件保持兼容

### 迁移策略

1. **蓝绿部署**: 零停机时间迁移
2. **灰度发布**: 逐步发布新版本
3. **回滚机制**: 快速回滚到上一版本
4. **数据迁移**: 自动化数据迁移脚本

## 📈 扩展性设计

### 水平扩展

1. **无状态服务**: API和Worker服务无状态化
2. **数据分片**: 数据库和消息队列支持分片
3. **负载均衡**: 使用负载均衡器分发请求
4. **自动扩展**: 基于负载自动增减实例

### 垂直扩展

1. **资源优化**: 优化CPU和内存使用
2. **缓存优化**: 多级缓存策略
3. **连接池优化**: 合理配置连接池
4. **算法优化**: 优化核心算法性能

### 功能扩展

1. **插件系统**: 支持自定义插件
2. **脚本支持**: 支持Lua等脚本语言
3. **WebHook**: 支持事件通知
4. **API扩展**: 支持自定义API端点

---

本文档描述了分布式任务调度系统的完整架构设计，包括模块架构、数据流、部署策略等方面。系统采用现代化架构设计，具有良好的扩展性、可用性和可维护性。