# 系统架构文档

## 🏗️ 总体架构

### 架构概述

分布式任务调度系统采用微服务架构，通过消息队列进行服务间通信，支持水平扩展和高可用部署。系统基于领域驱动设计(DDD)和清洁架构原则，实现了高度模块化和可扩展的架构。

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        分布式任务调度系统                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │   REST API      │  │   Scheduler     │  │   Observability │     │
│  │   (HTTP接口)     │  │   (任务调度)     │  │   (监控观察)     │     │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘     │
│           │                     │                     │           │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Message Queue                                 │   │
│  │              (消息队列通信层)                                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│           │                     │                     │           │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │   Worker Pool   │  │   Infrastructure│  │   Config        │     │
│  │   (Worker集群)   │  │   (基础设施)     │  │   (配置管理)     │     │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 核心设计原则

1. **SOLID原则**: 单一职责、开闭原则、里氏替换、接口隔离、依赖倒置
2. **微服务架构**: 服务解耦、独立部署、技术异构性
3. **领域驱动设计**: 明确的领域边界和业务抽象
4. **清洁架构**: 依赖倒置和层次分离
5. **事件驱动**: 通过消息队列实现异步通信
6. **高可用性**: 故障转移、负载均衡、数据备份
7. **可观测性**: 完整的监控、日志、追踪体系

## 📦 Crate架构

系统采用Rust workspace管理多个crate，每个crate负责特定的职责，严格遵循依赖层次：

### 新架构Crate依赖图

```
                    ┌─────────────────────┐
                    │   scheduler (bin)   │
                    └──────────┬──────────┘
                               │
           ┌───────────────────────────────────────────────┐
           │                                               │
    ┌─────────────────┐                          ┌─────────────────┐
    │ scheduler-api   │                          │scheduler-worker │
    └─────────┬───────┘                          └─────────┬───────┘
              │                                            │
    ┌─────────────────┐                          ┌─────────────────┐
    │scheduler-        │                          │ scheduler-      │
    │dispatcher       │                          │ application     │
    └─────────┬───────┘                          └─────────┬───────┘
              │                                            │
              │         ┌─────────────────┐                │
              └─────────┤scheduler-       ├────────────────┘
                        │infrastructure  │
                        └─────────┬───────┘
                                  │
                        ┌─────────────────┐
                        │scheduler-       │
                        │foundation       │
                        └─────────┬───────┘
                                  │
          ┌───────────────────────┼───────────────────────┐
          │                       │                       │
    ┌─────────────┐      ┌─────────────┐      ┌─────────────────┐
    │scheduler-   │      │scheduler-   │      │scheduler-       │
    │domain       │      │config       │      │observability    │
    └─────┬───────┘      └─────────────┘      └─────────────────┘
          │
    ┌─────────────┐
    │scheduler-   │
    │errors       │
    └─────────────┘
```

### 1. scheduler-foundation (基础设施层)

**职责**: 提供核心抽象、服务接口、依赖注入容器和基础构建块

```rust
// 依赖注入容器
pub struct Container { /* ... */ }

// 执行器注册表
pub struct DefaultExecutorRegistry { /* ... */ }
pub trait ExecutorRegistry { /* ... */ }

// 核心trait定义
pub trait TaskExecutor { /* ... */ }
pub trait Scheduler { /* ... */ }
pub trait MessageQueue { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-errors, scheduler-domain
- 作为基础设施层，为上层服务提供抽象接口
- 被 application、infrastructure、api、dispatcher、worker 模块依赖

### 2. scheduler-errors (错误处理层)

**职责**: 统一错误类型定义和处理

```rust
// 统一错误类型
pub enum SchedulerError { /* ... */ }
pub type SchedulerResult<T> = Result<T, SchedulerError>;
```

**依赖关系**:
- 无外部依赖，最底层crate
- 被其他所有模块依赖

### 3. scheduler-domain (领域层)

**职责**: 包含业务实体、领域服务、仓储接口和业务逻辑

```rust
// 领域实体
pub struct Task { /* ... */ }
pub struct TaskRun { /* ... */ }
pub struct WorkerInfo { /* ... */ }

// 领域服务
pub struct TaskDependencyService { /* ... */ }

// 仓储接口
pub trait TaskRepository { /* ... */ }
pub trait TaskRunRepository { /* ... */ }
pub trait WorkerRepository { /* ... */ }

// 查询构建器
pub struct TaskQueryBuilder { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-errors
- 被 foundation 重导出，供其他模块使用

### 4. scheduler-config (配置管理层)

**职责**: 统一配置管理、环境变量处理和配置验证

```rust
// 配置管理
pub struct AppConfig { /* ... */ }
pub struct DatabaseConfig { /* ... */ }
pub struct MessageQueueConfig { /* ... */ }

// 环境处理
pub struct Environment { /* ... */ }

// 熔断器配置
pub struct CircuitBreakerConfig { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-errors
- 被 foundation、infrastructure 等模块依赖

### 5. scheduler-observability (可观测性层)

**职责**: 监控、日志、追踪和指标收集

```rust
// 结构化日志
pub struct StructuredLogger { /* ... */ }

// 分布式追踪
pub struct CrossComponentTracer { /* ... */ }
pub trait MessageTracingExt { /* ... */ }

// 指标收集
pub struct MetricsCollector { /* ... */ }

// 遥测设置
pub struct TelemetrySetup { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-foundation, scheduler-domain
- 提供跨组件的监控和追踪能力

### 6. scheduler-application (应用层)

**职责**: 应用服务实现，协调领域对象完成业务用例

```rust
// 应用服务接口 (已移至 interfaces/)
pub trait TaskControlService { /* ... */ }
pub trait WorkerServiceTrait { /* ... */ }
pub trait SchedulerService { /* ... */ }

// 应用服务实现
pub struct TaskService { /* ... */ }
pub struct WorkerService { /* ... */ }
pub struct SchedulerServiceImpl { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-foundation, scheduler-domain, scheduler-errors
- 被 api、dispatcher、worker 模块依赖

### 7. scheduler-infrastructure (基础设施层)

**职责**: 提供数据库、消息队列、缓存等基础设施的具体实现

```rust
// 数据库实现
pub struct PostgreSQLTaskRepository { /* ... */ }
pub struct SQLiteTaskRepository { /* ... */ }

// 消息队列实现
pub struct RabbitMQMessageQueue { /* ... */ }
pub struct RedisStreamMessageQueue { /* ... */ }

// 缓存实现
pub struct RedisCacheManager { /* ... */ }

// 熔断器实现
pub struct CircuitBreakerWrapper { /* ... */ }

// 超时处理
pub struct TimeoutHandler { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-foundation, scheduler-domain, scheduler-config
- 实现 foundation 中定义的抽象接口
- 可能依赖外部库（PostgreSQL、Redis、RabbitMQ等）

### 8. scheduler-api (API接口层)

**职责**: 提供REST API接口，处理HTTP请求

```rust
// 路由定义
pub fn create_routes(state: AppState) -> Router { /* ... */ }

// 请求处理器
pub struct TaskHandler { /* ... */ }
pub struct WorkerHandler { /* ... */ }
pub struct AuthHandler { /* ... */ }

// 中间件
pub fn auth_layer() -> Layer { /* ... */ }
pub fn rate_limit_layer() -> Layer { /* ... */ }
pub fn cors_layer() -> Layer { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-foundation, scheduler-application, scheduler-domain, scheduler-infrastructure
- 作为系统的对外接口

### 9. scheduler-dispatcher (任务调度层)

**职责**: 任务调度、依赖管理、定时任务、故障恢复

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

// 恢复服务
pub struct RecoveryService { /* ... */ }

// 重试服务
pub struct RetryService { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-foundation, scheduler-application, scheduler-domain, scheduler-infrastructure
- 核心业务逻辑实现

### 10. scheduler-worker (任务执行层)

**职责**: 任务执行、心跳管理、生命周期管理

```rust
// Worker服务
pub struct WorkerService {
    executor_registry: Arc<dyn ExecutorRegistry>,
    heartbeat_manager: HeartbeatManager,
    // ...
}

// 任务执行器
pub struct ShellExecutor { /* ... */ }
pub struct HttpExecutor { /* ... */ }

// 执行器工厂
pub struct ExecutorFactory { /* ... */ }

// 心跳管理
pub struct HeartbeatManager { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-foundation, scheduler-application, scheduler-domain, scheduler-infrastructure
- 实现具体的任务执行逻辑

### 11. scheduler-testing-utils (测试工具层)

**职责**: 提供测试工具和模拟实现

```rust
// 测试数据构建器
pub struct TaskBuilder { /* ... */ }
pub struct TaskRunBuilder { /* ... */ }
pub struct WorkerInfoBuilder { /* ... */ }
pub struct MessageBuilder { /* ... */ }

// Mock 实现
pub struct MockTaskRepository { /* ... */ }
pub struct MockTaskRunRepository { /* ... */ }
pub struct MockWorkerRepository { /* ... */ }

// 测试容器
pub struct TestContainers { /* ... */ }
```

**依赖关系**:
- 依赖: scheduler-domain, scheduler-errors
- 被所有测试模块依赖

## 🔄 架构改进亮点

### 1. 清洁架构实现

系统严格遵循清洁架构原则：

- **依赖倒置**: 高层模块不依赖低层模块，都依赖于抽象
- **关注点分离**: 每个crate都有明确的职责边界
- **可测试性**: 通过依赖注入实现高度可测试的代码

### 2. 配置管理分离

- 将配置逻辑从 core 迁移到独立的 `scheduler-config` crate
- 支持环境变量、TOML配置文件、运行时配置
- 提供配置验证和默认值处理

### 3. 可观测性增强

- 独立的 `scheduler-observability` crate
- 分布式追踪支持，包括消息队列上下文传播
- 结构化日志和指标收集
- OpenTelemetry集成

### 4. 基础设施抽象

- `scheduler-foundation` 提供核心抽象和接口
- 依赖注入容器管理服务生命周期
- 执行器注册表支持动态执行器管理

### 6. 新增功能特性

- **多层缓存系统**: Redis基础的分布式缓存，支持任务定义和Worker状态缓存
- **熔断器模式**: 对数据库和消息队列调用实现熔断保护
- **速率限制**: 基于令牌桶算法的API速率限制中间件
- **认证授权**: JWT和API密钥认证，基于角色的权限控制
- **资源清理**: 任务执行器的完善资源管理和清理机制

### 7. 性能优化实现

- **并行任务扫描**: 使用 `futures::stream` 实现可配置并发度的任务处理
- **数据库查询优化**: 消除N+1查询模式，使用JOIN和批量加载
- **超时处理**: 为所有外部调用添加 `tokio::time::timeout` 包装
- **连接池优化**: 合理配置数据库和消息队列连接池

## 🎯 架构演进对比

### 重构前架构问题

```
┌──────────────────────────────────────────────────────────────┐
│                     旧架构问题分析                            │
├──────────────────────────────────────────────────────────────┤
│ 1. Core crate职责不清，混杂了配置、日志、基础设施           │
│ 2. 应用服务接口位置错误，违反依赖倒置原则                   │
│ 3. 可观测性代码分散，缺乏统一管理                           │
│ 4. 缺乏专门的配置管理层                                     │
│ 5. 基础设施实现与抽象耦合                                   │
└──────────────────────────────────────────────────────────────┘
```

### 重构后架构优势

```
┌──────────────────────────────────────────────────────────────┐
│                     新架构优势分析                            │
├──────────────────────────────────────────────────────────────┤
│ ✅ 清晰的依赖层次和职责划分                                 │
│ ✅ 严格遵循清洁架构和DDD原则                               │
│ ✅ 每个crate都有明确单一的职责                             │
│ ✅ 支持独立测试和部署                                       │
│ ✅ 便于维护和扩展                                           │
│ ✅ 完善的可观测性和监控体系                                 │
│ ✅ 企业级安全和性能特性                                     │
└──────────────────────────────────────────────────────────────┘
```

## 🔄 数据流架构

### 任务调度流程

```
1. 任务创建
   API → Application Service → Domain → Infrastructure → Database
   scheduler-api → scheduler-application → scheduler-domain → scheduler-infrastructure

2. 任务调度
   Dispatcher → Infrastructure → Database (查询待调度任务)
   Dispatcher → Infrastructure → Message Queue (发送任务消息)
   scheduler-dispatcher → scheduler-infrastructure

3. 任务执行
   Worker → Infrastructure → Message Queue (接收任务消息)
   Worker → TaskExecutor (执行任务)
   Worker → Infrastructure → Database (更新任务状态)
   scheduler-worker → scheduler-infrastructure

4. 状态更新与追踪
   Worker → Observability → Tracing (分布式追踪)
   Worker → Infrastructure → Message Queue (发送状态更新)
   Dispatcher → Infrastructure → Database (更新任务状态)
   API → Infrastructure → Database (查询最新状态)
   scheduler-observability 提供跨组件追踪
```

### Worker生命周期

```
1. Worker注册
   Worker → API → Application → Infrastructure → Database (注册Worker信息)
   Worker → Infrastructure → Message Queue (订阅任务队列)
   scheduler-worker → scheduler-api → scheduler-application

2. 心跳机制
   Worker → API → Application → Infrastructure (定期心跳)
   API → Infrastructure → Database (更新Worker状态)
   Dispatcher → Infrastructure → Database (检查Worker健康度)
   调用链经过多个crate但保持松耦合

3. 任务分配
   Dispatcher → Infrastructure → Database (查询可用Worker)
   Dispatcher → Infrastructure → Message Queue (发送任务到指定Worker)
   scheduler-dispatcher → scheduler-infrastructure

4. 故障处理与恢复
   Dispatcher → RecoveryService → Database (检测超时Worker)
   Dispatcher → RetryService → Message Queue (重新分配任务)
   scheduler-dispatcher 内置故障恢复机制
```

### 配置管理流程

```
1. 配置加载
   Config Module → Environment → Validation → Application
   scheduler-config 统一管理所有配置

2. 运行时配置更新
   API → Config Service → Infrastructure → Message Queue (配置变更通知)
   支持热重载和动态配置更新

3. 环境适配
   Config → Environment Detection → Feature Flags
   根据环境自动调整配置参数
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

// JWT令牌认证 (新增)
pub struct JwtToken {
    pub sub: String,        // 主题
    pub exp: usize,         // 过期时间
    pub permissions: Vec<Permission>,
    pub roles: Vec<Role>,   // 角色支持
}

// 认证守卫 (新增)
pub struct AuthGuard {
    pub required_permission: Permission,
    pub allow_anonymous: bool,
}
```

### 权限控制

```rust
// 权限定义 (已完善)
pub enum Permission {
    TaskRead,      // 任务读取
    TaskWrite,     // 任务写入
    TaskExecute,   // 任务执行
    TaskDelete,    // 任务删除
    WorkerRead,    // Worker读取
    WorkerWrite,   // Worker写入
    SystemRead,    // 系统读取
    SystemWrite,   // 系统写入
    ConfigRead,    // 配置读取
    ConfigWrite,   // 配置写入
}

// 角色定义 (新增)
pub struct Role {
    pub name: String,
    pub permissions: Vec<Permission>,
    pub description: String,
}

// 预定义角色
pub enum SystemRole {
    Admin,      // 管理员 - 所有权限
    Operator,   // 操作员 - 任务和Worker管理
    ReadOnly,   // 只读用户 - 查看权限
    Worker,     // Worker节点 - 任务执行权限
}
```

### 安全中间件

```rust
// 认证中间件
pub fn auth_middleware() -> impl Layer<Router> {
    // JWT验证和API密钥验证
}

// 速率限制中间件 (新增)
pub fn rate_limit_middleware() -> impl Layer<Router> {
    // 基于令牌桶算法的速率限制
}

// CORS中间件
pub fn cors_middleware() -> impl Layer<Router> {
    // 跨域资源共享控制
}
```

### 数据安全

1. **敏感信息加密**: 配置文件中的密码和密钥支持AES加密
2. **数据库连接**: 强制SSL/TLS加密连接
3. **消息传输**: 消息队列支持TLS加密通信
4. **审计日志**: 完整的操作审计记录，包含用户身份和操作上下文
5. **输入验证**: API端点的综合输入验证和参数清理

## 📊 可观测性架构

### 分布式追踪 (新增)

```rust
// 跨组件追踪
pub struct CrossComponentTracer {
    pub tracer: Arc<dyn Tracer>,
    pub context_propagator: TraceContextPropagator,
}

// 消息追踪扩展
pub trait MessageTracingExt {
    fn inject_trace_context(&mut self, context: &TraceContext);
    fn extract_trace_context(&self) -> Option<TraceContext>;
}

// 追踪上下文
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub baggage: HashMap<String, String>,
}
```

### 指标收集

```rust
// 系统指标 (已增强)
pub struct SystemMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_io: NetworkIo,
    pub open_file_descriptors: u64,
    pub thread_count: u32,
}

// 应用指标
pub struct ApplicationMetrics {
    pub task_count: u64,
    pub worker_count: u64,
    pub message_queue_size: u64,
    pub database_connections: u32,
    pub cache_hit_rate: f64,
    pub circuit_breaker_state: HashMap<String, CircuitBreakerState>,
}

// 业务指标 (已增强)
pub struct BusinessMetrics {
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub tasks_cancelled: u64,
    pub average_execution_time: f64,
    pub worker_utilization: f64,
    pub queue_depth: u64,
    pub retry_count: u64,
}
```

### 缓存监控 (新增)

```rust
// 缓存指标
pub struct CacheMetrics {
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub memory_usage: u64,
    pub operation_latency: Duration,
}

// 缓存健康检查
pub struct CacheHealthChecker {
    pub redis_connection_status: bool,
    pub response_time: Duration,
    pub memory_pressure: f64,
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

## 📋 架构更新总结

### 本次更新内容

本次架构文档更新反映了系统从原始设计到当前企业级分布式任务调度系统的完整演进：

#### 1. **Crate架构重构**
- 从7个模块扩展到11个专门化crate
- 清洁架构和DDD原则的严格实现
- 明确的依赖层次和职责划分

#### 2. **新增核心特性**
- ✅ **认证授权系统**: JWT + API密钥 + 基于角色的权限控制
- ✅ **分布式追踪**: 跨组件上下文传播和OpenTelemetry集成
- ✅ **多层缓存**: Redis分布式缓存和指标监控
- ✅ **熔断器保护**: 数据库和消息队列的弹性保护
- ✅ **速率限制**: 令牌桶算法API限流
- ✅ **资源管理**: 任务执行器的完善生命周期管理

#### 3. **性能优化实现**
- ✅ **并行处理**: 任务扫描和调度的并发优化
- ✅ **查询优化**: 消除N+1模式，批量查询实现
- ✅ **超时控制**: 全面的异步操作超时保护
- ✅ **连接池优化**: 数据库和消息队列连接管理

#### 4. **可观测性增强**
- ✅ **结构化日志**: 统一的日志格式和上下文
- ✅ **分布式追踪**: 请求全链路跟踪
- ✅ **指标监控**: 业务、系统、应用多维度监控
- ✅ **健康检查**: 组件状态监控和告警

### 架构成熟度评估

| 方面 | 重构前 | 重构后 | 改进程度 |
|------|--------|--------|----------|
| **模块化程度** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 显著提升 |
| **安全性** | ⭐⭐ | ⭐⭐⭐⭐⭐ | 根本改善 |
| **可观测性** | ⭐⭐ | ⭐⭐⭐⭐⭐ | 全面增强 |
| **性能** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 大幅优化 |
| **可维护性** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 显著提升 |
| **扩展性** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 根本改善 |
| **测试性** | ⭐⭐ | ⭐⭐⭐⭐⭐ | 全面提升 |

### 下一步演进方向

1. **微服务拆分**: 进一步按业务领域拆分独立可部署的服务
2. **事件溯源**: 实现事件驱动架构和事件存储
3. **多租户支持**: 添加租户隔离和资源配额管理
4. **插件生态**: 开发插件SDK和扩展机制
5. **云原生优化**: Kubernetes Operator和云平台集成

### 文档版本

- **版本**: v2.0 (对应系统架构重构)
- **更新日期**: 2025-08-13
- **更新内容**: 反映crate重构和新功能特性
- **下次更新**: 在微服务拆分完成后

---

本文档描述了分布式任务调度系统的完整架构设计，包括重构后的模块架构、数据流、新增特性、部署策略等方面。系统现已采用现代化企业级架构设计，具有优秀的扩展性、可用性、安全性和可维护性。