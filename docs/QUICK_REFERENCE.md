# 快速参考指南 - 分布式任务调度系统

## 🚀 快速命令

### 开发环境

```bash
# 启动开发环境
docker-compose -f docker-compose.dev.yml up -d

# 数据库迁移
cargo run --bin migrate

# 启动所有服务
cargo run --bin scheduler &    # 调度器
cargo run --bin worker &       # Worker
cargo run --bin api           # API服务

# 健康检查
curl http://localhost:8080/health
```

### 构建与测试

```bash
# 完整构建
cargo build --release

# 运行测试
cargo test --all                    # 所有测试
cargo test --test integration_tests # 集成测试
cargo test -p scheduler-core        # 特定crate

# 代码质量
cargo fmt --check                   # 格式检查
cargo clippy -- -D warnings         # 静态分析
cargo audit                         # 安全审计
```

## 📋 API快速参考

### 基础信息

- **Base URL**: `http://localhost:8080/api`
- **认证**: `Authorization: Bearer <token>`
- **Content-Type**: `application/json`

### 任务管理

```bash
# 创建任务
curl -X POST http://localhost:8080/api/tasks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "name": "my_task",
    "executor": "shell",
    "command": "echo hello world",
    "schedule": "0 */5 * * * *"
  }'

# 获取任务列表
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/tasks

# 获取任务详情
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/tasks/1

# 触发任务
curl -X POST \
  -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/tasks/1/trigger

# 暂停/恢复任务
curl -X POST \
  -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/tasks/1/pause

curl -X POST \
  -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/tasks/1/resume
```

### Worker管理

```bash
# 获取Worker列表
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/workers

# Worker详情
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/workers/worker-001

# Worker心跳
curl -X POST \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  http://localhost:8080/api/workers/worker-001/heartbeat \
  -d '{
    "current_task_count": 2,
    "load_average": 0.5,
    "memory_usage": 512,
    "cpu_usage": 25.0
  }'
```

### 系统监控

```bash
# 健康检查
curl http://localhost:8080/health

# 系统统计
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/system/stats

# Prometheus指标
curl http://localhost:8080/metrics

# 系统日志
curl -H "Authorization: Bearer <token>" \
  "http://localhost:8080/api/system/logs?level=error&page=1&size=50"
```

## 🔧 配置快速参考

### 环境变量

```bash
# 数据库配置
export DATABASE_URL="postgresql://user:pass@localhost:5432/scheduler"
export DATABASE_MAX_CONNECTIONS=20

# 消息队列配置
export RABBITMQ_URL="amqp://guest:guest@localhost:5672"
export REDIS_URL="redis://localhost:6379"

# API配置
export API_HOST="0.0.0.0"
export API_PORT=8080
export JWT_SECRET="your-secret-key"

# 日志配置
export RUST_LOG="info,scheduler=debug"
export LOG_FORMAT="json"
```

### 配置文件模板

```toml
# config/development.toml
[database]
url = "postgresql://localhost:5432/scheduler_dev"
max_connections = 10
connection_timeout = 30

[message_queue]
type = "rabbitmq"
url = "amqp://guest:guest@localhost:5672"

[api]
host = "127.0.0.1"
port = 8080
cors_origins = ["http://localhost:3000"]

[scheduler]
max_workers = 50
scheduling_interval = 5
task_timeout = 3600

[worker]
heartbeat_interval = 30
max_concurrent_tasks = 5
executor_types = ["shell", "python", "http"]

[observability]
metrics_enabled = true
tracing_enabled = true
log_level = "info"
```

## 📊 数据模型快速参考

### Task (任务)

```json
{
  "id": 1,
  "name": "数据处理任务",
  "description": "处理每日数据文件",
  "task_type": "python",
  "executor": "python",
  "command": "python process_data.py",
  "arguments": ["--input", "data.csv"],
  "schedule": "0 2 * * *",
  "priority": "normal",
  "retry_count": 3,
  "timeout_seconds": 3600,
  "status": "active",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z",
  "last_run_at": "2024-01-01T02:00:00Z",
  "next_run_at": "2024-01-02T02:00:00Z",
  "dependencies": [],
  "tags": ["data", "daily"],
  "metadata": {}
}
```

### TaskRun (任务运行)

```json
{
  "id": 123,
  "task_id": 1,
  "worker_id": "worker-001",
  "status": "completed",
  "started_at": "2024-01-01T02:00:00Z",
  "completed_at": "2024-01-01T02:00:30Z",
  "result": "处理成功",
  "error_message": null,
  "retry_count": 0,
  "exit_code": 0,
  "execution_time": 30.5
}
```

### Worker (工作节点)

```json
{
  "id": "worker-001",
  "hostname": "server-01",
  "ip_address": "192.168.1.100",
  "port": 8080,
  "task_types": ["python", "shell"],
  "max_concurrent_tasks": 5,
  "current_task_count": 2,
  "status": "active",
  "last_heartbeat": "2024-01-01T00:00:00Z",
  "registered_at": "2024-01-01T00:00:00Z",
  "load_average": 0.4,
  "memory_usage": 512,
  "cpu_usage": 25.5
}
```

## 🎯 Cron表达式参考

### 格式: `秒 分 时 日 月 周`

```bash
# 每分钟执行
"0 * * * * *"

# 每5分钟执行
"0 */5 * * * *"

# 每小时执行
"0 0 * * * *"

# 每天2点执行
"0 0 2 * * *"

# 每周一9点执行
"0 0 9 * * 1"

# 每月1号执行
"0 0 0 1 * *"

# 工作日每天9点执行
"0 0 9 * * 1-5"
```

## 🔍 故障排除快速参考

### 日志查询

```bash
# Docker Compose日志
docker-compose logs scheduler
docker-compose logs worker
docker-compose logs -f --tail=100

# 系统日志查询
curl -H "Authorization: Bearer <token>" \
  "http://localhost:8080/api/system/logs?level=error&start_time=2024-01-01T00:00:00Z"

# 数据库日志
tail -f /var/log/postgresql/postgresql-13-main.log

# RabbitMQ日志
docker exec rabbitmq rabbitmqctl list_queues
```

### 常见问题诊断

```bash
# 检查服务状态
curl http://localhost:8080/health

# 检查数据库连接
psql $DATABASE_URL -c "SELECT 1;"

# 检查RabbitMQ连接
docker exec rabbitmq rabbitmqctl status

# 检查Worker状态
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/workers

# 检查任务队列
curl -H "Authorization: Bearer <token>" \
  "http://localhost:8080/api/runs?status=pending"
```

### 性能监控

```bash
# 系统资源使用
htop
iostat -x 1
netstat -an | grep :8080

# 应用指标
curl http://localhost:8080/metrics | grep scheduler_

# 数据库性能
psql $DATABASE_URL -c "
SELECT query, calls, total_time, mean_time 
FROM pg_stat_statements 
ORDER BY total_time DESC LIMIT 10;
"
```

## 🛠️ 开发工具快速参考

### Cargo命令

```bash
# 依赖管理
cargo tree                    # 依赖树
cargo outdated               # 过时依赖
cargo audit                  # 安全审计
cargo update                 # 更新依赖

# 代码分析
cargo expand                 # 宏展开
cargo check --all-targets   # 编译检查
cargo doc --open            # 生成文档

# 性能分析
cargo bench                  # 性能测试
cargo flamegraph            # 火焰图
```

### 数据库工具

```bash
# 迁移管理
sqlx migrate add <name>      # 创建迁移
sqlx migrate run            # 运行迁移
sqlx migrate revert         # 回滚迁移

# 数据库查询
psql $DATABASE_URL
\dt                         # 列出表
\d tasks                    # 表结构
\q                          # 退出
```

### Docker工具

```bash
# 容器管理
docker-compose up -d        # 后台启动
docker-compose ps           # 容器状态
docker-compose logs -f      # 实时日志
docker-compose down         # 停止并删除

# 容器调试
docker exec -it scheduler_postgres_1 psql -U postgres
docker exec -it scheduler_rabbitmq_1 rabbitmqctl status
```

## 📈 性能基准参考

### 响应时间目标

| 操作 | 目标时间 | 说明 |
|------|----------|------|
| API响应 | <100ms | P99响应时间 |
| 任务调度 | <1s | 从触发到分配 |
| 数据库查询 | <50ms | 单次查询 |
| 心跳处理 | <10ms | Worker心跳 |

### 吞吐量目标

| 指标 | 目标值 | 说明 |
|------|--------|------|
| 任务调度 | 10,000/min | 每分钟调度任务数 |
| 消息处理 | 50,000/sec | 消息队列处理能力 |
| API请求 | 1,000/sec | 并发API请求 |
| Worker节点 | 100+ | 支持Worker数量 |

### 资源使用参考

| 资源 | 基础配置 | 生产配置 |
|------|----------|----------|
| CPU | 2核心 | 4-8核心 |
| 内存 | 512MB | 2-4GB |
| 存储 | 10GB | 100GB+ |
| 网络 | 100Mbps | 1Gbps |

## 🔐 安全配置参考

### JWT配置

```rust
// JWT密钥生成
use rand::Rng;
let secret: [u8; 32] = rand::thread_rng().gen();
let secret_base64 = base64::encode(secret);
```

### API密钥格式

```bash
# API密钥格式: scheduler_<32字符随机字符串>
scheduler_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6
```

### 权限级别

```yaml
permissions:
  - task.read     # 读取任务
  - task.write    # 创建/更新任务
  - task.execute  # 触发任务
  - task.delete   # 删除任务
  - worker.read   # 读取Worker信息
  - worker.write  # 管理Worker
  - system.read   # 系统监控
  - system.write  # 系统管理
```

---

此快速参考指南提供了分布式任务调度系统的常用命令、配置和操作指南，适合开发和运维人员日常使用。

**最后更新**: 2025-08-21  
**版本**: v1.0