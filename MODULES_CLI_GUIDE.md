# 模块单独入口和CLI工具使用指南

## 概述

本项目现在提供了多个独立的二进制入口程序，每个模块都可以单独启动，同时还提供了功能丰富的CLI管理工具。

## 可用的二进制程序

### 1. scheduler-dispatcher - 任务调度器
负责任务的调度和分发。

```bash
# 基本启动
cargo run --bin scheduler-dispatcher

# 指定配置文件和日志级别
cargo run --bin scheduler-dispatcher -- --config config/production.toml --log-level debug

# 使用JSON格式日志
cargo run --bin scheduler-dispatcher -- --log-format json
```

**选项参数：**
- `-c, --config <FILE>`: 配置文件路径 (默认: config/scheduler.toml)
- `-l, --log-level <LEVEL>`: 日志级别 [trace, debug, info, warn, error] (默认: info)
- `--log-format <FORMAT>`: 日志格式 [json, pretty] (默认: pretty)

### 2. scheduler-worker - Worker节点
执行从调度器分发来的任务。

```bash
# 启动Worker节点 (必须指定worker-id)
cargo run --bin scheduler-worker -- --worker-id worker-001

# 指定最大并发任务数
cargo run --bin scheduler-worker -- --worker-id worker-002 --max-tasks 20

# 完整配置示例
cargo run --bin scheduler-worker -- \
  --worker-id worker-001 \
  --config config/production.toml \
  --max-tasks 10 \
  --log-level debug
```

**选项参数：**
- `-w, --worker-id <ID>`: Worker节点唯一标识符 (必需)
- `-c, --config <FILE>`: 配置文件路径 (默认: config/scheduler.toml)
- `-m, --max-tasks <COUNT>`: 最大并发任务数量
- `-l, --log-level <LEVEL>`: 日志级别 [trace, debug, info, warn, error] (默认: info)
- `--log-format <FORMAT>`: 日志格式 [json, pretty] (默认: pretty)

### 3. scheduler-api - API服务器
提供REST API接口用于任务管理和系统监控。

```bash
# 基本启动
cargo run --bin scheduler-api

# 指定主机和端口
cargo run --bin scheduler-api -- --host 0.0.0.0 --port 9090

# 启用CORS支持
cargo run --bin scheduler-api -- --cors
```

**选项参数：**
- `-c, --config <FILE>`: 配置文件路径 (默认: config/scheduler.toml)
- `-h, --host <HOST>`: 绑定的主机地址 (默认: 127.0.0.1)
- `-p, --port <PORT>`: 监听端口 (默认: 8080)
- `--cors`: 启用CORS支持
- `-l, --log-level <LEVEL>`: 日志级别 [trace, debug, info, warn, error] (默认: info)
- `--log-format <FORMAT>`: 日志格式 [json, pretty] (默认: pretty)

### 4. scheduler - 完整系统 (原有入口)
运行完整的调度系统，支持所有模式。

```bash
# 运行所有组件
cargo run --bin scheduler

# 只运行特定模式
cargo run --bin scheduler -- --mode dispatcher
cargo run --bin scheduler -- --mode worker --worker-id worker-001
cargo run --bin scheduler -- --mode api
```

## CLI 管理工具

### scheduler-cli - 命令行管理工具
提供任务管理、Worker管理、系统监控等功能。

#### 任务管理

```bash
# 创建新任务
cargo run --bin scheduler-cli -- task create \
  --name "数据同步任务" \
  --description "每日数据同步" \
  --command "sync_data.sh" \
  --cron "0 2 * * *" \
  --priority 8 \
  --retries 3 \
  --timeout 3600

# 列出所有任务
cargo run --bin scheduler-cli -- task list

# 按状态过滤任务
cargo run --bin scheduler-cli -- task list --status running

# 分页查看任务 (每页10条，从第20条开始)
cargo run --bin scheduler-cli -- task list --limit 10 --offset 20

# 查看任务详情
cargo run --bin scheduler-cli -- task get <task-id>

# 更新任务
cargo run --bin scheduler-cli -- task update <task-id> \
  --name "新任务名称" \
  --description "新的描述"

# 删除任务
cargo run --bin scheduler-cli -- task delete <task-id>

# 强制删除 (不询问确认)
cargo run --bin scheduler-cli -- task delete <task-id> --force

# 启动任务
cargo run --bin scheduler-cli -- task start <task-id>

# 停止任务
cargo run --bin scheduler-cli -- task stop <task-id>

# 查看任务运行历史
cargo run --bin scheduler-cli -- task history <task-id> --limit 20
```

#### Worker管理

```bash
# 列出所有Worker节点
cargo run --bin scheduler-cli -- worker list

# 按状态过滤Worker
cargo run --bin scheduler-cli -- worker list --status online

# 查看Worker详情
cargo run --bin scheduler-cli -- worker get <worker-id>

# 注销Worker节点
cargo run --bin scheduler-cli -- worker unregister <worker-id>

# 强制注销
cargo run --bin scheduler-cli -- worker unregister <worker-id> --force
```

#### 系统状态监控

```bash
# 查看系统整体状态
cargo run --bin scheduler-cli -- status overall

# 查看任务统计
cargo run --bin scheduler-cli -- status tasks

# 查看Worker统计
cargo run --bin scheduler-cli -- status workers

# 查看系统健康状况
cargo run --bin scheduler-cli -- status health
```

#### 配置管理

```bash
# 显示当前配置
cargo run --bin scheduler-cli -- config show

# 验证配置文件
cargo run --bin scheduler-cli -- config validate

# 生成示例配置
cargo run --bin scheduler-cli -- config example
```

#### 全局选项

所有CLI命令都支持以下全局选项：

- `--api-url <URL>`: API服务器基础URL (默认: http://127.0.0.1:8080)
- `--api-key <KEY>`: API认证密钥 (也可通过 SCHEDULER_API_KEY 环境变量设置)
- `-c, --config <FILE>`: 配置文件路径 (默认: config/scheduler.toml)

示例：
```bash
# 使用自定义API URL和认证
cargo run --bin scheduler-cli -- \
  --api-url http://scheduler.example.com:8080 \
  --api-key your-api-key \
  task list

# 使用环境变量设置API密钥
export SCHEDULER_API_KEY=your-api-key
cargo run --bin scheduler-cli -- task list
```

## 部署场景

### 分布式部署

1. **调度器节点**：
```bash
# 在调度服务器上运行
cargo run --bin scheduler-dispatcher -- --config config/production.toml
```

2. **Worker节点**：
```bash
# 在各个Worker服务器上运行
cargo run --bin scheduler-worker -- \
  --worker-id worker-node-1 \
  --config config/production.toml \
  --max-tasks 50
```

3. **API服务器**：
```bash
# 在API服务器上运行
cargo run --bin scheduler-api -- \
  --host 0.0.0.0 \
  --port 8080 \
  --config config/production.toml \
  --cors
```

4. **管理操作**：
```bash
# 使用CLI工具管理系统
cargo run --bin scheduler-cli -- \
  --api-url http://api.scheduler.example.com:8080 \
  --api-key production-api-key \
  task list
```

### 开发环境

```bash
# 启动完整系统进行开发
cargo run --bin scheduler -- --mode all

# 或者分别启动各个组件
cargo run --bin scheduler-dispatcher &
cargo run --bin scheduler-worker -- --worker-id dev-worker &
cargo run --bin scheduler-api &

# 使用CLI进行测试
cargo run --bin scheduler-cli -- task create --name "测试任务" --command "echo hello"
```

## 构建和发布

```bash
# 构建所有二进制程序
cargo build --release --bins

# 构建特定程序
cargo build --release --bin scheduler-dispatcher
cargo build --release --bin scheduler-worker
cargo build --release --bin scheduler-api
cargo build --release --bin scheduler-cli

# 构建后的文件位于 target/release/ 目录下
ls target/release/scheduler*
```

## 注意事项

1. **配置文件**：确保各个模块使用相同的配置文件以保证系统一致性
2. **网络连接**：确保各个组件能够访问数据库和消息队列
3. **Worker ID**：每个Worker节点必须有唯一的ID
4. **API认证**：在生产环境中务必设置API密钥
5. **日志管理**：根据需要调整日志级别和格式
6. **监控**：使用CLI工具定期检查系统状态和健康状况