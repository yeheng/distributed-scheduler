# 配置管理系统

分布式任务调度系统的配置管理提供了灵活的配置加载、验证和环境变量覆盖机制。

## 功能特性

- **TOML格式配置文件解析**：支持标准TOML格式的配置文件
- **环境变量覆盖**：支持通过环境变量覆盖配置文件中的任何设置
- **配置验证**：启动时自动验证配置的有效性
- **多环境支持**：支持开发、测试、生产等不同环境的配置
- **类型安全**：使用Rust的类型系统确保配置的类型安全

## 配置结构

### 数据库配置 (DatabaseConfig)

```toml
[database]
url = "postgresql://localhost:5432/scheduler"
max_connections = 20
min_connections = 2
connection_timeout_seconds = 30
idle_timeout_seconds = 600
```

### 消息队列配置 (MessageQueueConfig)

```toml
[message_queue]
url = "amqp://localhost:5672"
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 60
connection_timeout_seconds = 30
```

### Dispatcher配置 (DispatcherConfig)

```toml
[dispatcher]
enabled = true
schedule_interval_seconds = 10
max_concurrent_dispatches = 100
worker_timeout_seconds = 90
dispatch_strategy = "round_robin"  # 支持: round_robin, load_based, task_type_affinity
```

### Worker配置 (WorkerConfig)

```toml
[worker]
enabled = false
worker_id = "worker-001"
hostname = "localhost"
ip_address = "127.0.0.1"
max_concurrent_tasks = 5
supported_task_types = ["shell", "http"]
heartbeat_interval_seconds = 30
task_poll_interval_seconds = 5
```

### API配置 (ApiConfig)

```toml
[api]
enabled = true
bind_address = "0.0.0.0:8080"
cors_enabled = true
cors_origins = ["*"]
request_timeout_seconds = 30
max_request_size_mb = 10
```

### 可观测性配置 (ObservabilityConfig)

```toml
[observability]
tracing_enabled = true
metrics_enabled = true
metrics_endpoint = "/metrics"
log_level = "info"  # 支持: trace, debug, info, warn, error
jaeger_endpoint = "http://localhost:14268/api/traces"
```

## 使用方法

### 基本用法

```rust
use scheduler_core::{Config, ConfigLoader};

// 1. 加载默认配置
let config = Config::default();

// 2. 从文件加载配置
let config = Config::load(Some("config/production.toml"))?;

// 3. 使用ConfigLoader自动加载
let config = ConfigLoader::load()?;

// 4. 从TOML字符串加载
let config = Config::from_toml(toml_content)?;

// 5. 验证配置
config.validate()?;
```

### 环境变量覆盖

所有配置项都可以通过环境变量覆盖，环境变量命名规则：

- 前缀：`SCHEDULER_`
- 分隔符：`_`
- 嵌套结构：使用下划线连接

示例：

```bash
# 覆盖数据库最大连接数
export SCHEDULER_DATABASE_MAX_CONNECTIONS=50

# 覆盖Worker ID
export SCHEDULER_WORKER_WORKER_ID=prod-worker-001

# 覆盖日志级别
export SCHEDULER_OBSERVABILITY_LOG_LEVEL=warn
```

### 多环境配置

系统支持通过`SCHEDULER_ENV`环境变量指定环境：

```bash
# 开发环境（默认）
export SCHEDULER_ENV=development

# 生产环境
export SCHEDULER_ENV=production

# 测试环境
export SCHEDULER_ENV=test
```

配置文件查找顺序：

1. `SCHEDULER_CONFIG_PATH`指定的文件
2. `config/{SCHEDULER_ENV}.toml`
3. `config/scheduler.toml`
4. `scheduler.toml`
5. `/etc/scheduler/config.toml`

### 常见环境变量支持

系统还支持一些常见的环境变量：

```bash
# 数据库连接（优先级高于配置文件）
export DATABASE_URL="postgresql://prod:5432/scheduler"

# 消息队列连接
export RABBITMQ_URL="amqp://prod:5672"
export AMQP_URL="amqp://prod:5672"
```

## 配置验证

系统在启动时会自动验证配置的有效性，包括：

- **URL格式验证**：数据库和消息队列URL格式
- **数值范围验证**：连接数、超时时间等必须为正数
- **枚举值验证**：调度策略、日志级别等必须为有效值
- **必填字段验证**：关键配置项不能为空
- **逻辑一致性验证**：如最小连接数不能大于最大连接数

## 示例配置文件

### 开发环境 (config/development.toml)

```toml
[database]
url = "postgresql://localhost:5432/scheduler_dev"
max_connections = 5
min_connections = 1

[dispatcher]
enabled = true
schedule_interval_seconds = 5
max_concurrent_dispatches = 10

[worker]
enabled = true
worker_id = "dev-worker-001"
max_concurrent_tasks = 2

[observability]
log_level = "debug"
tracing_enabled = true
metrics_enabled = false
```

### 生产环境 (config/production.toml)

```toml
[database]
url = "postgresql://db-host:5432/scheduler_prod"
max_connections = 50
min_connections = 10
connection_timeout_seconds = 60

[message_queue]
url = "amqp://mq-host:5672"
max_retries = 5
retry_delay_seconds = 120

[dispatcher]
enabled = true
schedule_interval_seconds = 30
max_concurrent_dispatches = 500
worker_timeout_seconds = 180

[worker]
enabled = false
max_concurrent_tasks = 20
heartbeat_interval_seconds = 60

[api]
cors_enabled = false
cors_origins = []
request_timeout_seconds = 60
max_request_size_mb = 50

[observability]
log_level = "warn"
tracing_enabled = true
metrics_enabled = true
jaeger_endpoint = "http://jaeger:14268/api/traces"
```

## 最佳实践

1. **环境分离**：为不同环境创建独立的配置文件
2. **敏感信息**：使用环境变量存储敏感信息（如密码、密钥）
3. **配置验证**：在应用启动时立即验证配置
4. **默认值**：为所有配置项提供合理的默认值
5. **文档化**：为每个配置项添加清晰的注释说明

## 故障排除

### 常见错误

1. **配置文件不存在**

   ```
   错误: 配置文件不存在: /path/to/config.toml
   解决: 检查文件路径或使用默认配置
   ```

2. **TOML格式错误**

   ```
   错误: 解析TOML配置失败
   解决: 检查TOML语法，确保格式正确
   ```

3. **配置验证失败**

   ```
   错误: 数据库URL必须是PostgreSQL格式
   解决: 检查URL格式，确保以postgresql://开头
   ```

4. **环境变量格式错误**

   ```
   错误: 无效的日志级别: invalid
   解决: 使用有效的日志级别 (trace, debug, info, warn, error)
   ```

### 调试技巧

1. **启用调试日志**：设置`SCHEDULER_OBSERVABILITY_LOG_LEVEL=debug`
2. **检查环境变量**：使用`env | grep SCHEDULER`查看所有相关环境变量
3. **配置序列化**：使用`config.to_toml()`查看最终的配置内容
4. **分步验证**：单独验证每个配置模块
