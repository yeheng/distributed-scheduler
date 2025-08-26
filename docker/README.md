# Docker 部署指南

分布式任务调度系统的 Docker 容器化部署指南。

## 目录结构

```
docker/
├── README.md                 # 本文档
├── Dockerfile               # 多阶段构建 Dockerfile
├── docker-compose.yml       # 完整系统部署
├── docker-compose.dev.yml   # 开发环境依赖服务
├── .dockerignore           # Docker 构建忽略文件
└── monitoring/
    └── prometheus.yml       # Prometheus 监控配置
```

## 快速开始

### 1. 开发环境

仅启动依赖服务（PostgreSQL、RabbitMQ），本地运行应用：

```bash
# 启动依赖服务
docker-compose -f docker-compose.dev.yml up -d

# 等待服务就绪
docker-compose -f docker-compose.dev.yml ps

# 本地运行应用
cargo run -- --config config/development.toml --mode all
```

### 2. 完整容器化部署

启动完整的分布式系统：

```bash
# 构建并启动所有服务
docker-compose up -d

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f

# 停止所有服务
docker-compose down
```

### 3. 生产环境部署

```bash
# 使用生产配置
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d

# 或者使用环境变量覆盖
DATABASE_URL=postgresql://prod-db:5432/scheduler \
MESSAGE_QUEUE_URL=amqp://prod-mq:5672 \
docker-compose up -d
```

## 服务说明

### 核心服务

| 服务名 | 端口 | 说明 |
|--------|------|------|
| postgres | 5432 | PostgreSQL 数据库 |
| rabbitmq | 5672, 15672 | RabbitMQ 消息队列 |
| dispatcher | 8080 | 任务调度器 |
| api | 8081 | REST API 服务器 |
| worker-1 | - | 工作节点 1 |
| worker-2 | - | 工作节点 2 |

### 监控服务（可选）

| 服务名 | 端口 | 说明 |
|--------|------|------|
| prometheus | 9090 | 指标收集 |
| grafana | 3000 | 监控面板 |

启动监控服务：

```bash
docker-compose --profile monitoring up -d
```

## 镜像构建

### 本地构建

```bash
# 构建镜像
docker build -t scheduler:latest .

# 查看镜像大小
docker images scheduler

# 运行单个容器
docker run -d \
  --name scheduler-test \
  -p 8080:8080 \
  -e DATABASE_URL=postgresql://host.docker.internal:5432/scheduler \
  -e MESSAGE_QUEUE_URL=amqp://host.docker.internal:5672 \
  scheduler:latest
```

### 多阶段构建优化

Dockerfile 使用多阶段构建来优化镜像大小：

1. **构建阶段**: 使用完整的 Rust 镜像编译应用
2. **运行时阶段**: 使用轻量级 Debian 镜像运行应用

优化效果：

- 构建镜像: ~2GB
- 运行时镜像: ~200MB

## 配置管理

### 环境变量

支持通过环境变量覆盖配置：

```bash
# 数据库配置
DATABASE_URL=postgresql://user:pass@host:port/db
DATABASE_MAX_CONNECTIONS=20

# 消息队列配置
MESSAGE_QUEUE_URL=amqp://user:pass@host:port
MESSAGE_QUEUE_MAX_RETRIES=5

# 应用配置
RUST_LOG=info
RUST_BACKTRACE=1
```

### 配置文件挂载

```bash
# 挂载自定义配置
docker run -d \
  -v /path/to/config:/app/config:ro \
  scheduler:latest \
  scheduler --config config/custom.toml
```

## 数据持久化

### 数据卷

系统使用 Docker 数据卷持久化数据：

```bash
# 查看数据卷
docker volume ls | grep scheduler

# 备份数据库
docker exec scheduler-postgres pg_dump -U postgres scheduler > backup.sql

# 恢复数据库
docker exec -i scheduler-postgres psql -U postgres scheduler < backup.sql
```

### 数据目录

容器内数据目录：

- 数据库数据: `/var/lib/postgresql/data`
- RabbitMQ 数据: `/var/lib/rabbitmq`
- 应用日志: `/var/log/scheduler`

## 健康检查

### 内置健康检查

所有服务都配置了健康检查：

```bash
# 查看健康状态
docker-compose ps

# 手动健康检查
curl http://localhost:8080/api/health
curl http://localhost:8081/api/health
```

### 监控端点

- 健康检查: `GET /api/health`
- 指标收集: `GET /metrics`
- 系统信息: `GET /api/system/info`

## 日志管理

### 日志查看

```bash
# 查看所有服务日志
docker-compose logs

# 查看特定服务日志
docker-compose logs dispatcher
docker-compose logs worker-1

# 实时跟踪日志
docker-compose logs -f --tail=100
```

### 日志配置

支持多种日志格式：

```bash
# JSON 格式日志
docker run -e LOG_FORMAT=json scheduler:latest

# Pretty 格式日志（默认）
docker run -e LOG_FORMAT=pretty scheduler:latest
```

## 扩展部署

### 水平扩展 Worker

```bash
# 扩展 Worker 节点到 5 个
docker-compose up -d --scale worker-1=5

# 或者添加新的 Worker 服务
docker-compose run -d \
  --name scheduler-worker-3 \
  worker-1 \
  scheduler --mode worker --worker-id worker-3
```

### 多 Dispatcher 部署

```bash
# 启动多个 Dispatcher 实例
docker-compose up -d --scale dispatcher=3
```

## 故障排除

### 常见问题

1. **数据库连接失败**

   ```bash
   # 检查数据库状态
   docker-compose exec postgres pg_isready -U postgres
   
   # 查看数据库日志
   docker-compose logs postgres
   ```

2. **消息队列连接失败**

   ```bash
   # 检查 RabbitMQ 状态
   docker-compose exec rabbitmq rabbitmq-diagnostics ping
   
   # 访问管理界面
   open http://localhost:15672
   ```

3. **应用启动失败**

   ```bash
   # 查看应用日志
   docker-compose logs dispatcher
   
   # 进入容器调试
   docker-compose exec dispatcher bash
   ```

### 性能调优

1. **资源限制**

   ```yaml
   services:
     dispatcher:
       deploy:
         resources:
           limits:
             cpus: '2.0'
             memory: 2G
           reservations:
             cpus: '1.0'
             memory: 1G
   ```

2. **数据库连接池**

   ```bash
   # 调整数据库连接数
   -e DATABASE_MAX_CONNECTIONS=50
   ```

## 安全配置

### 网络安全

```yaml
# 限制网络访问
networks:
  scheduler-network:
    driver: bridge
    internal: true  # 禁止外部访问
```

### 用户权限

```dockerfile
# 使用非 root 用户运行
USER scheduler
```

### 敏感信息

```bash
# 使用 Docker secrets
echo "password123" | docker secret create db_password -

# 在 compose 文件中引用
secrets:
  - db_password
```

## 监控和告警

### Prometheus 指标

访问 <http://localhost:9090> 查看 Prometheus 监控。

关键指标：

- 任务执行成功率
- 任务执行时间
- Worker 节点状态
- 队列长度

### Grafana 面板

访问 <http://localhost:3000> 查看 Grafana 面板（admin/admin）。

预配置面板：

- 系统概览
- 任务执行统计
- Worker 节点监控
- 数据库性能

## 备份和恢复

### 自动备份

```bash
# 创建备份脚本
cat > backup.sh << 'EOF'
#!/bin/bash
DATE=$(date +%Y%m%d_%H%M%S)
docker exec scheduler-postgres pg_dump -U postgres scheduler > backup_${DATE}.sql
EOF

# 设置定时备份
crontab -e
0 2 * * * /path/to/backup.sh
```

### 灾难恢复

```bash
# 停止服务
docker-compose down

# 恢复数据
docker-compose up -d postgres
docker exec -i scheduler-postgres psql -U postgres scheduler < backup.sql

# 重启所有服务
docker-compose up -d
```
