# 消息队列配置切换机制

本文档介绍如何在分布式任务调度系统中使用消息队列配置切换机制，实现在RabbitMQ和Redis Stream之间的无缝切换。

## 概述

消息队列配置切换机制允许系统在不修改业务代码的情况下，通过配置文件在不同的消息队列实现之间进行切换。目前支持：

- **RabbitMQ**: 功能完整的消息代理，适合复杂的消息路由场景
- **Redis Stream**: 轻量级的流式消息队列，适合简单的消息传递场景

## 核心组件

### 1. MessageQueueFactory

消息队列工厂负责根据配置创建相应的消息队列实例。

```rust
use scheduler_infrastructure::MessageQueueFactory;
use scheduler_core::config::model::MessageQueueConfig;

// 创建消息队列实例
let queue = MessageQueueFactory::create(&config).await?;
```

### 2. MessageQueueManager

消息队列管理器支持运行时切换消息队列类型。

```rust
use scheduler_infrastructure::MessageQueueManager;

// 创建管理器
let mut manager = MessageQueueManager::new(initial_config).await?;

// 运行时切换
manager.switch_to(new_config).await?;
```

## 配置方式

### RabbitMQ配置

```toml
[message_queue]
type = "rabbitmq"
url = "amqp://guest:guest@localhost:5672/"
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 60
connection_timeout_seconds = 30
```

### Redis Stream配置（使用URL）

```toml
[message_queue]
type = "redis_stream"
url = "redis://localhost:6379/0"
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 60
connection_timeout_seconds = 30
```

### Redis Stream配置（使用配置段）

```toml
[message_queue]
type = "redis_stream"
url = ""  # 可以为空
task_queue = "tasks"
status_queue = "status_updates"
heartbeat_queue = "heartbeats"
control_queue = "control"
max_retries = 3
retry_delay_seconds = 60
connection_timeout_seconds = 30

[message_queue.redis]
host = "127.0.0.1"
port = 6379
database = 0
password = "your_password"  # 可选
connection_timeout_seconds = 30
max_retry_attempts = 3
retry_delay_seconds = 1
```

## 使用示例

### 基本使用

```rust
use scheduler_core::config::model::AppConfig;
use scheduler_infrastructure::MessageQueueManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 加载配置
    let config = AppConfig::load(Some("config/redis-stream.toml"))?;
    
    // 创建消息队列管理器
    let manager = MessageQueueManager::new(config.message_queue).await?;
    
    // 使用消息队列
    let message = create_test_message();
    manager.publish_message("tasks", &message).await?;
    
    let messages = manager.consume_messages("tasks").await?;
    for msg in messages {
        println!("处理消息: {}", msg.id);
        manager.ack_message(&msg.id).await?;
    }
    
    Ok(())
}
```

### 运行时切换

```rust
use scheduler_core::config::model::{MessageQueueConfig, MessageQueueType};
use scheduler_infrastructure::MessageQueueManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始使用Redis Stream
    let redis_config = create_redis_config();
    let mut manager = MessageQueueManager::new(redis_config).await?;
    
    println!("当前使用: {}", manager.get_current_type_string());
    
    // 发布消息
    let message = create_test_message();
    manager.publish_message("tasks", &message).await?;
    
    // 切换到RabbitMQ
    let rabbitmq_config = create_rabbitmq_config();
    manager.switch_to(rabbitmq_config).await?;
    
    println!("切换后使用: {}", manager.get_current_type_string());
    
    // 继续使用相同的接口
    let message2 = create_test_message();
    manager.publish_message("tasks", &message2).await?;
    
    Ok(())
}
```

### 配置验证

```rust
use scheduler_infrastructure::MessageQueueFactory;

// 验证配置
if let Err(e) = MessageQueueFactory::validate_config(&config) {
    eprintln!("配置验证失败: {}", e);
    return;
}

println!("配置验证通过");
```

## 配置文件示例

项目提供了两个配置文件示例：

- `config/rabbitmq.toml`: RabbitMQ配置示例
- `config/redis-stream.toml`: Redis Stream配置示例

可以直接复制这些文件并根据实际环境修改配置参数。

## 部署建议

### 开发环境

开发环境建议使用Redis Stream，因为：
- 部署简单，只需要Redis服务器
- 配置简单，减少开发环境复杂性
- 性能足够满足开发测试需求

```bash
# 启动Redis
docker run -d --name redis -p 6379:6379 redis:latest

# 使用Redis Stream配置
cp config/redis-stream.toml config/scheduler.toml
```

### 生产环境

生产环境可以根据需求选择：

**选择RabbitMQ的场景：**
- 需要复杂的消息路由
- 需要消息持久化保证
- 需要集群高可用
- 团队熟悉RabbitMQ运维

**选择Redis Stream的场景：**
- 消息传递模式简单
- 已有Redis基础设施
- 追求更高性能
- 希望减少组件复杂性

## 性能对比

| 特性 | RabbitMQ | Redis Stream |
|------|----------|--------------|
| 吞吐量 | 中等 | 高 |
| 延迟 | 中等 | 低 |
| 内存使用 | 中等 | 低 |
| 磁盘使用 | 高 | 中等 |
| 功能完整性 | 高 | 中等 |
| 运维复杂性 | 高 | 低 |

## 故障排除

### 常见错误

1. **连接失败**
   ```
   错误: Failed to connect to Redis/RabbitMQ
   解决: 检查服务器是否运行，网络是否可达
   ```

2. **配置错误**
   ```
   错误: Redis Stream配置缺失
   解决: 提供redis配置段或有效的Redis URL
   ```

3. **切换失败**
   ```
   错误: 切换消息队列类型失败
   解决: 确保目标消息队列服务器正常运行
   ```

### 调试技巧

1. **启用详细日志**
   ```rust
   std::env::set_var("RUST_LOG", "debug");
   tracing_subscriber::fmt::init();
   ```

2. **检查连接状态**
   ```rust
   // 对于RabbitMQ
   if let Some(rabbitmq) = queue.as_any().downcast_ref::<RabbitMQMessageQueue>() {
       println!("RabbitMQ连接状态: {}", rabbitmq.is_connected());
   }
   ```

3. **验证配置**
   ```rust
   match MessageQueueFactory::validate_config(&config) {
       Ok(()) => println!("配置有效"),
       Err(e) => println!("配置错误: {}", e),
   }
   ```

## 最佳实践

1. **配置管理**
   - 使用环境变量覆盖敏感配置（如密码）
   - 为不同环境准备不同的配置文件
   - 定期验证配置的有效性

2. **错误处理**
   - 实现重试机制处理临时连接问题
   - 记录详细的错误日志便于排查
   - 提供降级方案应对消息队列不可用

3. **监控**
   - 监控消息队列的连接状态
   - 监控消息处理的延迟和吞吐量
   - 设置告警机制及时发现问题

4. **测试**
   - 编写集成测试验证切换功能
   - 测试各种故障场景的处理
   - 进行性能测试确保满足需求

## 扩展

如果需要支持其他消息队列系统（如Apache Kafka、Apache Pulsar等），可以：

1. 实现相应的MessageQueue trait
2. 在MessageQueueFactory中添加新的类型支持
3. 更新配置模型添加新的配置选项
4. 编写相应的测试和文档

这种设计保证了系统的可扩展性，可以轻松添加新的消息队列实现。