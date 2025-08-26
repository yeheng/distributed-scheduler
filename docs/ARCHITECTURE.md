# 系统架构

## 🏗️ 架构概述

分布式任务调度系统采用微服务架构，基于Rust构建，支持高并发任务调度和分布式Worker管理。

```
┌─────────────────────────────────────────────────────────────┐
│                    分布式任务调度系统                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐   │
│  │   API 服务   │    │   调度器     │    │   Worker    │   │
│  │   (REST)    │    │(Dispatcher) │    │   (执行器)   │   │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘   │
│         │                  │                  │          │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              消息队列 (Message Queue)                │   │
│  │         RabbitMQ / Redis Stream                     │   │
│  └──────────────────────────────────────────────────────┘   │
│         │                  │                  │          │
│  ┌──────┴──────┐    ┌──────┴──────┐    ┌──────┴──────┐   │
│  │   数据库     │    │    缓存      │    │    监控      │   │
│  │(PostgreSQL) │    │   (Redis)   │    │(Prometheus) │   │
│  └─────────────┘    └─────────────┘    └─────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## 📦 模块架构

### Crate依赖层次

```
scheduler (主程序)
├── api/           # HTTP API服务
├── dispatcher/    # 任务调度器
├── worker/        # 任务执行器
├── application/   # 业务逻辑层
├── infrastructure/# 基础设施层
├── domain/        # 领域模型
├── config/        # 配置管理
├── observability/ # 监控日志
└── errors/        # 错误处理
```

### 核心组件职责

#### API服务 (api/)
- HTTP REST接口
- 请求验证和路由
- 认证授权中间件
- API文档生成

#### 调度器 (dispatcher/)  
- 任务调度策略
- 依赖管理
- 故障检测和恢复
- Worker负载均衡

#### Worker (worker/)
- 任务执行引擎
- 心跳管理
- 状态上报
- 支持多种执行器类型

#### 基础设施 (infrastructure/)
- 数据库访问层
- 消息队列适配器
- 缓存管理
- 外部服务集成

## 🔄 数据流

### 任务创建流程

```
用户请求 → API服务 → 验证参数 → 存储任务 → 发送调度消息 → 调度器接收
```

### 任务执行流程

```
调度器 → 选择Worker → 发送任务消息 → Worker执行 → 结果上报 → 状态更新
```

### 状态同步流程

```
Worker心跳 → 调度器更新状态 → API查询最新状态 → 返回给用户
```

## 🗃️ 数据模型

### 核心实体

#### 任务 (Task)
```rust
pub struct Task {
    pub id: Option<i64>,
    pub name: String,           // 任务名称
    pub executor: String,       // 执行器类型 (shell, http, etc)
    pub command: Option<String>, // Shell命令
    pub schedule: Option<String>, // Cron表达式
    pub status: TaskStatus,     // 任务状态
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

#### 任务运行 (TaskRun)  
```rust
pub struct TaskRun {
    pub id: Option<i64>,
    pub task_id: i64,
    pub worker_id: Option<String>,
    pub status: TaskRunStatus,  // 运行状态
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<String>, // 执行结果
    pub error: Option<String>,  // 错误信息
}
```

#### Worker信息 (WorkerInfo)
```rust
pub struct WorkerInfo {
    pub id: String,            // Worker唯一标识
    pub name: String,          // Worker名称
    pub status: WorkerStatus,  // 状态 (Online, Offline, Busy)
    pub last_heartbeat: DateTime<Utc>,
    pub current_tasks: i32,    // 当前执行任务数
    pub max_tasks: i32,        // 最大任务容量
}
```

## 🔧 关键技术决策

### 消息队列选择
- **RabbitMQ**: 可靠消息传递，支持复杂路由
- **Redis Stream**: 高性能，简单部署
- 支持运行时切换，通过配置管理

### 数据库选择
- **PostgreSQL**: 生产环境推荐，支持事务
- **SQLite**: 开发环境，简单部署
- 使用SQLx实现类型安全的SQL操作

### 并发模型
- 基于Tokio异步运行时
- 无锁数据结构减少竞争
- 背压控制防止过载

### 容错机制
- 熔断器模式防止级联故障
- 指数退避重试策略
- 故障转移和自动恢复

## 📊 性能特性

### 调度性能
- **吞吐量**: 10,000+ tasks/minute
- **延迟**: < 100ms (P99)
- **并发**: 1,000+ concurrent tasks

### 扩展性
- **Worker节点**: 100+ nodes
- **水平扩展**: 支持多调度器实例
- **数据分片**: 支持数据库分片

### 资源使用
- **内存**: 基线 < 50MB per service
- **CPU**: 多核并行处理
- **网络**: 高效二进制协议

## 🔐 安全设计

### 认证授权
- API密钥认证
- JWT令牌支持
- 基于角色的访问控制 (RBAC)

### 数据安全
- 敏感配置加密存储
- 传输层TLS加密
- SQL注入防护

### 网络安全
- 防火墙配置建议
- 网络隔离
- 审计日志

## 🔍 监控与可观测性

### 指标收集
- 系统指标: CPU、内存、网络
- 业务指标: 任务成功率、执行时间
- 自定义指标: Prometheus格式

### 日志系统
- 结构化日志 (JSON格式)
- 分级日志记录
- 日志聚合和搜索

### 分布式追踪
- OpenTelemetry集成
- 跨服务请求追踪
- 性能瓶颈分析

## 🚀 部署架构

### 开发环境
```
单机部署 + Docker Compose
- 所有服务运行在同一主机
- 适合开发和测试
```

### 生产环境
```
分布式部署 + Kubernetes
- 服务独立部署和扩展
- 负载均衡和故障转移
- 数据持久化和备份
```

### 混合云部署
```
多云环境支持
- 跨区域灾备
- 数据同步机制
- 网络连通性管理
```

