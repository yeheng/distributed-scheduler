# Redis Stream消息队列架构设计

## 概述

基于Redis Stream的轻量级消息队列实现，完全兼容现有的MessageQueue trait接口。该设计严格遵循SLOID、KISS、YAGNI原则，仅实现当前需求所需的最小功能集。

## 架构

### 核心组件

```
┌─────────────────────────────┐
│   RedisStreamMessageQueue   │
├─────────────────────────────┤
│ - connection: Connection    │
│ - config: RedisConfig       │
│ - streams: StreamManager    │
└─────────────────────────────┘
           │
           ▼
┌─────────────────────────────┐
│      Redis Client          │
│    (redis-rs crate)        │
└─────────────────────────────┘
           │
           ▼
┌─────────────────────────────┐
│      Redis Server          │
│    (Redis Streams)         │
└─────────────────────────────┘
```

### 数据映射

| 概念映射        | RabbitMQ      | Redis Stream |
|----------------|---------------|--------------|
| 队列            | Queue         | Stream       |
| 消息            | Message       | Stream Entry |
| 消费者          | Consumer      | Consumer Group |
| 消息确认        | ACK           | XACK         |
| 消息拒绝        | NACK/Requeue  | XDEL/XADD    |

### Stream设计

#### Stream结构
```
Stream Key: <queue_name>
Stream Entry: {
    id: "<timestamp>-<sequence>",
    fields: {
        "message_id": "<uuid>",
        "data": "<serialized_message>",
        "timestamp": "<unix_timestamp>"
    }
}
```

#### Consumer Group设计
```
Consumer Group: <queue_name>_group
Consumer: <worker_id>
```

### 组件和接口

#### 1. RedisStreamMessageQueue
- **职责**: 实现MessageQueue trait，提供Redis Stream的消息队列功能
- **依赖**: Redis客户端、配置、序列化

#### 2. RedisConfig
- **职责**: 封装Redis连接配置
- **字段**: host, port, database, password, timeout
- **默认值**: 127.0.0.1:6379, db 0, 30s timeout

#### 3. StreamManager
- **职责**: 管理Redis Stream和消费者组的生命周期
- **方法**: create_stream, create_consumer_group, ensure_stream_exists

## 数据模型

### Redis配置扩展

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub database: i64,
    pub password: Option<String>,
    pub connection_timeout_seconds: u64,
    pub max_retry_attempts: u32,
    pub retry_delay_seconds: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            database: 0,
            password: None,
            connection_timeout_seconds: 30,
            max_retry_attempts: 3,
            retry_delay_seconds: 1,
        }
    }
}
```

### 消息队列配置扩展

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueConfig {
    pub r#type: MessageQueueType, // "rabbitmq" | "redis_stream"
    pub url: String,              // 保持兼容
    pub redis: Option<RedisConfig>, // Redis特定配置
    pub task_queue: String,
    pub status_queue: String,
    pub heartbeat_queue: String,
    pub control_queue: String,
    pub max_retries: i32,
    pub retry_delay_seconds: u64,
    pub connection_timeout_seconds: u64,
}
```

## 实现策略

### 1. 消息发布 (XADD)
```rust
// 简化流程：直接将消息写入Stream
XADD <queue_name> * message_id <id> data <payload>
```

### 2. 消息消费 (XREADGROUP)
```rust
// 从消费者组读取消息
XREADGROUP GROUP <queue_name>_group <consumer_id> COUNT 100 STREAMS <queue_name> >
```

### 3. 消息确认 (XACK)
```rust
// 确认消息已处理
XACK <queue_name> <queue_name>_group <message_id>
```

### 4. 消息拒绝
```rust
// 拒绝消息并可选重新入队
XDEL <queue_name> <message_id>  // 删除消息
// 如果需要重新入队，显式重新发布
```

## 错误处理

### 错误类型
- `RedisConnectionError`: 连接失败
- `StreamNotFoundError`: Stream不存在
- `ConsumerGroupError`: 消费者组操作失败
- `SerializationError`: 消息序列化/反序列化失败
- `TimeoutError`: 操作超时

### 重试策略
- 指数退避重试
- 最大重试次数限制
- 连接池自动恢复

## 测试策略

### 单元测试
- 消息序列化/反序列化
- Stream操作 (使用Redis mock)
- 错误处理逻辑

### 集成测试
- 与真实Redis实例的交互
- 并发消息处理
- 故障恢复场景

### 兼容性测试
- 与RabbitMQ实现的等价性测试
- 配置切换测试
- 性能对比测试

## 部署配置

### 配置文件示例
```toml
[message_queue]
type = "redis_stream"
url = "redis://localhost:6379"

[message_queue.redis]
host = "localhost"
port = 6379
database = 0
password = ""
connection_timeout_seconds = 30
max_retry_attempts = 3
retry_delay_seconds = 1
```

## 性能考虑

### Redis Stream特性
- **持久化**: Stream数据持久化到磁盘
- **内存效率**: 使用紧凑的Stream格式
- **并发**: 支持多个消费者组
- **回溯**: 支持消息重放

### 优化策略
- 合理设置Stream最大长度
- 使用批量操作减少网络往返
- 消费者组负载均衡

## 安全性

### 连接安全
- 支持Redis AUTH认证
- TLS/SSL连接支持
- 连接池复用

### 数据安全
- 消息加密传输
- 敏感数据脱敏
- 访问权限控制

## 监控与可观测性

### 指标收集
- Stream长度监控
- 消费者组延迟
- 消息处理速率
- 错误率统计

### 日志记录
- 关键操作日志
- 错误堆栈跟踪
- 性能指标记录

## 部署兼容性

### 向后兼容
- 完全实现MessageQueue trait
- 保持现有API接口不变
- 配置文件格式兼容

### 渐进式迁移
- 支持运行时切换消息队列类型
- 零停机迁移方案
- 数据迁移工具

## 技术决策记录

### 选择Redis Stream的原因
1. **轻量级**: 相比RabbitMQ部署更简单
2. **性能**: 更高的吞吐量和更低延迟
3. **一致性**: 与现有Redis基础设施集成
4. **可靠性**: 持久化和故障恢复能力

### 不实现的功能（YAGNI）
- 复杂的路由规则
- 优先级队列
- 延迟消息
- 死信队列（超出当前需求范围）

### 架构权衡
- **简单性 vs 功能完整性**: 优先简单实现
- **性能 vs 可靠性**: 平衡两者需求
- **兼容性 vs 优化**: 保持接口兼容前提下优化