# 分布式任务调度系统架构文档

## 项目概览

这是一个用 Rust 构建的高性能分布式任务调度系统，采用微服务架构，具有基于消息队列的通信机制和清洁架构原则。系统专为高可扩展性和容错性设计，严格遵循 DRY、SOLID、YAGNI 原则。

## 系统架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                        用户接口层                                │
├─────────────────┬─────────────────┬─────────────────┬─────────────────┤
│   REST API      │    CLI 工具     │   Web 控制台    │   第三方集成    │
│   (Axum)        │                 │                 │                 │
└─────────────────┼─────────────────┼─────────────────┼─────────────────┘
                  │                 │                 │
┌─────────────────────────────────────────────────────────────────┐
│                      应用服务层                                  │
├─────────────────┬─────────────────┬─────────────────┬─────────────────┤
│  任务控制服务   │  调度策略服务   │  工作节点服务   │  监控告警服务   │
│                 │                 │                 │                 │
└─────────────────┼─────────────────┼─────────────────┼─────────────────┘
                  │                 │                 │
┌─────────────────────────────────────────────────────────────────┐
│                      领域层                                      │
├─────────────────┬─────────────────┬─────────────────┬─────────────────┤
│     任务域      │    工作节点域   │    消息域       │    事件域       │
│  Task, TaskRun  │  WorkerInfo     │   Message       │   Events        │
└─────────────────┼─────────────────┼─────────────────┼─────────────────┘
                  │                 │                 │
┌─────────────────────────────────────────────────────────────────┐
│                     基础设施层                                   │
├─────────────────┬─────────────────┬─────────────────┬─────────────────┤
│   数据持久化    │    消息队列     │     缓存        │   监控指标      │
│ PostgreSQL/     │ RabbitMQ/       │    Redis        │  Prometheus     │
│   SQLite        │ Redis Stream    │                 │   OpenTelemetry │
└─────────────────┴─────────────────┴─────────────────┴─────────────────┘
```

### 核心组件

#### 1. 调度器 (Dispatcher)
- **职责**: 任务调度、分发、依赖管理
- **位置**: `crates/dispatcher/`
- **核心模块**:
  - 调度策略 (`strategies/`)
  - 任务依赖管理 (`dependencies/`)
  - 工作节点故障检测 (`worker_failure_detector.rs`)
  - 重试服务 (`retry_service.rs`)

#### 2. 工作节点 (Worker)
- **职责**: 任务执行、心跳上报、生命周期管理
- **位置**: `crates/worker/`
- **核心模块**:
  - 任务执行器 (`executors/`)
  - 心跳管理 (`components/heartbeat_manager.rs`)
  - 生命周期控制 (`components/worker_lifecycle.rs`)
  - 调度器客户端 (`components/dispatcher_client.rs`)

#### 3. API 服务 (API)
- **职责**: REST API 接口、认证授权、请求处理
- **位置**: `crates/api/`
- **核心模块**:
  - 请求处理器 (`handlers/`)
  - 认证系统 (`auth/`)
  - 输入验证 (`validation/`)
  - 错误处理 (`error.rs`)

#### 4. CLI 工具 (CLI)
- **职责**: 命令行管理工具
- **位置**: `crates/cli/`

## 模块架构

### Crate 依赖关系

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│      API        │    │   Dispatcher    │    │     Worker      │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
┌─────────────────────────────────┼─────────────────────────────────┐
│                            Application                           │
└─────────────────────────────────┼─────────────────────────────────┘
                                 │
┌─────────────────────────────────┼─────────────────────────────────┐
│                             Domain                               │
└─────────────────────────────────┼─────────────────────────────────┘
                                 │
┌─────────┬─────────┬─────────────┼─────────┬─────────┬─────────────┐
│ Config  │ Errors  │Infrastructure│Observab.│  Core   │Common/Utils │
└─────────┴─────────┴─────────────┴─────────┴─────────┴─────────────┘
```

### 各 Crate 详细说明

#### 核心领域和业务逻辑层

**`crates/domain/`** - 领域模型和业务实体
- `entities.rs` - 核心实体 (Task, TaskRun, WorkerInfo, Message)
- `services.rs` - 领域服务接口
- `events.rs` - 领域事件
- `ports/` - 端口定义 (Repository, MessageQueue interfaces)

**`crates/application/`** - 应用服务和用例
- `services/` - 业务用例实现
- `ports/` - 服务接口定义
- 负责协调领域对象完成业务流程

**`crates/errors/`** - 统一错误处理
- `SchedulerError` 枚举类型
- `SchedulerResult<T>` 类型别名
- 错误转换和处理逻辑

#### 基础设施和实现层

**`crates/infrastructure/`** - 基础设施实现
- `database/` - 数据库实现 (PostgreSQL, SQLite)
  - `postgres/` - PostgreSQL 仓储实现
  - `sqlite/` - SQLite 仓储实现
- `message_queue/` - 消息队列实现
  - `rabbitmq/` - RabbitMQ 实现
  - `redis_stream/` - Redis Stream 实现
- `cache/` - 缓存实现和管理
- 工厂模式实现 (`*_factory.rs`)

**`crates/config/`** - 配置管理
- TOML 配置文件支持
- 环境变量替换
- 配置验证和热更新
- 安全配置加密

**`crates/observability/`** - 可观测性
- 结构化日志
- 分布式追踪
- 指标收集
- 跨组件追踪

#### 服务和应用层

**`crates/api/`** - REST API 服务
- Axum 框架构建
- JWT 和 API Key 认证
- 基于角色的权限控制
- 输入验证和错误处理

**`crates/dispatcher/`** - 调度器核心
- 调度策略实现
- 依赖管理
- Cron 作业处理
- 工作节点故障检测

**`crates/worker/`** - 工作节点服务
- 任务执行引擎
- 心跳机制
- 生命周期管理
- 多种执行器支持

**`crates/cli/`** - 命令行工具
- 管理命令
- 配置工具
- 诊断工具

**`crates/core/`** - 核心模型和共享抽象
- 依赖注入容器
- 共享数据结构
- 核心抽象

**`crates/common/`** - 通用工具和辅助函数
**`crates/testing-utils/`** - 测试工具和模拟对象

## 核心设计模式

### 1. 清洁架构 (Clean Architecture)
- 领域驱动设计，关注点清晰分离
- 依赖方向：外层依赖内层，内层不依赖外层
- 端口和适配器模式实现技术无关性

### 2. 依赖注入 (Dependency Injection)
```rust
use scheduler_core::DependencyContainer;

// 服务抽象使用 Arc<dyn Trait>
let container = DependencyContainer::new(config).await?;
let task_service: Arc<dyn TaskService> = container.task_service();
```

### 3. 仓储模式 (Repository Pattern)
```rust
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, task: &Task) -> SchedulerResult<Task>;
    async fn get_by_id(&self, id: i64) -> SchedulerResult<Option<Task>>;
    async fn list(&self, filter: &TaskFilter) -> SchedulerResult<Vec<Task>>;
}
```

### 4. 服务层模式 (Service Layer)
```rust
#[async_trait]
pub trait TaskControlService: Send + Sync {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun>;
    async fn pause_task(&self, task_id: i64) -> SchedulerResult<()>;
}
```

### 5. 工厂模式 (Factory Pattern)
```rust
pub struct MessageQueueFactory;

impl MessageQueueFactory {
    pub async fn create(config: &MessageQueueConfig) -> SchedulerResult<Arc<dyn MessageQueue>> {
        match config.r#type.as_str() {
            "rabbitmq" => RabbitMQMessageQueue::new(config).await,
            "redis_stream" => RedisStreamMessageQueue::new(config).await,
            _ => Err(SchedulerError::ConfigurationError("不支持的消息队列类型".into()))
        }
    }
}
```

## 数据流和处理流程

### 任务生命周期
```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   创建任务   │────│   调度任务   │────│   分发任务   │────│   执行任务   │
│   (API)      │    │(Dispatcher)  │    │(MessageQueue)│    │  (Worker)    │
└──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘
        │                    │                    │                    │
        ▼                    ▼                    ▼                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                        状态更新流程                                       │
│  TaskRun: Pending → Dispatched → Running → Completed/Failed/Timeout     │
└──────────────────────────────────────────────────────────────────────────┘
```

### 消息流处理
```
API Service ──┐
              ├─→ Message Queue ──→ Dispatcher ──→ Worker Nodes
Worker ───────┘         │                │
                        └── Status ←─────┘
                         Updates
```

### 工作节点管理流程
```
Worker Registration
        │
        ▼
Heartbeat Monitoring ←─────┐
        │                  │
        ▼                  │
Task Assignment ───────────┘
        │
        ▼
Task Execution
        │
        ▼
Status Reporting
```

## 技术栈

### 核心技术
- **Rust 2021 Edition** - 系统编程语言
- **Tokio** - 异步运行时
- **Axum** - Web 框架
- **SQLx** - 数据库抽象层
- **Serde** - 序列化/反序列化

### 数据库
- **PostgreSQL** - 生产环境主数据库
- **SQLite** - 开发和测试数据库
- **Redis** - 缓存和 Stream 消息队列

### 消息队列
- **RabbitMQ** - 生产级消息队列
- **Redis Stream** - 高性能流式消息队列

### 监控和观测
- **OpenTelemetry** - 分布式追踪
- **Prometheus** - 指标收集
- **Tracing** - 结构化日志
- **Metrics** - 性能指标

### 认证和安全
- **JWT** - 用户身份验证
- **API Keys** - 服务间认证
- **RBAC** - 基于角色的权限控制
- **Input Validation** - 输入验证

## 部署架构

### 单节点部署
```
┌─────────────────────────────────────────────────┐
│                服务器节点                       │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐│
│  │ API Service │ │ Dispatcher  │ │   Worker    ││
│  └─────────────┘ └─────────────┘ └─────────────┘│
│  ┌─────────────────────────────────────────────┐│
│  │          数据库 + 消息队列               ││
│  └─────────────────────────────────────────────┘│
└─────────────────────────────────────────────────┘
```

### 分布式部署
```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ API Gateway │    │ Dispatcher  │    │Load Balancer│
└──────┬──────┘    └──────┬──────┘    └──────┬──────┘
       │                  │                  │
┌──────▼──────┐    ┌──────▼──────┐    ┌──────▼──────┐
│API Service  │    │Message Queue│    │Worker Pool  │
│(Multiple)   │    │RabbitMQ/    │    │(Auto-scale) │
└─────────────┘    │Redis Stream │    └─────────────┘
                   └─────────────┘
                          │
                   ┌──────▼──────┐
                   │  Database   │
                   │ PostgreSQL  │
                   │  (Cluster)  │
                   └─────────────┘
```

## 服务通信

### 消息路由策略
```rust
// 任务执行消息路由
"task.execution.{task_type}"     // 按任务类型路由

// 状态更新消息路由  
"status.update.{worker_id}"      // 按工作节点路由

// 工作节点心跳路由
"worker.heartbeat.{worker_id}"   // 按工作节点路由

// 任务控制消息路由
"task.control.{action}"          // 按控制动作路由
```

### 消息格式
```json
{
  "id": "uuid",
  "message_type": {
    "type": "TaskExecution",
    "task_run_id": 123,
    "task_id": 456,
    "task_name": "数据备份",
    "task_type": "backup",
    "parameters": {...},
    "timeout_seconds": 3600,
    "retry_count": 0
  },
  "payload": {...},
  "timestamp": "2024-01-01T00:00:00Z",
  "retry_count": 0,
  "correlation_id": "trace-id",
  "trace_headers": {
    "traceparent": "00-...",
    "tracestate": "..."
  }
}
```

## 可扩展性设计

### 水平扩展
1. **API 服务**: 无状态设计，可任意扩展
2. **调度器**: 支持多实例部署，通过消息队列协调
3. **工作节点**: 自动发现和注册，动态扩缩容
4. **数据库**: 支持读写分离和分片

### 负载均衡
- **轮询策略**: 平均分配任务负载
- **负载感知**: 根据工作节点当前负载分配
- **任务类型匹配**: 根据工作节点能力分配

### 容错机制
- **重试策略**: 指数退避重试
- **熔断器**: 防止级联故障
- **超时控制**: 防止任务无限执行
- **健康检查**: 定期检测服务状态

## 配置管理

### 分层配置
```
base.toml           # 基础默认配置
├── development.toml # 开发环境覆盖
├── staging.toml    # 测试环境覆盖
└── production.toml # 生产环境覆盖

auth.toml           # 认证配置
security.toml       # 安全配置  
timeouts.toml       # 超时配置
```

### 配置热更新
系统支持配置热更新，无需重启服务。配置变更会触发相关组件重新初始化。

## 安全架构

### 认证流程
```
用户/服务 ──login──→ JWT Token ──access──→ API Endpoints
    │                    │                      │
    └──refresh──→ Refresh Token ────────────────┘
                         │
                    API Key ─────────────────────┘
```

### 权限控制
```rust
pub enum Permission {
    Admin,          // 管理员权限
    TaskRead,       // 任务读权限
    TaskWrite,      // 任务写权限
    TaskDelete,     // 任务删除权限
    WorkerRead,     // 工作节点读权限
    WorkerWrite,    // 工作节点写权限
    SystemRead,     // 系统读权限
    SystemWrite,    // 系统写权限
}
```

### 安全特性
- JWT 令牌认证和刷新
- API 密钥认证
- 基于角色的访问控制 (RBAC)
- 输入验证和 SQL 注入防护
- 敏感配置加密
- 限流和请求节流

## 监控和可观测性

### 监控指标
- **系统指标**: CPU、内存、网络使用率
- **业务指标**: 任务执行成功率、响应时间
- **应用指标**: API 请求量、错误率
- **基础设施指标**: 数据库连接数、消息队列长度

### 分布式追踪
```rust
// 跨组件追踪
let span = CrossComponentTracer::instrument_service_call(
    "api",
    "trigger_task", 
    vec![
        ("task.id", task_id.to_string()),
        ("user.id", user_id.to_string()),
    ],
);
```

### 日志策略
- **结构化日志**: JSON 格式，便于解析
- **日志级别**: ERROR, WARN, INFO, DEBUG, TRACE
- **上下文信息**: 请求ID、用户ID、任务ID
- **性能日志**: 执行时间、资源使用

## 性能优化

### 数据库优化
- 连接池管理
- 索引优化
- 查询批处理
- 读写分离

### 消息队列优化
- 批量操作
- 消息压缩
- 持久化策略
- 消息路由优化

### 缓存策略
- Redis 多级缓存
- 缓存预热
- 失效策略
- 一致性保证

### 并发处理
- Tokio 异步运行时
- 并发任务执行
- 资源池管理
- 背压控制

## 开发指南

### 代码规范
- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 进行静态检查
- 遵循 Rust 命名约定
- 编写全面的文档注释

### 测试策略
- **单元测试**: 业务逻辑测试
- **集成测试**: 跨模块功能测试
- **端到端测试**: 完整流程测试
- **性能测试**: 基准测试和压力测试

### 开发工作流
1. **环境搭建**: Docker Compose 启动依赖服务
2. **代码开发**: 遵循架构约束和编码规范
3. **测试验证**: 运行完整测试套件
4. **架构验证**: 执行架构合规检查
5. **性能基准**: 运行性能基准测试
6. **文档更新**: 更新相关技术文档

### 架构约束检查
```bash
# 运行架构合规检查
./scripts/architecture_check.sh

# 性能基准测试
./scripts/run_benchmarks.sh
```

## 版本和发布

### 版本策略
- 遵循语义化版本 (SemVer)
- 主版本：破坏性变更
- 次版本：新功能添加
- 修订版本：bug 修复

### 发布流程
1. 功能开发和测试
2. 版本号更新
3. 变更日志生成
4. Docker 镜像构建
5. 自动化部署

## 故障排除

### 常见问题
1. **消息队列连接失败**: 检查网络和认证配置
2. **数据库连接超时**: 检查连接池配置和网络延迟
3. **任务执行超时**: 调整超时设置和资源配额
4. **工作节点下线**: 检查心跳配置和网络稳定性

### 监控告警
- **服务健康**: API 健康检查端点
- **业务监控**: 任务执行成功率监控
- **资源监控**: 系统资源使用率监控
- **错误监控**: 错误率和异常监控

### 性能调优
- 数据库查询优化
- 消息队列吞吐量调优
- 工作节点负载均衡优化
- 缓存命中率提升

这个架构确保了系统的高可用性、高性能和易维护性，同时保持了良好的扩展性和技术债务控制。

