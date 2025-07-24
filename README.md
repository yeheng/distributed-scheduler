# 分布式任务调度系统 (Distributed Task Scheduler)

一个基于Rust构建的高性能、可扩展的分布式任务调度系统。

## 项目结构

```
scheduler/
├── Cargo.toml              # Workspace配置
├── core/                   # 核心类型和接口
│   ├── src/
│   │   ├── models/         # 数据模型
│   │   ├── traits/         # 服务接口
│   │   ├── errors.rs       # 错误定义
│   │   └── config.rs       # 配置结构
├── dispatcher/             # 调度器服务
├── worker/                 # Worker服务
├── api/                    # REST API服务
└── infrastructure/         # 基础设施抽象
```

## 核心组件

### Core模块

- **Task**: 任务定义和状态管理
- **TaskRun**: 任务执行实例
- **Worker**: Worker节点信息
- **Message**: 消息队列通信结构
- **Traits**: 服务接口定义

### 数据模型

- `Task`: 任务定义，包含调度信息、参数、依赖等
- `TaskRun`: 任务执行实例，跟踪执行状态和结果
- `WorkerInfo`: Worker节点注册信息和状态
- `Message`: 统一的消息队列通信格式

### 错误处理

- 统一的`SchedulerError`错误类型
- 标准的`Result<T>`类型别名
- 详细的错误分类和上下文信息

## 开发状态

当前已完成：

- [x] 项目结构和核心接口设置
- [x] 核心数据结构定义
- [x] 基础错误类型和Result类型
- [x] 服务接口trait定义

## 构建项目

```bash
# 检查项目
cargo check

# 构建项目
cargo build

# 运行测试
cargo test
```

## 技术栈

- **语言**: Rust 2021 Edition
- **数据库**: PostgreSQL + SQLx
- **消息队列**: RabbitMQ
- **异步运行时**: Tokio
- **序列化**: Serde
- **配置**: TOML格式
