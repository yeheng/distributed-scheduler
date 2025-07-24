# 分布式任务调度系统设计文档

## 概述

本文档描述了一个分布式任务调度系统的架构设计。该系统采用 Dispatcher-Worker 架构模式，使用 RabbitMQ 作为消息队列，PostgreSQL 作为数据存储，支持高可用性、水平扩展和可靠的任务执行。

### 核心设计原则

1. **简化架构**: Dispatcher 负责任务调度，Worker 节点无状态执行任务
2. **高可用性**: 通过多个 Dispatcher 实例和消息队列确保系统的高可用性
3. **水平扩展**: Worker 节点可以动态加入和退出，支持负载的水平扩展
4. **数据一致性**: 利用 PostgreSQL 的 ACID 特性确保任务状态的一致性
5. **故障恢复**: 实现完善的故障检测和自动恢复机制

## 架构

### 系统架构图

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ Dispatcher Node │    │ Dispatcher Node │    │ Dispatcher Node │
│                 │    │                 │    │                 │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │     PostgreSQL          │
                    │    (Data Storage)       │
                    └─────────────────────────┘
                                 
                    ┌─────────────────────────┐
                    │      RabbitMQ           │
                    │   (Message Queue)       │
                    └────────────┬────────────┘
                                 │
          ┌──────────────────────┼──────────────────────┐
          │                      │                      │
┌─────────▼───────┐    ┌─────────▼───────┐    ┌─────────▼───────┐
│  Worker Node 1  │    │  Worker Node 2  │    │  Worker Node N  │
│   (Stateless)   │    │   (Stateless)   │    │   (Stateless)   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### 组件说明

- **Dispatcher Node**: 负责任务调度和状态管理，可以有多个实例运行
- **Worker Node**: 无状态的任务执行节点，从消息队列获取任务
- **PostgreSQL**: 数据存储，保存任务定义和执行历史
- **RabbitMQ**: 消息队列，用于任务分发和状态更新

## 组件和接口

### 1. Dispatcher 组件

#### 1.1 Task Scheduler

负责扫描待执行任务并创建执行实例。

```rust
#[async_trait]
pub trait TaskSchedulerService: Send + Sync {
    async fn scan_and_schedule(&self) -> Result<Vec<TaskRun>>;
    async fn check_dependencies(&self, task: &Task) -> Result<bool>;
    async fn create_task_run(&self, task: &Task) -> Result<TaskRun>;
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> Result<()>;
}

pub struct TaskScheduler {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    message_queue: Arc<dyn MessageQueue>,
}

#[async_trait]
impl TaskSchedulerService for TaskScheduler {
    async fn scan_and_schedule(&self) -> Result<Vec<TaskRun>>;
    async fn check_dependencies(&self, task: &Task) -> Result<bool>;
    async fn create_task_run(&self, task: &Task) -> Result<TaskRun>;
    async fn dispatch_to_queue(&self, task_run: &TaskRun) -> Result<()>;
}
```

#### 1.3 Task Control Service

负责任务的生命周期控制操作。

```rust
#[async_trait]
pub trait TaskControlService: Send + Sync {
    async fn trigger_task(&self, task_id: i64) -> Result<TaskRun>;
    async fn pause_task(&self, task_id: i64) -> Result<()>;
    async fn resume_task(&self, task_id: i64) -> Result<()>;
    async fn restart_task(&self, task_id: i64) -> Result<TaskRun>;
    async fn abort_task_run(&self, task_run_id: i64) -> Result<()>;
}

pub struct TaskController {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
    message_queue: Arc<dyn MessageQueue>,
}

#[async_trait]
impl TaskControlService for TaskController {
    async fn trigger_task(&self, task_id: i64) -> Result<TaskRun>;
    async fn pause_task(&self, task_id: i64) -> Result<()>;
    async fn resume_task(&self, task_id: i64) -> Result<()>;
    async fn restart_task(&self, task_id: i64) -> Result<TaskRun>;
    async fn abort_task_run(&self, task_run_id: i64) -> Result<()>;
}
```

#### 1.4 Task Dispatch Strategy

负责决定任务分派到哪个 Worker 的策略。

```rust
#[async_trait]
pub trait TaskDispatchStrategy: Send + Sync {
    async fn select_worker(&self, task: &Task, available_workers: &[WorkerInfo]) -> Result<Option<String>>;
}

pub struct RoundRobinStrategy;
pub struct LoadBasedStrategy;
pub struct TaskTypeAffinityStrategy;

#[async_trait]
impl TaskDispatchStrategy for RoundRobinStrategy {
    async fn select_worker(&self, task: &Task, available_workers: &[WorkerInfo]) -> Result<Option<String>>;
}

#[async_trait]
impl TaskDispatchStrategy for LoadBasedStrategy {
    async fn select_worker(&self, task: &Task, available_workers: &[WorkerInfo]) -> Result<Option<String>>;
}

#[async_trait]
impl TaskDispatchStrategy for TaskTypeAffinityStrategy {
    async fn select_worker(&self, task: &Task, available_workers: &[WorkerInfo]) -> Result<Option<String>>;
}
```

#### 1.2 State Listener

监听来自 Worker 的状态更新消息。

```rust
#[async_trait]
pub trait StateListenerService: Send + Sync {
    async fn listen_for_updates(&self) -> Result<()>;
    async fn process_status_update(&self, update: TaskStatusUpdate) -> Result<()>;
}

pub struct StateListener {
    task_run_repo: Arc<dyn TaskRunRepository>,
    message_queue: Arc<dyn MessageQueue>,
}

#[async_trait]
impl StateListenerService for StateListener {
    async fn listen_for_updates(&self) -> Result<()>;
    async fn process_status_update(&self, update: TaskStatusUpdate) -> Result<()>;
}
```
```
```

### 2. Worker 组件

#### 2.1 Task Executor

负责具体任务的执行。

**设计决策**: 采用插件化的执行器设计，支持不同类型的任务执行器（Shell、HTTP、自定义等）。

```rust
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute(&self, task_run: &TaskRun) -> Result<TaskResult>;
    fn supports_task_type(&self, task_type: &str) -> bool;
}

pub struct ShellExecutor;
pub struct HttpExecutor;
```

#### 2.2 Worker Service

Worker 节点的主服务，负责任务轮询和执行协调。

```rust
#[async_trait]
pub trait WorkerServiceTrait: Send + Sync {
    async fn start(&self) -> Result<()>;
    async fn poll_and_execute_tasks(&self) -> Result<()>;
    async fn send_status_update(&self, update: TaskStatusUpdate) -> Result<()>;
}

pub struct WorkerService {
    worker_id: String,
    message_queue: Arc<dyn MessageQueue>,
    executors: HashMap<String, Box<dyn TaskExecutor>>,
    max_concurrent_tasks: usize,
}

#[async_trait]
impl WorkerServiceTrait for WorkerService {
    async fn start(&self) -> Result<()>;
    async fn poll_and_execute_tasks(&self) -> Result<()>;
    async fn send_status_update(&self, update: TaskStatusUpdate) -> Result<()>;
}
```

### 3. 数据访问层

#### 3.1 Repository 接口

定义数据访问的抽象接口，便于测试和扩展。

```rust
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create_task(&self, task: &Task) -> Result<Task>;
    async fn get_task(&self, id: i64) -> Result<Option<Task>>;
    async fn update_task(&self, task: &Task) -> Result<()>;
    async fn list_tasks(&self, filter: TaskFilter) -> Result<Vec<Task>>;
    async fn get_schedulable_tasks(&self, now: DateTime<Utc>) -> Result<Vec<Task>>;
}

#[async_trait]
pub trait TaskRunRepository: Send + Sync {
    async fn create_task_run(&self, task_run: &TaskRun) -> Result<TaskRun>;
    async fn update_task_run_status(&self, id: i64, status: TaskRunStatus, result: Option<String>) -> Result<()>;
    async fn get_pending_task_runs(&self, worker_id: Option<&str>) -> Result<Vec<TaskRun>>;
    async fn get_running_tasks_for_worker(&self, worker_id: &str) -> Result<Vec<TaskRun>>;
}
```

### 4. 消息队列抽象层

**设计决策**: 引入 MessageQueue 抽象层，支持 RabbitMQ 等消息队列系统，使用统一的消息对象实现任务分发和状态更新的解耦。

```rust
#[async_trait]
pub trait MessageQueue: Send + Sync {
    // 消息分发
    async fn publish_message(&self, queue: &str, message: &Message) -> Result<()>;
    
    // 消息消费
    async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub message_type: MessageType,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub retry_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    TaskExecution(TaskExecutionMessage),
    StatusUpdate(StatusUpdateMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionMessage {
    pub task_run_id: i64,
    pub task_id: i64,
    pub task_type: String,
    pub parameters: serde_json::Value,
    pub timeout_seconds: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdateMessage {
    pub task_run_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: String,
    pub result: Option<String>,
    pub error_message: Option<String>,
}
```

## 数据模型

### 核心实体

#### Task (任务定义)

```rust
#[derive(Debug, Clone)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub task_type: String,           // "shell", "http", etc.
    pub schedule: String,            // cron 表达式
    pub parameters: serde_json::Value,
    pub timeout_seconds: i32,
    pub max_retries: i32,
    pub status: TaskStatus,          // ACTIVE, INACTIVE
    pub dependencies: Vec<i64>,      // 依赖的任务 ID
    pub shard_config: Option<ShardConfig>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Active,
    Inactive,
}
```

#### TaskRun (任务执行实例)

```rust
#[derive(Debug, Clone)]
pub struct TaskRun {
    pub id: i64,
    pub task_id: i64,
    pub status: TaskRunStatus,
    pub worker_id: Option<String>,
    pub retry_count: i32,
    pub shard_index: Option<i32>,
    pub shard_total: Option<i32>,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub enum TaskRunStatus {
    Pending,
    Dispatched,
    Running,
    Completed,
    Failed,
    Timeout,
}
```

#### Worker (工作节点)

```rust
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub id: String,
    pub hostname: String,
    pub ip_address: String,
    pub supported_task_types: Vec<String>,
    pub max_concurrent_tasks: i32,
    pub status: WorkerStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub registered_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum WorkerStatus {
    Alive,
    Down,
}
```

### 数据库表结构

```sql
-- 任务定义表
CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    task_type VARCHAR(50) NOT NULL,
    schedule VARCHAR(100) NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}',
    timeout_seconds INTEGER NOT NULL DEFAULT 300,
    max_retries INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    dependencies BIGINT[] DEFAULT '{}',
    shard_config JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 任务执行实例表
CREATE TABLE task_runs (
    id BIGSERIAL PRIMARY KEY,
    task_id BIGINT NOT NULL REFERENCES tasks(id),
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    worker_id VARCHAR(255),
    retry_count INTEGER NOT NULL DEFAULT 0,
    shard_index INTEGER,
    shard_total INTEGER,
    scheduled_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    result TEXT,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Worker 节点表
CREATE TABLE workers (
    id VARCHAR(255) PRIMARY KEY,
    hostname VARCHAR(255) NOT NULL,
    ip_address INET NOT NULL,
    supported_task_types TEXT[] NOT NULL,
    max_concurrent_tasks INTEGER NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'ALIVE',
    last_heartbeat TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 索引优化
CREATE INDEX idx_tasks_schedule_status ON tasks(schedule, status) WHERE status = 'ACTIVE';
CREATE INDEX idx_task_runs_status_worker ON task_runs(status, worker_id);
CREATE INDEX idx_task_runs_task_id_status ON task_runs(task_id, status);
CREATE INDEX idx_workers_status_heartbeat ON workers(status, last_heartbeat);
```

## 错误处理

### 错误类型定义

```rust
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Task not found: {id}")]
    TaskNotFound { id: i64 },
    
    #[error("Worker not found: {id}")]
    WorkerNotFound { id: String },
    
    #[error("Invalid cron expression: {expr}")]
    InvalidCron { expr: String },
    
    #[error("Task execution timeout")]
    ExecutionTimeout,
    
    #[error("Circular dependency detected")]
    CircularDependency,
    
    #[error("Leadership lost")]
    LeadershipLost,
}
```

### 错误处理策略

1. **数据库连接错误**: 实现指数退避重试机制
2. **任务执行错误**: 根据 max_retries 配置进行重试
3. **Worker 失效**: 自动重新分配任务到其他 Worker
4. **消息队列失效**: 实现重连机制和消息持久化

## 测试策略

### 单元测试

- Repository 层的数据访问逻辑
- 任务调度算法
- 依赖检查逻辑
- CRON 表达式解析

### 集成测试

- Dispatcher-Worker 协作流程
- 数据库事务一致性
- 故障转移机制
- 任务重试逻辑

### 端到端测试

- 完整任务生命周期
- 多节点协作场景
- 高负载压力测试
- 网络分区恢复测试

### 测试工具和框架

- 使用 `testcontainers` 进行数据库集成测试
- 使用 `tokio-test` 进行异步代码测试
- 使用 `mockall` 进行 mock 测试

## 部署和运维

### 配置管理

使用 TOML 格式的配置文件，支持环境变量覆盖：

```toml
[database]
url = "postgresql://user:pass@localhost/scheduler"
max_connections = 10

[dispatcher]
enabled = true
schedule_interval_seconds = 10

[worker]
enabled = false
worker_id = "worker-001"
max_concurrent_tasks = 5
heartbeat_interval_seconds = 30

[api]
bind_address = "0.0.0.0:8080"
```

### 监控和可观测性

- 使用 `tracing` 库实现结构化日志
- 暴露 Prometheus 指标端点
- 支持 OpenTelemetry 分布式追踪（Post-MVP）

### 部署模式

1. **单机模式**: Dispatcher 和 Worker 在同一进程中运行
2. **分布式模式**: Dispatcher 和 Worker 分别部署
3. **容器化部署**: 提供 Docker 镜像和 Kubernetes 配置

## 性能考虑

### 扩展性设计

- Worker 节点支持水平扩展
- 数据库连接池优化
- 任务轮询批量处理
- 合理的索引设计

### 性能优化

- 使用数据库连接池减少连接开销
- 批量处理任务状态更新
- 合理设置轮询间隔避免过度查询
- 实现任务分片支持大规模并行处理

### 资源管理

- Worker 并发任务数限制
- 内存使用监控和限制
- 数据库查询超时设置
- 优雅关闭机制

## 安全性

### 认证和授权（Post-MVP）

- API 接口的身份验证
- 基于角色的访问控制
- 敏感参数加密存储

### 网络安全（Post-MVP）

- TLS 加密通信
- 网络访问控制
- 审计日志记录

## 未来扩展

### 消息队列支持

当前使用数据库作为任务队列，未来可以通过 TaskBroker 抽象层轻松切换到 RabbitMQ、Apache Kafka 等消息队列系统。

### 高级调度功能

- 任务优先级调度
- 资源感知调度
- 地理位置感知调度

### 企业级功能

- 多租户支持
- 任务模板和工作流
- 可视化任务编排界面