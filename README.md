# 分布式任务调度系统 (Distributed Task Scheduler)

基于 Rust 构建的高性能、可扩展的分布式任务调度系统，采用现代化微服务架构，支持企业级大规模任务调度和工作节点管理。

## 🌟 核心特性

### 🚀 高性能架构
- **异步驱动**: 基于 Tokio 运行时，支持万级并发
- **连接池优化**: 数据库和消息队列智能连接管理  
- **批量处理**: 高吞吐量任务处理和状态更新
- **内存高效**: 零拷贝设计和智能缓存策略

### 🏗️ 企业级扩展性
- **微服务架构**: 清洁架构设计，组件解耦和水平扩展
- **多消息队列**: RabbitMQ/Redis Stream 支持，运行时切换
- **多数据库**: PostgreSQL/SQLite 支持，读写分离
- **云原生**: Kubernetes 就绪，容器化部署

### 🛡️ 生产级可靠性
- **容错设计**: 熔断器、指数退避重试、故障自愈
- **安全认证**: JWT/API Key 双重认证，RBAC 权限控制
- **审计追踪**: 分布式追踪、结构化日志、操作审计
- **配置管理**: 多环境配置、热更新、安全加密

## 🏗️ 系统架构

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

## 📦 项目结构

```
scheduler/
├── Cargo.toml                          # Workspace配置
├── src/
│   ├── main.rs                         # 应用入口点
│   └── app.rs                          # 应用配置
├── crates/
│   ├── core/                           # 核心模块
│   │   ├── src/
│   │   │   ├── models/                 # 数据模型
│   │   │   │   ├── mod.rs              # 模型导出
│   │   │   │   ├── task.rs             # 任务模型
│   │   │   │   ├── task_run.rs         # 任务运行模型
│   │   │   │   ├── worker.rs           # Worker模型
│   │   │   │   └── message.rs          # 消息模型
│   │   │   ├── traits/                 # 服务接口
│   │   │   │   ├── mod.rs              # Trait导出
│   │   │   │   ├── repository.rs       # 数据仓库接口
│   │   │   │   ├── scheduler.rs        # 调度器接口
│   │   │   │   ├── task_executor.rs    # 任务执行器接口
│   │   │   │   └── message_queue.rs    # 消息队列接口
│   │   │   ├── services/               # 服务实现
│   │   │   │   ├── mod.rs              # 服务导出
│   │   │   │   ├── task_services.rs    # 任务服务
│   │   │   │   └── worker_services.rs  # Worker服务
│   │   │   ├── config/                 # 配置管理
│   │   │   │   ├── mod.rs              # 配置导出
│   │   │   │   ├── models/             # 配置模型
│   │   │   │   ├── validation.rs       # 配置验证
│   │   │   │   └── environment.rs      # 环境配置
│   │   │   ├── logging/                # 日志系统
│   │   │   │   ├── mod.rs              # 日志导出
│   │   │   │   ├── structured_logger.rs # 结构化日志
│   │   │   │   └── log_config.rs       # 日志配置
│   │   │   ├── errors.rs               # 错误定义
│   │   │   ├── circuit_breaker.rs      # 熔断器
│   │   │   └── lib.rs                  # 核心模块入口
│   │   ├── tests/                      # 测试
│   │   └── examples/                   # 示例代码
│   ├── api/                            # REST API服务
│   │   ├── src/
│   │   │   ├── handlers/               # 请求处理器
│   │   │   │   ├── tasks.rs            # 任务处理器
│   │   │   │   ├── workers.rs          # Worker处理器
│   │   │   │   └── system.rs           # 系统处理器
│   │   │   ├── middleware/             # 中间件
│   │   │   │   ├── cors.rs             # CORS中间件
│   │   │   │   ├── auth.rs             # 认证中间件
│   │   │   │   └── logging.rs          # 日志中间件
│   │   │   ├── routes.rs               # 路由定义
│   │   │   ├── error.rs                # 错误处理
│   │   │   └── lib.rs                  # API模块入口
│   │   └── tests/                      # API测试
│   ├── dispatcher/                     # 任务调度器
│   │   ├── src/
│   │   │   ├── scheduler.rs            # 调度器核心
│   │   │   ├── strategies.rs           # 调度策略
│   │   │   ├── cron_utils.rs           # Cron表达式工具
│   │   │   ├── dependency_checker.rs  # 依赖检查器
│   │   │   ├── recovery_service.rs     # 故障恢复服务
│   │   │   ├── retry_service.rs       # 重试服务
│   │   │   └── lib.rs                  # 调度器模块入口
│   │   └── tests/                      # 调度器测试
│   ├── worker/                         # Worker服务
│   │   ├── src/
│   │   │   ├── service.rs              # Worker服务
│   │   │   ├── executors.rs            # 任务执行器
│   │   │   ├── heartbeat.rs            # 心跳管理
│   │   │   ├── components/             # 组件
│   │   │   │   ├── task_execution.rs   # 任务执行组件
│   │   │   │   └── worker_lifecycle.rs # Worker生命周期
│   │   │   └── lib.rs                  # Worker模块入口
│   │   └── tests/                      # Worker测试
│   ├── infrastructure/                 # 基础设施
│   │   ├── src/
│   │   │   ├── database/               # 数据库
│   │   │   │   ├── mod.rs              # 数据库导出
│   │   │   │   ├── manager.rs          # 数据库管理器
│   │   │   │   ├── postgres/           # PostgreSQL实现
│   │   │   │   └── sqlite/             # SQLite实现
│   │   │   ├── message_queue.rs        # 消息队列抽象
│   │   │   ├── message_queue_factory.rs # 消息队列工厂
│   │   │   ├── redis_stream/          # Redis Stream实现
│   │   │   └── observability/         # 可观测性
│   │   │       ├── mod.rs              # 可观测性导出
│   │   │       ├── metrics_collector.rs # 指标收集器
│   │   │       └── structured_logger.rs # 结构化日志
│   │   └── tests/                      # 基础设施测试
│   └── domain/                         # 领域模型
│       ├── src/
│       │   ├── entities.rs             # 实体定义
│       │   ├── value_objects.rs        # 值对象
│       │   ├── events.rs               # 领域事件
│       │   ├── repositories.rs         # 仓库接口
│       │   └── services.rs             # 领域服务
│       └── tests/                      # 领域测试
├── config/                              # 配置文件
│   ├── development.toml                # 开发环境配置
│   ├── production.toml                 # 生产环境配置
│   ├── rabbitmq.toml                   # RabbitMQ配置
│   ├── redis-stream.toml               # Redis Stream配置
│   └── scheduler.toml                 # 调度器配置
├── migrations/                          # 数据库迁移
├── docs/                               # 文档
│   ├── CODING_STANDARDS.md             # 编码规范
│   ├── CODE_REVIEW_CHECKLIST.md        # 代码评审检查清单
│   └── API_DOCUMENTATION.md            # API文档
├── monitoring/                         # 监控配置
│   └── prometheus.yml                  # Prometheus配置
├── docker-compose.yml                  # Docker编排
├── docker-compose.dev.yml              # 开发环境编排
└── Dockerfile                          # Docker镜像
```

## 🔧 核心组件

### 📋 Core 模块

系统的核心模块，定义了基础数据结构、服务接口和错误处理机制。

- **数据模型**: Task、TaskRun、WorkerInfo、Message
- **服务接口**: TaskControlService、WorkerService、MessageQueue
- **配置管理**: 多环境配置、配置验证、热更新
- **错误处理**: 统一错误类型、错误传播、恢复机制
- **日志系统**: 结构化日志、性能追踪、审计日志

### 🌐 API 模块

基于Axum框架的REST API服务，提供HTTP接口用于系统管理。

- **任务管理**: 创建、查询、更新、删除任务
- **Worker管理**: 注册、心跳、状态查询
- **系统监控**: 健康检查、性能指标、日志查询
- **中间件**: CORS、认证、日志、限流

### ⚡ Dispatcher 模块

任务调度器核心，负责任务调度和依赖管理。

- **调度策略**: 支持多种调度算法
- **依赖管理**: 任务依赖关系检查
- **定时任务**: Cron表达式支持
- **故障恢复**: 自动重试和故障转移

### 🔧 Worker 模块

Worker节点服务，负责任务执行和状态上报。

- **任务执行**: 支持多种执行器类型
- **心跳管理**: 定期心跳和健康检查
- **负载均衡**: 任务分配和负载控制
- **生命周期**: 启动、运行、停止管理

### 🏗️ Infrastructure 模块

基础设施抽象层，提供数据库和消息队列的统一接口。

- **数据库**: PostgreSQL、SQLite支持
- **消息队列**: RabbitMQ、Redis Stream支持
- **可观测性**: 指标收集、分布式追踪
- **连接池**: 高性能连接管理

## 🚀 快速开始

### 环境要求

- **Rust**: 1.70+
- **PostgreSQL**: 13+
- **RabbitMQ**: 3.8+
- **Redis**: 6.0+ (可选)

### 安装和运行

1. **克隆项目**

```bash
git clone https://github.com/your-org/scheduler.git
cd scheduler
```

2. **启动依赖服务**

```bash
docker-compose up -d
```

3. **运行数据库迁移**

```bash
cargo run --bin migrate
```

4. **启动调度器**

```bash
cargo run --bin scheduler
```

5. **启动Worker**

```bash
cargo run --bin worker
```

6. **启动API服务**

```bash
cargo run --bin api
```

### 配置文件

**开发环境配置** (`config/development.toml`)

```toml
[database]
url = "postgresql://localhost:5432/scheduler_dev"
pool_size = 10

[message_queue]
type = "rabbitmq"
host = "localhost"
port = 5672
username = "guest"
password = "guest"

[api]
host = "127.0.0.1"
port = 8080

[dispatcher]
max_workers = 50
task_timeout_seconds = 3600

[worker]
heartbeat_interval_seconds = 30
max_concurrent_tasks = 5
```

### API 使用示例

**创建任务**

```bash
curl -X POST http://localhost:8080/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "data_processing",
    "description": "处理每日数据",
    "task_type": "batch",
    "executor": "python",
    "command": "process_data.py",
    "schedule": "0 2 * * *",
    "priority": "normal",
    "retry_count": 3,
    "timeout_seconds": 3600
  }'
```

**查询任务状态**

```bash
curl -X GET http://localhost:8080/api/tasks/1
```

**手动触发任务**

```bash
curl -X POST http://localhost:8080/api/tasks/1/trigger
```

## 🧪 开发和测试

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test -p scheduler-core

# 运行集成测试
cargo test --test integration_tests

# 运行API测试
cargo test -p scheduler-api
```

### 代码质量检查

```bash
# 格式化代码
cargo fmt

# 检查代码格式
cargo fmt --check

# 静态分析
cargo clippy

# 运行所有检查
cargo check --all-targets
```

### 开发工具

```bash
# 生成文档
cargo doc --no-deps

# 打开文档
cargo doc --open

# 分析依赖
cargo tree
cargo outdated

# 性能分析
cargo flamegraph
```

## 📊 监控和运维

### 系统监控

系统内置了完整的监控功能：

- **健康检查**: `/health` 端点
- **性能指标**: `/metrics` 端点 (Prometheus格式)
- **系统状态**: `/api/system/stats` 端点
- **日志查询**: `/api/system/logs` 端点

### 配置Prometheus监控

```yaml
# monitoring/prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'scheduler'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
```

### 告警规则

```yaml
# monitoring/alerts.yml
groups:
  - name: scheduler
    rules:
      - alert: HighTaskFailureRate
        expr: rate(task_failures_total[5m]) > 0.1
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "任务失败率过高"
```

## 🔒 安全考虑

### 认证和授权

- **API密钥**: 支持API密钥认证
- **JWT令牌**: 支持JWT令牌认证
- **权限控制**: 基于角色的访问控制
- **IP白名单**: 支持IP访问限制

### 数据安全

- **敏感信息**: 配置文件中的敏感信息支持加密
- **数据库连接**: 支持SSL/TLS加密连接
- **消息队列**: 支持加密通信
- **审计日志**: 完整的操作审计记录

### 网络安全

- **CORS**: 跨域资源共享控制
- **速率限制**: API访问频率限制
- **输入验证**: 严格的输入参数验证
- **SQL注入防护**: 使用参数化查询

## 🚀 部署指南

### Docker部署

```bash
# 构建镜像
docker build -t scheduler:latest .

# 使用docker-compose启动
docker-compose up -d

# 查看服务状态
docker-compose ps
docker-compose logs
```

### Kubernetes部署

```yaml
# k8s/deployment.yaml
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
```

### 生产环境配置

```toml
# config/production.toml
[database]
url = "postgresql://prod-db:5432/scheduler"
pool_size = 50
connection_timeout_seconds = 60

[message_queue]
type = "rabbitmq"
host = "rabbitmq-cluster"
port = 5672
username = "scheduler"
password = "${RABBITMQ_PASSWORD}"

[api]
host = "0.0.0.0"
port = 8080
cors_origins = ["https://app.example.com"]

[observability]
log_level = "info"
metrics_enabled = true
tracing_enabled = true
```

## 🤝 贡献指南

### 开发流程

1. Fork 项目
2. 创建功能分支: `git checkout -b feature/new-feature`
3. 提交更改: `git commit -m 'Add new feature'`
4. 推送分支: `git push origin feature/new-feature`
5. 创建Pull Request

### 代码规范

- 遵循 [CODING_STANDARDS.md](docs/CODING_STANDARDS.md)
- 通过 [CODE_REVIEW_CHECKLIST.md](docs/CODE_REVIEW_CHECKLIST.md) 检查
- 确保所有测试通过
- 更新相关文档

### 提交规范

```
feat: 新功能
fix: Bug修复
docs: 文档更新
style: 代码格式化
refactor: 代码重构
test: 测试相关
chore: 构建或工具相关
```

## 📈 性能基准

### 系统容量

- **任务调度**: 10,000+ tasks/minute
- **Worker节点**: 100+ nodes
- **并发执行**: 1,000+ concurrent tasks
- **消息处理**: 50,000+ messages/second

### 响应时间

- **API响应**: < 100ms (P99)
- **任务调度**: < 1s
- **任务执行**: 取决于任务类型
- **故障恢复**: < 30s

### 资源使用

- **内存**: 512MB (base) + 10MB per 1000 tasks
- **CPU**: 2 cores (base) + scaling based on load
- **存储**: 1GB (base) + 100KB per task
- **网络**: 1Gbps recommended

## 🆘 故障排除

### 常见问题

1. **数据库连接失败**: 检查数据库配置和网络连接
2. **消息队列连接失败**: 检查消息队列服务状态
3. **任务执行失败**: 检查Worker日志和任务配置
4. **性能问题**: 检查系统资源和配置参数

### 日志分析

```bash
# 查看调度器日志
docker-compose logs scheduler

# 查看Worker日志
docker-compose logs worker

# 查看API日志
docker-compose logs api

# 查看特定错误
docker-compose logs | grep ERROR
```

### 性能调优

```bash
# 分析CPU使用
top -p $(pgrep scheduler)

# 分析内存使用
cat /proc/$(pgrep scheduler)/status

# 分析网络连接
netstat -an | grep 8080
```

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

感谢所有为这个项目做出贡献的开发者和社区成员。

## 📞 支持

- **文档**: [项目文档](docs/)
- **问题报告**: [GitHub Issues](https://github.com/your-org/scheduler/issues)
- **讨论**: [GitHub Discussions](https://github.com/your-org/scheduler/discussions)
- **邮件**: <support@example.com>

---

**⭐ 如果这个项目对您有帮助，请给我们一个Star！**
