# Redis Stream消息消费实现文档

## 概述

本文档描述了Redis Stream消息队列的消息消费功能实现，包括消费者组管理、消息反序列化和待处理消息处理等核心功能。

## 实现的功能

### 1. 消息消费 (`consume_messages`)

主要的消息消费方法，实现了以下功能：

- **队列名称验证**: 确保队列名称符合Redis Stream要求
- **Stream和消费者组自动创建**: 如果不存在则自动创建
- **待处理消息优先处理**: 首先处理已读取但未确认的消息
- **新消息消费**: 然后读取新的未处理消息
- **消息ID映射**: 维护Message ID到Redis Stream ID的映射关系

```rust
async fn consume_messages(&self, queue: &str) -> Result<Vec<Message>>
```

### 2. 消费者组管理

#### 自动创建消费者组
```rust
async fn ensure_consumer_group_exists(&self, stream_key: &str, group_name: &str) -> Result<()>
```

- 使用`XGROUP CREATE`命令创建消费者组
- 如果消费者组已存在，忽略`BUSYGROUP`错误
- 支持`MKSTREAM`选项，如果Stream不存在则自动创建

#### 消费者组命名规则
```rust
fn get_consumer_group_name(&self, queue: &str) -> String
```

格式：`{consumer_group_prefix}_{queue}_group`

例如：`scheduler_tasks_group`

### 3. 消息反序列化

#### 消息解析流程
1. **Redis响应解析**: 解析`XREADGROUP`返回的复杂嵌套结构
2. **字段提取**: 从Redis Stream条目中提取字段键值对
3. **消息反序列化**: 将JSON字符串反序列化为Message对象
4. **重试次数更新**: 从Redis字段中恢复重试次数
5. **ID映射存储**: 存储Message ID到Stream ID的映射

#### 支持的字段
- `data`: 序列化的消息JSON数据
- `message_id`: 消息的唯一标识符
- `timestamp`: 消息时间戳
- `retry_count`: 重试次数
- `correlation_id`: 关联ID

### 4. 待处理消息处理

#### 待处理消息消费
```rust
async fn consume_pending_messages(&self, queue: &str) -> Result<Vec<Message>>
```

- 使用`XREADGROUP`命令读取待处理消息（参数为`0`而不是`>`）
- 处理已被消费者读取但未确认的消息
- 确保消息不会丢失

#### 新消息消费
```rust
async fn consume_new_messages(&self, queue: &str) -> Result<Vec<Message>>
```

- 使用`XREADGROUP`命令读取新消息（参数为`>`）
- 支持阻塞读取（`BLOCK 1000`毫秒）
- 批量读取（`COUNT 100`条消息）

### 5. 消息ID映射管理

为了支持后续的ACK/NACK操作，实现了Message ID到Redis Stream ID的映射：

```rust
message_id_mapping: Arc<Mutex<HashMap<String, String>>>
```

- **存储时机**: 消息消费时自动存储映射关系
- **线程安全**: 使用Arc<Mutex>确保并发安全
- **用途**: 用于`ack_message`和`nack_message`操作

## 技术实现细节

### Redis Stream响应结构

Redis `XREADGROUP`命令返回的数据结构：
```
[
  [
    "stream_name",
    [
      ["1234567890-0", ["field1", "value1", "field2", "value2"]],
      ["1234567891-0", ["field1", "value1", "field2", "value2"]]
    ]
  ]
]
```

### 解析逻辑

1. **外层循环**: 遍历每个Stream的数据
2. **条目循环**: 遍历每个Stream中的消息条目
3. **字段解析**: 将字段数组转换为HashMap
4. **消息重建**: 从`data`字段反序列化Message对象

### 错误处理

- **连接错误**: 自动重试连接
- **反序列化错误**: 记录警告但继续处理其他消息
- **Redis错误**: 转换为SchedulerError并传播
- **部分失败**: 允许部分消息处理失败，不影响其他消息

## 配置参数

### RedisStreamConfig相关参数

```rust
pub struct RedisStreamConfig {
    pub consumer_group_prefix: String,  // 消费者组前缀
    pub consumer_id: String,           // 消费者ID
    // ... 其他配置
}
```

### 消费行为配置

- **批量大小**: 每次最多读取100条消息
- **阻塞时间**: 等待新消息最多1秒
- **重试机制**: 继承发布消息的重试配置

## 使用示例

### 基本消费
```rust
let config = RedisStreamConfig::default();
let queue = RedisStreamMessageQueue::new(config)?;

// 消费消息
let messages = queue.consume_messages("task_queue").await?;
for message in messages {
    println!("Consumed message: {}", message.id);
    // 处理消息...
}
```

### 多消费者场景
```rust
// 消费者1
let config1 = RedisStreamConfig {
    consumer_id: "worker-001".to_string(),
    ..Default::default()
};
let queue1 = RedisStreamMessageQueue::new(config1)?;

// 消费者2
let config2 = RedisStreamConfig {
    consumer_id: "worker-002".to_string(),
    ..Default::default()
};
let queue2 = RedisStreamMessageQueue::new(config2)?;

// 两个消费者从同一个消费者组消费消息
let messages1 = queue1.consume_messages("task_queue").await?;
let messages2 = queue2.consume_messages("task_queue").await?;
```

## 性能特性

### 优势
- **高吞吐量**: Redis Stream原生支持高并发
- **持久化**: 消息持久化到磁盘
- **负载均衡**: 消费者组自动分配消息
- **故障恢复**: 待处理消息机制确保消息不丢失

### 注意事项
- **内存使用**: Stream会占用Redis内存
- **消息确认**: 需要及时确认消息以释放内存
- **消费者管理**: 需要合理管理消费者数量

## 测试覆盖

### 单元测试
- 消费者组名称生成
- 消息ID映射管理
- 队列名称验证
- 消息序列化/反序列化

### 集成测试
- 完整的发布-消费流程
- 多消费者场景
- 待处理消息处理
- 错误恢复机制

### 示例程序
- `redis_stream_consume_demo.rs`: 完整的消费功能演示

## 与RabbitMQ的兼容性

实现完全兼容MessageQueue trait接口：
- 相同的方法签名
- 相同的错误处理模式
- 相同的消息格式
- 无需修改业务代码即可切换

## 后续任务

当前实现完成了消息消费的核心功能，后续任务包括：
1. 完善ACK/NACK机制（任务6）
2. 实现队列管理功能（任务7）
3. 添加更多集成测试（任务8）
4. 实现配置切换机制（任务9）
5. 性能优化和监控（任务10）