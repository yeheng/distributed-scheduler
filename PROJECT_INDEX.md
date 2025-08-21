# 分布式任务调度系统 - 项目文档索引

## 📋 概述

本文档提供分布式任务调度系统的完整文档导航和知识库索引。系统基于Rust构建，采用现代化微服务架构，支持高性能、可扩展的任务调度和Worker管理。

## 🏗️ 项目架构概览

### 核心组件

```
scheduler/
├── 🌐 API层 (scheduler-api)          # REST API服务
├── ⚡ 调度器 (scheduler-dispatcher)   # 任务调度核心  
├── 👷 Worker (scheduler-worker)      # 任务执行节点
├── 🏗️ 应用层 (scheduler-application) # 业务服务
├── 🔧 基础设施 (scheduler-infrastructure) # 数据库/消息队列
├── 📦 领域层 (scheduler-domain)      # 业务模型
├── ⚙️ 配置 (scheduler-config)        # 配置管理
├── 📊 可观测性 (scheduler-observability) # 监控日志
├── 🔗 核心 (scheduler-core)          # 依赖注入
└── ❌ 错误 (scheduler-errors)        # 错误处理
```

### 技术栈

- **语言**: Rust 1.70+
- **Web框架**: Axum 0.8
- **数据库**: PostgreSQL 13+ / SQLite
- **消息队列**: RabbitMQ 3.8+ / Redis Stream
- **缓存**: Redis 6.0+
- **监控**: Prometheus + OpenTelemetry
- **容器**: Docker + Kubernetes

## 📚 核心文档

### 🏛️ 架构设计

| 文档 | 描述 | 最后更新 |
|------|------|----------|
| [系统架构](docs/ARCHITECTURE.md) | 完整架构设计文档，包含Crate依赖图、数据流、部署架构 | 2025-08-13 |
| [编码规范](docs/CODING_STANDARDS.md) | Rust编码标准、异步编程规范、文档注释规范 | - |
| [代码评审清单](docs/CODE_REVIEW_CHECKLIST.md) | 代码评审检查要点和质量标准 | - |

### 🔌 API文档

| 文档 | 描述 | 涵盖范围 |
|------|------|----------|
| [API文档](docs/API_DOCUMENTATION.md) | 完整REST API规范 | 任务管理、Worker管理、系统监控、认证授权 |
| [认证文档](docs/AUTHENTICATION.md) | 安全认证实现 | API密钥、JWT令牌、权限控制 |

### ⚙️ 配置与安全

| 文档 | 描述 | 重点内容 |
|------|------|----------|
| [配置安全设置](docs/CONFIG_SECURITY_SETUP.md) | 安全配置指南 | 敏感信息加密、安全策略 |

### 📖 用户指南

| 文档 | 描述 | 适用对象 |
|------|------|----------|
| [README](README.md) | 项目概览和快速开始 | 新用户、开发者 |
| [Claude使用指南](CLAUDE.md) | Claude Code开发指导 | AI辅助开发 |
| [任务清单](tasks.md) | 项目TODO和开发计划 | 开发团队 |

## 🏗️ 代码架构导航

### 核心模块结构

```
crates/
├── 🔗 core/                    # 依赖注入容器和服务抽象
│   ├── src/lib.rs             # 容器定义和服务注册
│   └── src/di.rs              # 依赖注入实现
├── 📦 domain/                  # 领域模型和业务逻辑
│   ├── src/entities.rs        # 业务实体: Task, TaskRun, WorkerInfo
│   ├── src/repositories.rs    # 仓储接口定义
│   ├── src/services.rs        # 领域服务接口
│   └── src/value_objects.rs   # 值对象定义
├── 🏗️ application/            # 应用服务层
│   ├── src/services/          # 业务用例实现
│   ├── src/interfaces/        # 服务接口定义
│   └── src/ports/            # 端口适配器
├── 🔧 infrastructure/         # 基础设施实现
│   ├── src/database/         # 数据库实现 (PostgreSQL/SQLite)
│   ├── src/message_queue.rs  # 消息队列抽象
│   ├── src/redis_stream/     # Redis Stream实现
│   └── src/cache/           # 缓存管理
├── 🌐 api/                    # HTTP API接口
│   ├── src/handlers/         # 请求处理器
│   ├── src/middleware.rs     # 中间件 (认证/CORS/限流)
│   └── src/routes.rs         # 路由定义
├── ⚡ dispatcher/             # 任务调度器
│   ├── src/scheduler.rs      # 调度器核心
│   ├── src/strategies.rs     # 调度策略
│   ├── src/dependency_checker.rs # 依赖检查器
│   └── src/recovery_service.rs   # 故障恢复
├── 👷 worker/                 # 任务执行器
│   ├── src/service.rs        # Worker服务
│   ├── src/executors.rs      # 任务执行器实现
│   └── src/components/       # Worker组件
├── ⚙️ config/                 # 配置管理
│   ├── src/models/           # 配置模型
│   ├── src/validation.rs     # 配置验证
│   └── src/security.rs       # 安全配置
├── 📊 observability/          # 可观测性
│   ├── src/metrics_collector.rs # 指标收集
│   ├── src/structured_logger.rs # 结构化日志
│   └── src/cross_component_tracer.rs # 分布式追踪
└── ❌ errors/                 # 错误处理
    └── src/lib.rs            # 统一错误类型定义
```

## 🚀 快速开始导航

### 开发环境设置

1. **环境要求**
   - Rust 1.70+
   - PostgreSQL 13+
   - RabbitMQ 3.8+
   - Redis 6.0+ (可选)

2. **快速启动**
   ```bash
   # 启动依赖服务
   docker-compose up -d
   
   # 运行数据库迁移
   cargo run --bin migrate
   
   # 启动服务 (并行启动)
   cargo run --bin scheduler &
   cargo run --bin worker &
   cargo run --bin api
   ```

3. **验证安装**
   ```bash
   # 健康检查
   curl http://localhost:8080/health
   
   # 创建测试任务
   curl -X POST http://localhost:8080/api/tasks \
     -H "Content-Type: application/json" \
     -d '{"name":"test","executor":"shell","command":"echo hello"}'
   ```

### API使用快速参考

| 操作 | 方法 | 端点 | 说明 |
|------|------|------|------|
| 创建任务 | POST | `/api/tasks` | 创建新任务 |
| 查询任务 | GET | `/api/tasks` | 获取任务列表 |
| 任务详情 | GET | `/api/tasks/{id}` | 获取任务详情 |
| 触发任务 | POST | `/api/tasks/{id}/trigger` | 手动触发执行 |
| Worker列表 | GET | `/api/workers` | 获取Worker状态 |
| 系统健康 | GET | `/api/system/health` | 系统健康检查 |
| 系统指标 | GET | `/api/system/metrics` | Prometheus指标 |

## 🧪 测试与质量

### 测试结构

```
tests/
├── integration/               # 集成测试
│   ├── api_tests.rs          # API接口测试
│   ├── database_tests.rs     # 数据库集成测试
│   └── message_queue_tests.rs # 消息队列测试
├── unit/                     # 单元测试
└── benchmarks/               # 性能基准测试
```

### 质量检查命令

```bash
# 完整测试套件
cargo test --all

# 代码格式检查
cargo fmt --check

# 静态分析
cargo clippy -- -D warnings

# 性能基准测试
cargo bench

# 生成文档
cargo doc --no-deps --open
```

## 📊 监控与运维

### 监控端点

| 端点 | 描述 | 格式 |
|------|------|------|
| `/health` | 健康检查 | JSON |
| `/metrics` | Prometheus指标 | Prometheus |
| `/api/system/stats` | 系统统计 | JSON |
| `/api/system/logs` | 系统日志 | JSON |

### 关键指标

- **任务指标**: 创建数、完成数、失败数、平均执行时间
- **Worker指标**: 注册数、活跃数、负载率、心跳状态
- **系统指标**: CPU使用率、内存使用、数据库连接池、消息队列深度
- **业务指标**: 吞吐量、成功率、响应时间

## 🔧 开发工具链

### 推荐开发工具

- **IDE**: VS Code + rust-analyzer
- **调试**: rust-gdb, tokio-console
- **性能分析**: cargo flamegraph, perf
- **依赖管理**: cargo-outdated, cargo-audit

### 开发工作流

1. **功能开发**
   ```bash
   git checkout -b feature/new-feature
   cargo check --all-targets
   cargo test
   cargo clippy
   ```

2. **代码提交**
   ```bash
   cargo fmt
   git add .
   git commit -m "feat: 新功能描述"
   ```

3. **集成测试**
   ```bash
   docker-compose up -d
   cargo test --test integration_tests
   ```

## 🗂️ 配置文件导航

### 配置文件结构

```
config/
├── development.toml          # 开发环境配置
├── production.toml           # 生产环境配置
├── staging-template.toml     # 测试环境模板
├── security-template.toml    # 安全配置模板
├── scheduler.toml           # 调度器配置
├── rabbitmq.toml           # RabbitMQ配置
├── redis-stream.toml       # Redis Stream配置
├── auth_example.toml       # 认证配置示例
└── timeout_example.toml    # 超时配置示例
```

### 配置优先级

1. 环境变量 (最高优先级)
2. 命令行参数
3. 配置文件
4. 默认值 (最低优先级)

## 🚦 部署指南

### 容器化部署

```bash
# Docker构建
docker build -t scheduler:latest .

# Docker Compose部署
docker-compose up -d

# 健康检查
docker-compose ps
docker-compose logs scheduler
```

### Kubernetes部署

```bash
# 部署到K8s集群
kubectl apply -f k8s/

# 检查部署状态
kubectl get pods -l app=scheduler
kubectl logs -f deployment/scheduler
```

## 🔍 故障排除

### 常见问题解决

| 问题 | 可能原因 | 解决方案 |
|------|----------|----------|
| 数据库连接失败 | 配置错误/网络问题 | 检查DATABASE_URL配置 |
| 任务执行失败 | Worker不可用 | 检查Worker状态和日志 |
| API响应慢 | 数据库性能问题 | 检查连接池配置和索引 |
| 消息队列错误 | RabbitMQ连接问题 | 检查消息队列服务状态 |

### 日志分析

```bash
# 查看特定服务日志
docker-compose logs scheduler
docker-compose logs worker

# 查看错误日志
docker-compose logs | grep ERROR

# 实时日志监控
docker-compose logs -f
```

## 📈 性能基准

### 系统容量指标

- **任务调度**: 10,000+ tasks/minute
- **Worker节点**: 100+ concurrent workers
- **并发执行**: 1,000+ concurrent tasks
- **API响应**: <100ms (P99)
- **消息处理**: 50,000+ messages/second

### 资源需求

- **内存**: 512MB基础 + 10MB/1000任务
- **CPU**: 2核心基础 + 负载自适应
- **存储**: 1GB基础 + 100KB/任务
- **网络**: 建议1Gbps

## 🤝 贡献指南

### 代码贡献流程

1. Fork项目
2. 创建功能分支: `git checkout -b feature/new-feature`
3. 遵循[编码规范](docs/CODING_STANDARDS.md)
4. 通过[代码评审清单](docs/CODE_REVIEW_CHECKLIST.md)
5. 提交Pull Request

### 文档贡献

- API文档更新遵循OpenAPI 3.0规范
- 架构文档使用PlantUML图表
- 代码注释遵循rustdoc标准

---

## 📞 支持资源

- **项目仓库**: [GitHub Repository]
- **问题报告**: [GitHub Issues] 
- **文档站点**: [Project Documentation]
- **社区讨论**: [GitHub Discussions]

---

**最后更新**: 2025-08-21  
**文档版本**: v2.0  
**系统版本**: v1.0.0

*本索引文档为分布式任务调度系统的完整导航指南，涵盖架构设计、开发指南、部署运维等全生命周期内容。*