# 快速开始指南

## 📋 系统要求

### 必需组件
- **Rust**: 1.70+
- **数据库**: PostgreSQL 13+ 或 SQLite
- **消息队列**: RabbitMQ 3.8+ 或 Redis Stream
- **Docker**: 用于依赖服务

### 推荐工具
- **Docker Compose**: 快速启动依赖服务
- **cargo-watch**: 自动重建

## 🛠️ 环境设置

### 1. 安装依赖

```bash
# 安装Rust工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add rustfmt clippy

# 安装开发工具
cargo install cargo-watch
```

### 2. 启动依赖服务

```bash
# 使用Docker Compose启动PostgreSQL和RabbitMQ
docker-compose up -d

# 或者手动启动服务
# PostgreSQL: createdb scheduler_dev
# RabbitMQ: 启动默认配置的RabbitMQ服务
```

### 3. 配置环境变量

```bash
# 复制配置模板
cp config/development.toml config/local.toml

# 编辑配置文件设置数据库连接等
```

## 🚀 运行系统

### 1. 数据库初始化

```bash
# 运行数据库迁移
cargo run --bin migrate
```

### 2. 启动服务

```bash
# 方式一：分别启动各服务
cargo run --bin api &          # API服务 (端口8080)
cargo run --bin dispatcher &   # 调度器
cargo run --bin worker         # Worker节点

# 方式二：开发模式（自动重启）
cargo watch -x "run --bin api"
```

### 3. 验证安装

```bash
# 健康检查
curl http://localhost:8080/health

# 查看API文档
curl http://localhost:8080/api-docs

# 创建测试任务
curl -X POST http://localhost:8080/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test_task",
    "executor": "shell",
    "command": "echo Hello World",
    "schedule": null
  }'
```

## 📝 基本使用

### 创建任务

```bash
# Shell命令任务
curl -X POST http://localhost:8080/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "backup_task",
    "executor": "shell",
    "command": "pg_dump mydb > backup.sql"
  }'

# HTTP请求任务
curl -X POST http://localhost:8080/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "webhook_task",
    "executor": "http",
    "url": "https://api.example.com/webhook",
    "method": "POST"
  }'

# 定时任务（Cron表达式）
curl -X POST http://localhost:8080/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "daily_report",
    "executor": "shell",
    "command": "generate_report.sh",
    "schedule": "0 9 * * *"
  }'
```

### 管理任务

```bash
# 查看任务列表
curl http://localhost:8080/api/tasks

# 查看任务详情
curl http://localhost:8080/api/tasks/{task_id}

# 手动触发任务
curl -X POST http://localhost:8080/api/tasks/{task_id}/trigger

# 暂停任务
curl -X PUT http://localhost:8080/api/tasks/{task_id} \
  -H "Content-Type: application/json" \
  -d '{"status": "paused"}'
```

### 监控系统

```bash
# 系统状态
curl http://localhost:8080/api/system/status

# Worker状态
curl http://localhost:8080/api/workers

# Prometheus指标
curl http://localhost:8080/metrics
```

## 🔧 开发模式

### 实时开发

```bash
# API服务自动重启
cargo watch -x "run --bin api"

# 运行测试
cargo test

# 代码格式检查
cargo fmt
cargo clippy
```

### 配置调试

```bash
# 查看当前配置
cargo run --bin cli config show

# 验证配置文件
cargo run --bin cli config validate
```

## 📊 监控与日志

### 日志查看

```bash
# 查看服务日志
RUST_LOG=info cargo run --bin api

# 结构化日志输出
RUST_LOG=scheduler=debug cargo run --bin dispatcher
```

### 系统监控

- **健康检查**: http://localhost:8080/health
- **系统指标**: http://localhost:8080/metrics
- **API文档**: http://localhost:8080/api-docs

## ❌ 常见问题

### 数据库连接失败

```bash
# 检查PostgreSQL服务状态
docker ps | grep postgres

# 检查配置文件中的数据库URL
cat config/local.toml | grep database
```

### 消息队列连接失败

```bash
# 检查RabbitMQ服务状态
docker ps | grep rabbitmq

# 检查RabbitMQ管理界面
open http://localhost:15672  # guest/guest
```

### 端口占用

```bash
# 检查端口占用
lsof -i :8080

# 使用不同端口
SCHEDULER_API_PORT=8081 cargo run --bin api
```

## 📖 下一步

- 阅读 [系统架构](ARCHITECTURE.md) 了解系统设计
- 查看 [API参考](API.md) 获取完整API文档
- 参考 [开发指南](DEVELOPMENT.md) 进行二次开发