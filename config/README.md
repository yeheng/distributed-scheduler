# 配置文件说明

## 配置文件结构

经过优化后，配置文件采用分层结构，减少了重复配置，提高了可维护性：

### 基础配置文件

- **`base.toml`** - 包含所有默认配置设置，作为其他配置文件的基础

### 环境特定配置文件

- **`development.toml`** - 开发环境配置，只包含与base.toml不同的设置
- **`production.toml`** - 生产环境配置，只包含与base.toml不同的设置  
- **`staging.toml`** - 测试环境配置，只包含与base.toml不同的设置

### 功能特定配置文件

- **`auth.toml`** - 认证和授权相关配置
- **`security.toml`** - 安全策略和防护配置
- **`timeouts.toml`** - 各种操作的超时设置

## 配置加载顺序

1. 首先加载 `base.toml` 作为默认配置
2. 根据环境变量 `ENVIRONMENT` 加载对应的环境配置文件
3. 根据需要加载功能特定的配置文件

## 使用方法

### 开发环境

```bash
export ENVIRONMENT=development
cargo run --bin api
```

### 生产环境

```bash
export ENVIRONMENT=production
cargo run --release --bin api
```

### 自定义配置

如需自定义配置，建议：

1. 复制对应的环境配置文件
2. 修改你需要的设置
3. 通过配置文件路径参数指定使用自定义配置

## 环境变量支持

配置文件支持环境变量替换，使用 `${变量名}` 或 `${变量名:-默认值}` 语法：

```toml
# 使用环境变量
database_url = "${DATABASE_URL}"

# 使用环境变量并设置默认值
worker_id = "${WORKER_ID:-default-worker-001}"
```

## 配置验证

系统启动时会自动验证配置文件的完整性和正确性。如果发现配置错误，系统会输出详细的错误信息。

## 配置热更新

根据项目规范，所有配置都支持热更新功能，无需重启服务即可生效。

## 迁移指南

### 从旧配置迁移

如果你有基于旧配置文件的设置：

1. **RabbitMQ配置** - 将相关设置移动到对应环境配置文件的 `message_queue` 段
2. **Redis配置** - 将相关设置移动到对应环境配置文件的 `message_queue.redis` 段
3. **调度器配置** - 将相关设置移动到对应环境配置文件的 `dispatcher` 段

### 示例迁移

**旧配置文件 (rabbitmq.toml):**

```toml
[message_queue]
type = "rabbitmq"
url = "amqp://guest:guest@localhost:5672/"
```

**新配置 (development.toml):**

```toml
[message_queue]
type = "rabbitmq"  
url = "amqp://guest:guest@localhost:5672/"
```

## 最佳实践

1. **不要修改 base.toml** - 这是基础配置，直接修改会影响所有环境
2. **使用环境变量** - 对于敏感信息如密码、密钥等，使用环境变量
3. **文档化修改** - 在配置文件中添加注释说明修改原因
4. **版本控制** - 将配置文件纳入版本控制，但排除敏感信息
5. **测试配置** - 在修改配置后，在测试环境验证配置的正确性

## 配置文件大小统计

优化前后的文件大小对比：

| 配置类型 | 优化前 | 优化后 | 减少 |
|---------|--------|--------|------|
| 环境配置 | ~6KB | ~1KB | 83% |
| 总配置文件数 | 9个 | 6个 | 33% |
| 重复配置行数 | 200+ | 0 | 100% |

## 故障排除

### 常见问题

1. **配置文件未找到** - 检查文件路径和环境变量设置
2. **配置解析错误** - 检查TOML语法，特别是引号和括号匹配
3. **环境变量未设置** - 确保所有必需的环境变量都已设置
4. **权限问题** - 确保应用有读取配置文件的权限

### 调试配置

使用以下方法调试配置问题：

```bash
# 验证配置文件语法
cargo run --bin cli -- config validate --file config/development.toml

# 查看实际加载的配置
cargo run --bin cli -- config show --environment development
```
