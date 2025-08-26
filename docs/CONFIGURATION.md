# 配置指南

## 📋 配置概述

系统采用分层配置管理，支持多环境部署和配置热更新。配置文件使用TOML格式，支持环境变量注入。

## 📁 配置文件结构

```
config/
├── base.toml           # 基础配置（所有环境共享）
├── development.toml    # 开发环境配置
├── staging.toml        # 测试环境配置
├── production.toml     # 生产环境配置
└── timeouts.toml      # 超时配置
```

## ⚙️ 基础配置

### 基本设置 (base.toml)

```toml
[app]
name = "scheduler"
version = "1.0.0"
environment = "development"

[api]
host = "0.0.0.0"
port = 8080
timeout_seconds = 30
max_connections = 1000

[database]
url = "postgresql://localhost/scheduler_${APP_ENV:-dev}"
max_connections = 10
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600

[message_queue]
type = "rabbitmq"  # rabbitmq | redis_stream

[message_queue.rabbitmq]
url = "amqp://guest:guest@localhost:5672/%2F"
exchange = "scheduler_exchange"
queue = "task_queue"
prefetch_count = 10

[message_queue.redis_stream]
url = "redis://localhost:6379"
stream_name = "tasks"
consumer_group = "scheduler"

[worker]
heartbeat_interval_seconds = 30
max_concurrent_tasks = 10
task_timeout_seconds = 3600

[observability]
enable_metrics = true
enable_tracing = true
metrics_port = 9090
log_level = "info"
```

## 🌍 环境配置

### 开发环境 (development.toml)

```toml
[database]
url = "sqlite:./scheduler_dev.db"

[api]
port = 8080
cors_enabled = true

[observability]
log_level = "debug"
enable_request_logging = true
```

### 生产环境 (production.toml)

```toml
[database] 
url = "${DATABASE_URL}"
max_connections = 50
connection_timeout_seconds = 10

[api]
port = "${PORT:-8080}"
cors_enabled = false
rate_limit_enabled = true
rate_limit_per_minute = 1000

[message_queue]
type = "${MESSAGE_QUEUE_TYPE:-rabbitmq}"

[message_queue.rabbitmq]
url = "${RABBITMQ_URL}"
prefetch_count = 100

[worker]
max_concurrent_tasks = "${WORKER_MAX_TASKS:-20}"

[observability]
log_level = "info"
enable_metrics = true
metrics_endpoint = "${METRICS_ENDPOINT}"

[security]
api_keys_file = "${API_KEYS_FILE}"
jwt_secret = "${JWT_SECRET}"
```

## 🔐 安全配置

### API密钥配置

创建 `config/api_keys.toml`:

```toml
# 管理员密钥
[api_keys."admin_key_hash"]
name = "admin"
permissions = ["Admin"]
is_active = true
created_at = "2024-01-01T00:00:00Z"

# 操作员密钥
[api_keys."operator_key_hash"]
name = "operator" 
permissions = ["TaskRead", "TaskWrite", "WorkerRead"]
is_active = true
created_at = "2024-01-01T00:00:00Z"

# 只读密钥
[api_keys."readonly_key_hash"]
name = "monitor"
permissions = ["TaskRead", "WorkerRead", "SystemRead"]
is_active = true
created_at = "2024-01-01T00:00:00Z"
```

### JWT配置

```toml
[security.jwt]
secret = "${JWT_SECRET}"
expiration_hours = 24
issuer = "scheduler-api"
algorithm = "HS256"
```

## 🎯 配置加载

### 环境变量

```bash
# 设置运行环境
export SCHEDULER_ENV=production

# 数据库连接
export DATABASE_URL="postgresql://user:pass@localhost/scheduler_prod"

# 消息队列
export RABBITMQ_URL="amqp://user:pass@localhost:5672/"

# 安全配置
export JWT_SECRET="your-secret-key"
export API_KEYS_FILE="/etc/scheduler/api_keys.toml"
```

### 配置优先级

配置加载顺序（后者覆盖前者）：

1. **基础配置** (`base.toml`)
2. **环境配置** (`${SCHEDULER_ENV}.toml`)
3. **环境变量** (`${VAR_NAME}`)
4. **命令行参数**

### 配置验证

```bash
# 验证配置文件
cargo run --bin cli config validate

# 查看当前配置
cargo run --bin cli config show

# 测试配置连接
cargo run --bin cli config test-connections
```

## 🚀 部署配置

### Docker环境变量

```bash
# docker-compose.yml
version: '3.8'
services:
  scheduler-api:
    image: scheduler:latest
    environment:
      - SCHEDULER_ENV=production
      - DATABASE_URL=postgresql://postgres:password@db:5432/scheduler
      - RABBITMQ_URL=amqp://rabbitmq:5672/
      - JWT_SECRET=production-secret-key
      - RUST_LOG=scheduler=info
    ports:
      - "8080:8080"
```

### Kubernetes ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: scheduler-config
data:
  SCHEDULER_ENV: "production"
  DATABASE_URL: "postgresql://postgres:password@db:5432/scheduler"
  RABBITMQ_URL: "amqp://rabbitmq:5672/"
  RUST_LOG: "scheduler=info"
---
apiVersion: v1
kind: Secret
metadata:
  name: scheduler-secrets
type: Opaque
stringData:
  JWT_SECRET: "your-production-jwt-secret"
```

## ⚡ 性能调优

### 数据库优化

```toml
[database]
max_connections = 50        # 根据CPU核数调整
min_connections = 5         # 保持最小连接池
connection_timeout_seconds = 10
idle_timeout_seconds = 300
max_lifetime_seconds = 3600

# 连接池配置
acquire_timeout_seconds = 30
statement_cache_capacity = 100
```

### 消息队列优化

```toml
[message_queue.rabbitmq]
prefetch_count = 100        # 批量获取消息
connection_timeout_seconds = 10
heartbeat_interval_seconds = 60
channel_max = 2047

[message_queue.redis_stream]
max_length = 10000          # 流最大长度
batch_size = 50             # 批量处理大小
read_timeout_seconds = 5
```

### Worker性能

```toml
[worker]
max_concurrent_tasks = 20   # 根据资源调整
heartbeat_interval_seconds = 30
task_timeout_seconds = 3600
queue_size = 1000
worker_threads = 4          # 工作线程数
```

## 📊 监控配置

### Prometheus指标

```toml
[observability.metrics]
enabled = true
port = 9090
path = "/metrics"
update_interval_seconds = 15

# 自定义指标
[observability.metrics.custom]
business_metrics_enabled = true
detailed_task_metrics = true
worker_performance_metrics = true
```

### 日志配置

```toml
[observability.logging]
level = "info"
format = "json"             # json | pretty
output = "stdout"           # stdout | file
file_path = "/var/log/scheduler.log"
max_file_size_mb = 100
max_files = 10
```

### 分布式追踪

```toml
[observability.tracing]
enabled = true
service_name = "scheduler"
jaeger_endpoint = "http://jaeger:14268/api/traces"
sample_rate = 0.1           # 采样率 10%
```

## ❌ 故障排除

### 常见配置问题

**数据库连接失败**
```bash
# 检查数据库URL格式
# 正确: postgresql://user:pass@host:port/database  
# 错误: postgres://user:pass@host:port/database

# 测试连接
psql "postgresql://user:pass@host:port/database"
```

**消息队列连接失败**  
```bash
# 检查RabbitMQ状态
docker logs rabbitmq-container

# 检查连接URL格式
# 正确: amqp://user:pass@host:port/vhost
# 错误: rabbitmq://user:pass@host:port/
```

**配置文件语法错误**
```bash
# 验证TOML语法
cargo run --bin cli config validate

# 查看具体错误信息
RUST_LOG=debug cargo run --bin api
```

### 配置热更新

```bash
# 发送SIGHUP信号重新加载配置
kill -HUP $(pgrep scheduler-api)

# 或通过API重新加载
curl -X POST http://localhost:8080/api/system/reload-config \
  -H "Authorization: Bearer admin-key"
```