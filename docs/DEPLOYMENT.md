# 分布式任务调度系统部署指南

## 概览

本文档提供了分布式任务调度系统的完整部署指南，包括开发环境搭建、生产环境部署、配置管理、监控设置等内容。

## 快速开始

### 前置条件
- Docker 20.10+
- Docker Compose 1.29+
- Rust 1.75+ (本地开发)
- PostgreSQL 15+ (生产环境)

### 1. 克隆项目
```bash
git clone <repository-url>
cd job
```

### 2. 开发环境快速启动
```bash
# 启动依赖服务
docker-compose -f docker-compose.dev.yml up -d

# 等待服务就绪
docker-compose -f docker-compose.dev.yml ps

# 本地运行应用
export ENVIRONMENT=development
cargo run --bin scheduler -- --mode all
```

### 3. 完整容器化部署
```bash
# 构建并启动所有服务
docker-compose up -d

# 查看服务状态
docker-compose ps

# 访问 API 服务
curl http://localhost:8081/health
```

## 配置管理

### 配置文件结构
```
config/
├── base.toml           # 基础默认配置
├── development.toml    # 开发环境配置
├── staging.toml        # 测试环境配置
├── production.toml     # 生产环境配置
├── auth.toml          # 认证配置
├── security.toml      # 安全配置
└── timeouts.toml      # 超时配置
```

### 基础配置说明

#### 数据库配置
```toml
[database]
url = "postgresql://localhost:5432/scheduler"
max_connections = 10
min_connections = 1
connection_timeout_seconds = 30
idle_timeout_seconds = 600
```

#### 消息队列配置
```toml
[message_queue]
type = "redis_stream"  # 或 "rabbitmq"
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
max_retries = 3
retry_delay_seconds = 60

# Redis Stream 配置
[message_queue.redis]
host = "localhost"
port = 6379
database = 0
connection_timeout_seconds = 30
```

#### 服务配置
```toml
[dispatcher]
enabled = true
schedule_interval_seconds = 10
max_concurrent_dispatches = 100
worker_timeout_seconds = 90

[worker]
enabled = false
max_concurrent_tasks = 5
supported_task_types = ["shell", "http"]
heartbeat_interval_seconds = 30

[api]
enabled = true
bind_address = "127.0.0.1:8080"
cors_enabled = true
cors_origins = ["http://localhost:3000"]
```

### 环境变量支持

系统支持通过环境变量覆盖配置：

```bash
# 数据库配置
export DATABASE_URL="postgresql://user:pass@host:port/db"
export DATABASE_MAX_CONNECTIONS=20

# 消息队列配置
export MESSAGE_QUEUE_URL="amqp://user:pass@host:port"
export MESSAGE_QUEUE_TYPE="rabbitmq"

# 日志配置
export RUST_LOG=info
export LOG_FORMAT=json

# 认证配置
export JWT_SECRET="your-secret-key"
export API_KEYS='{"key1": ["Admin"], "key2": ["TaskRead", "TaskWrite"]}'
```

### 配置文件中使用环境变量
```toml
# 配置文件支持环境变量替换
database_url = "${DATABASE_URL:-postgresql://localhost:5432/scheduler}"
worker_id = "${WORKER_ID:-default-worker-001}"
```

## 部署模式

### 开发环境部署

#### 1. 仅依赖服务
```bash
# 启动 PostgreSQL 和 RabbitMQ
docker-compose -f docker-compose.dev.yml up -d

# 本地运行应用
cargo run --bin scheduler -- --config config/development.toml --mode all
```

#### 2. 分离服务模式
```bash
# 终端1: 启动 API 服务
cargo run --bin scheduler-api

# 终端2: 启动调度器
cargo run --bin scheduler-dispatcher

# 终端3: 启动工作节点
cargo run --bin scheduler-worker -- --worker-id dev-worker-1
```

### 生产环境部署

#### 1. 单节点部署
```bash
# 使用生产配置
export ENVIRONMENT=production
docker-compose -f docker-compose.yml up -d

# 验证服务状态
docker-compose ps
curl http://localhost:8080/health
```

#### 2. 分布式部署

**步骤 1: 基础设施准备**
```bash
# 启动数据库集群
docker-compose -f docker-compose.infrastructure.yml up -d

# 等待数据库初始化完成
./scripts/wait-for-db.sh
```

**步骤 2: 核心服务部署**
```bash
# 部署调度器服务
docker-compose -f docker-compose.scheduler.yml up -d

# 部署 API 服务
docker-compose -f docker-compose.api.yml up -d

# 部署工作节点
docker-compose -f docker-compose.workers.yml up -d
```

**步骤 3: 监控服务部署**
```bash
# 启动监控栈
docker-compose --profile monitoring up -d

# 访问监控面板
# Prometheus: http://localhost:9090
# Grafana: http://localhost:3000 (admin/admin)
```

### Kubernetes 部署

#### 1. 准备配置
```bash
# 创建 namespace
kubectl create namespace scheduler

# 创建配置映射
kubectl create configmap scheduler-config \
  --from-file=config/production.toml \
  --namespace=scheduler

# 创建 secret
kubectl create secret generic scheduler-secrets \
  --from-literal=DATABASE_URL="postgresql://user:pass@db:5432/scheduler" \
  --from-literal=JWT_SECRET="your-jwt-secret" \
  --namespace=scheduler
```

#### 2. 部署服务
```bash
# 部署数据库
kubectl apply -f k8s/database.yaml

# 部署消息队列
kubectl apply -f k8s/rabbitmq.yaml

# 部署核心服务
kubectl apply -f k8s/api.yaml
kubectl apply -f k8s/dispatcher.yaml
kubectl apply -f k8s/worker.yaml

# 部署监控
kubectl apply -f k8s/monitoring.yaml
```

#### 3. 示例 Kubernetes 配置
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: scheduler-api
  namespace: scheduler
spec:
  replicas: 3
  selector:
    matchLabels:
      app: scheduler-api
  template:
    metadata:
      labels:
        app: scheduler-api
    spec:
      containers:
      - name: api
        image: scheduler:latest
        command: ["scheduler", "--mode", "api"]
        ports:
        - containerPort: 8080
        env:
        - name: RUST_LOG
          value: "info"
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: scheduler-secrets
              key: DATABASE_URL
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

## 服务二进制

系统提供 5 个主要二进制文件：

### 1. scheduler (主程序)
```bash
# 所有模式运行
./scheduler --mode all

# 特定模式运行
./scheduler --mode api           # 仅 API 服务
./scheduler --mode dispatcher    # 仅调度器
./scheduler --mode worker        # 仅工作节点

# 指定配置文件
./scheduler --config config/production.toml --mode api

# 指定工作节点ID
./scheduler --mode worker --worker-id worker-001
```

### 2. 独立服务二进制
```bash
# API 服务
./scheduler-api --config config/api.toml

# 调度器服务  
./scheduler-dispatcher --config config/dispatcher.toml

# 工作节点服务
./scheduler-worker --worker-id worker-001

# CLI 工具
./scheduler-cli task list
./scheduler-cli worker status
```

## 环境配置

### 开发环境 (development.toml)
```toml
# 开发环境特定配置，覆盖 base.toml 中的设置

[database]
url = "postgresql://postgres:123456@localhost:5432/scheduler_dev"

[message_queue]
type = "redis_stream"
[message_queue.redis]
host = "localhost"
port = 6379

[api.auth]
enabled = false  # 开发环境禁用认证

[observability]
log_level = "debug"
```

### 生产环境 (production.toml)
```toml
# 生产环境配置

[database]
url = "${DATABASE_URL}"
max_connections = 50
min_connections = 5

[message_queue]
type = "rabbitmq"
url = "${MESSAGE_QUEUE_URL}"
max_retries = 5

[api]
bind_address = "0.0.0.0:8080"
cors_origins = ["${FRONTEND_URL}"]

[api.auth]
enabled = true
jwt_secret = "${JWT_SECRET}"

[api.rate_limiting]
enabled = true
max_requests_per_minute = 1000

[observability]
log_level = "info"
```

### 测试环境 (staging.toml)
```toml
# 测试环境配置，接近生产环境但资源适中

[database]
url = "${STAGING_DATABASE_URL}"
max_connections = 20

[message_queue]
type = "rabbitmq"
url = "${STAGING_MESSAGE_QUEUE_URL}"

[api.auth]
enabled = true
jwt_secret = "${STAGING_JWT_SECRET}"

[observability]
log_level = "debug"
```

## 数据库管理

### 迁移管理
```bash
# 运行数据库迁移
sqlx migrate run --database-url $DATABASE_URL

# 创建新迁移
sqlx migrate add create_tasks_table

# 检查迁移状态
sqlx migrate info --database-url $DATABASE_URL

# 重置数据库（开发环境）
sqlx database reset --database-url $DATABASE_URL
```

### 备份和恢复
```bash
# 创建备份
pg_dump -h localhost -U postgres -d scheduler > backup_$(date +%Y%m%d).sql

# 恢复备份
psql -h localhost -U postgres -d scheduler < backup_20240101.sql

# 容器化备份
docker exec scheduler-postgres pg_dump -U postgres scheduler > backup.sql

# 容器化恢复
docker exec -i scheduler-postgres psql -U postgres scheduler < backup.sql
```

## 监控设置

### Prometheus 配置
```yaml
# monitoring/prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'scheduler-api'
    static_configs:
      - targets: ['api:8080']
    metrics_path: '/metrics'
    
  - job_name: 'scheduler-dispatcher'
    static_configs:
      - targets: ['dispatcher:8080']
    metrics_path: '/metrics'
    
  - job_name: 'scheduler-workers'
    static_configs:
      - targets: ['worker-1:8080', 'worker-2:8080']
    metrics_path: '/metrics'
```

### Grafana 仪表板
- 系统概览：CPU、内存、网络使用率
- 任务执行统计：成功率、执行时间、队列长度
- 工作节点监控：负载、心跳状态、任务分布
- 数据库性能：连接数、查询时间、慢查询

### 告警规则
```yaml
# alerts.yml
groups:
  - name: scheduler
    rules:
      - alert: HighTaskFailureRate
        expr: rate(scheduler_task_failures_total[5m]) > 0.1
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Task failure rate is high"
          
      - alert: WorkerNodeDown
        expr: up{job="scheduler-workers"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Worker node is down"
```

## 性能调优

### 数据库优化
```toml
[database]
# 连接池配置
max_connections = 50        # 根据负载调整
min_connections = 5         
connection_timeout_seconds = 10
idle_timeout_seconds = 300

# 查询优化
statement_timeout_seconds = 30
query_cache_size = 1000
```

### 消息队列优化
```toml
[message_queue]
# RabbitMQ 优化
prefetch_count = 100        # 预取消息数
consumer_timeout_seconds = 60
publisher_confirms = true
durable_queues = true

# Redis Stream 优化
[message_queue.redis]
max_stream_length = 10000
consumer_group_size = 5
read_timeout_seconds = 5
```

### 工作节点优化
```toml
[worker]
max_concurrent_tasks = 10    # 根据 CPU 核心数调整
heartbeat_interval_seconds = 15  # 根据网络稳定性调整
task_poll_interval_seconds = 1   # 低延迟调度
worker_memory_limit_mb = 2048    # 内存限制
```

## 安全配置

### SSL/TLS 配置
```toml
[api.tls]
enabled = true
cert_file = "/etc/ssl/certs/scheduler.crt"
key_file = "/etc/ssl/private/scheduler.key"
ca_file = "/etc/ssl/certs/ca.crt"
```

### 认证配置
```toml
[api.auth]
enabled = true
jwt_secret = "${JWT_SECRET}"
jwt_expiration_hours = 24
refresh_token_expiration_days = 30

# API 密钥配置
[api.auth.api_keys]
"admin-key" = ["Admin"]
"readonly-key" = ["TaskRead", "WorkerRead", "SystemRead"]
```

### 网络安全
```toml
[api.security]
# CORS 配置
cors_enabled = true
cors_origins = ["https://your-frontend.com"]
cors_max_age_seconds = 3600

# 限流配置
rate_limiting_enabled = true
max_requests_per_minute = 1000
burst_size = 100

# 请求大小限制
max_request_size_mb = 10
max_json_payload_size_mb = 5
```

## 高可用部署

### 负载均衡配置

#### Nginx 配置
```nginx
upstream scheduler_api {
    server api-1:8080;
    server api-2:8080;
    server api-3:8080;
}

upstream scheduler_dispatcher {
    server dispatcher-1:8080;
    server dispatcher-2:8080;
}

server {
    listen 80;
    server_name scheduler.yourdomain.com;
    
    location /api/ {
        proxy_pass http://scheduler_api;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
    
    location /metrics {
        proxy_pass http://scheduler_api;
        allow 10.0.0.0/8;  # 仅内网访问
        deny all;
    }
}
```

#### HAProxy 配置
```
global
    daemon
    maxconn 4096

defaults
    mode http
    timeout connect 5000ms
    timeout client 50000ms
    timeout server 50000ms

frontend scheduler_frontend
    bind *:80
    use_backend scheduler_api_backend

backend scheduler_api_backend
    balance roundrobin
    option httpchk GET /health
    server api1 api-1:8080 check
    server api2 api-2:8080 check
    server api3 api-3:8080 check
```

### 数据库集群

#### PostgreSQL 主从配置
```toml
# 主数据库
[database]
url = "postgresql://postgres:pass@db-primary:5432/scheduler"
read_only = false

# 读库配置
[database.read_replicas]
replica1 = "postgresql://postgres:pass@db-replica1:5432/scheduler"
replica2 = "postgresql://postgres:pass@db-replica2:5432/scheduler"
```

#### 连接池配置
```toml
[database.pool]
max_connections = 100
min_connections = 10
acquire_timeout_seconds = 30
max_lifetime_seconds = 1800
idle_timeout_seconds = 600
```

## 容器镜像管理

### 构建优化
```dockerfile
# 多阶段构建
FROM rust:1.75-slim AS builder
# ... 构建阶段

FROM debian:bullseye-slim AS runtime
# 运行时镜像 ~200MB vs ~2GB 构建镜像
```

### 镜像标签策略
```bash
# 开发版本
docker tag scheduler:latest scheduler:dev-$(git rev-parse --short HEAD)

# 发布版本
docker tag scheduler:latest scheduler:v1.0.0
docker tag scheduler:latest scheduler:latest

# 推送到镜像仓库
docker push scheduler:v1.0.0
docker push scheduler:latest
```

### 镜像安全扫描
```bash
# 漏洞扫描
docker scout cves scheduler:latest

# 基础镜像更新检查
docker scout recommendations scheduler:latest
```

## 日志和监控

### 日志配置
```toml
[observability]
log_level = "info"
log_format = "json"  # json 或 pretty
log_file = "/var/log/scheduler/app.log"
max_log_file_size_mb = 100
max_log_files = 10

# 结构化日志字段
[observability.log_fields]
service_name = "scheduler"
service_version = "1.0.0"
environment = "production"
```

### 监控指标
```rust
// 自定义指标示例
use metrics::{counter, histogram, gauge};

// 任务执行计数
counter!("tasks_executed_total", 
         "task_type" => task_type,
         "status" => "success");

// 执行时间分布
histogram!("task_execution_duration_seconds", execution_time);

// 工作节点负载
gauge!("worker_load_percentage", load_percentage,
       "worker_id" => worker_id);
```

### 告警配置
```yaml
# alertmanager.yml
global:
  smtp_smarthost: 'localhost:587'
  smtp_from: 'alertmanager@yourdomain.com'

route:
  group_by: ['alertname']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 1h
  receiver: 'web.hook'

receivers:
- name: 'web.hook'
  email_configs:
  - to: 'admin@yourdomain.com'
    subject: 'Scheduler Alert: {{ .GroupLabels.alertname }}'
```

## 故障排除

### 常见问题诊断

#### 1. 服务启动失败
```bash
# 检查日志
docker-compose logs service_name

# 检查配置
./scheduler config validate --file config/production.toml

# 检查端口占用
netstat -tlnp | grep 8080
```

#### 2. 数据库连接问题
```bash
# 测试连接
psql -h localhost -U postgres -d scheduler -c "SELECT 1;"

# 检查连接数
psql -h localhost -U postgres -c "SELECT count(*) FROM pg_stat_activity;"

# 查看慢查询
psql -h localhost -U postgres -c "SELECT * FROM pg_stat_statements ORDER BY total_time DESC LIMIT 10;"
```

#### 3. 消息队列问题
```bash
# RabbitMQ 管理
docker exec rabbitmq rabbitmqctl list_queues
docker exec rabbitmq rabbitmqctl list_connections

# Redis 监控
docker exec redis redis-cli info
docker exec redis redis-cli monitor
```

#### 4. 性能问题诊断
```bash
# CPU 和内存使用
docker stats

# 网络连接
docker exec service_name netstat -an

# 应用指标
curl http://localhost:8080/metrics | grep scheduler_
```

### 健康检查端点

```bash
# 系统健康
curl http://localhost:8080/health

# 详细系统信息
curl http://localhost:8080/api/system/info

# 组件健康状态
curl http://localhost:8080/api/system/health/detailed
```

### 日志分析

#### 结构化日志查询
```bash
# 查看错误日志
docker-compose logs | jq 'select(.level == "ERROR")'

# 查看特定任务日志
docker-compose logs | jq 'select(.task_id == "123")'

# 查看性能指标
docker-compose logs | jq 'select(.target == "metrics")'
```

#### 日志聚合
```bash
# ELK Stack 部署
docker-compose -f docker-compose.logging.yml up -d

# Fluentd 配置
# 收集所有容器日志并发送到 Elasticsearch
```

## 维护操作

### 滚动更新
```bash
# 无停机更新
docker-compose pull
docker-compose up -d --no-deps service_name

# Kubernetes 滚动更新
kubectl set image deployment/scheduler-api api=scheduler:v1.1.0
kubectl rollout status deployment/scheduler-api
```

### 配置热更新
```bash
# 更新配置（支持热更新的配置）
kubectl create configmap scheduler-config \
  --from-file=config/production-new.toml \
  --dry-run=client -o yaml | kubectl apply -f -

# 触发配置重载
curl -X POST http://localhost:8080/api/system/reload-config
```

### 数据维护
```bash
# 清理旧数据
./scheduler-cli data cleanup --older-than 30d

# 压缩数据库
./scheduler-cli data vacuum

# 重建索引
./scheduler-cli data reindex
```

## 备份策略

### 自动备份脚本
```bash
#!/bin/bash
# backup.sh

DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/backup/scheduler"

mkdir -p $BACKUP_DIR

# 数据库备份
docker exec scheduler-postgres pg_dump \
  -U postgres scheduler > $BACKUP_DIR/db_$DATE.sql

# 配置备份
tar czf $BACKUP_DIR/config_$DATE.tar.gz config/

# 清理旧备份（保留30天）
find $BACKUP_DIR -name "*.sql" -mtime +30 -delete
find $BACKUP_DIR -name "*.tar.gz" -mtime +30 -delete
```

### 定时备份设置
```bash
# 添加到 crontab
crontab -e
0 2 * * * /opt/scheduler/backup.sh
```

这份部署指南涵盖了从开发到生产的完整部署流程，确保系统能够稳定、安全、高效地运行。