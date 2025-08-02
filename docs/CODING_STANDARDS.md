# 调度器项目编码规范

## 🎯 概述

本文档定义了调度器项目的统一编码规范，确保代码的一致性、可维护性和可读性。

## 📋 异步函数签名规范

### 标准模式

```rust
// ✅ 推荐：方法签名
pub async fn method_name(&self, param: Type) -> SchedulerResult<ReturnType> {
    // 实现
}

// ✅ 推荐：函数签名  
pub async fn function_name(param: &Type) -> SchedulerResult<ReturnType> {
    // 实现
}

// ✅ 推荐：创建函数
pub async fn new(config: Config) -> SchedulerResult<Self> {
    // 实现
}
```

### 返回类型规范

```rust
// ✅ 使用项目统一的 Result 类型
use crate::SchedulerResult;

// ✅ 无返回值
async fn operation(&self) -> SchedulerResult<()>

// ✅ 有返回值
async fn get_data(&self) -> SchedulerResult<Data>

// ✅ 泛型返回值
async fn process<T>(&self, input: T) -> SchedulerResult<T>
```

### 参数规范

```rust
// ✅ 借用参数
async fn process_config(&self, config: &Config) -> SchedulerResult<()>

// ✅ 移动参数（当需要所有权时）
async fn consume_data(&self, data: Data) -> SchedulerResult<()>

// ✅ 可选参数
async fn optional_param(&self, required: &str, optional: Option<&str>) -> SchedulerResult<()>
```

## 📝 文档注释规范

### 模块文档

```rust
//! 任务调度核心模块
//!
//! 此模块提供任务调度的核心功能，包括：
//! - 任务创建和管理
//! - 调度策略执行
//! - 依赖关系处理
//!
//! # 示例
//!
//! ```rust
//! use scheduler_core::TaskScheduler;
//! 
//! let scheduler = TaskScheduler::new(config).await?;
//! scheduler.start().await?;
//! ```
```

### 结构体文档

```rust
/// 任务执行器注册表
///
/// 管理所有可用的任务执行器，支持动态注册和查找。
/// 执行器按类型分类，每种类型可以有多个实现。
///
/// # 线程安全
/// 
/// 此结构体是线程安全的，可以在多个异步任务间共享。
#[derive(Debug)]
pub struct ExecutorRegistry {
    executors: HashMap<String, Box<dyn TaskExecutor>>,
}
```

### 函数文档

```rust
/// 创建新的任务运行实例
///
/// 根据任务定义创建一个新的运行实例，并初始化必要的上下文信息。
///
/// # 参数
///
/// * `task` - 要执行的任务定义
/// * `trigger_type` - 触发类型（手动/定时/依赖）
/// * `context` - 执行上下文信息
///
/// # 返回值
///
/// 成功时返回新创建的 `TaskRun` 实例，失败时返回 `SchedulerError`。
///
/// # 错误
///
/// * `ValidationError` - 任务定义无效
/// * `ResourceError` - 资源不足
/// * `DatabaseError` - 数据库操作失败
///
/// # 示例
///
/// ```rust
/// let task_run = scheduler.create_task_run(
///     &task,
///     TriggerType::Manual,
///     context
/// ).await?;
/// ```
pub async fn create_task_run(
    &self,
    task: &Task,
    trigger_type: TriggerType,
    context: Option<TaskContext>,
) -> SchedulerResult<TaskRun> {
    // 实现
}
```

### 特征文档

```rust
/// 任务执行器特征
///
/// 定义任务执行的核心接口。所有执行器都必须实现此特征。
///
/// # 实现要求
///
/// - 必须是线程安全的（Send + Sync）
/// - 支持异步执行
/// - 提供详细的错误信息
/// - 支持执行状态查询
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// 执行任务
    async fn execute(&self, task: &Task) -> SchedulerResult<TaskResult>;
}
```

## 🔧 错误处理规范

### 统一错误类型

```rust
// ✅ 使用项目统一的错误类型
use crate::errors::SchedulerError;
use crate::SchedulerResult;

// ✅ 函数签名
async fn operation() -> SchedulerResult<T> {
    // 使用 ? 操作符传播错误
    let result = some_operation().await?;
    Ok(result)
}
```

### 错误创建和传播

```rust
// ✅ 创建具体错误
return Err(SchedulerError::ValidationError(
    "任务名称不能为空".to_string()
));

// ✅ 错误链式传播
some_operation()
    .await
    .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;
```

## 🏗️ 结构化编程规范

### 构建器模式

```rust
/// 任务构建器
#[derive(Default)]
pub struct TaskBuilder {
    name: Option<String>,
    schedule: Option<String>,
    executor: Option<String>,
}

impl TaskBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置任务名称
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 构建任务实例
    pub fn build(self) -> SchedulerResult<Task> {
        let name = self.name.ok_or_else(|| 
            SchedulerError::ValidationError("任务名称是必需的".to_string())
        )?;
        
        Ok(Task::new(name))
    }
}
```

### 服务层模式

```rust
/// 任务服务实现
pub struct TaskService {
    repository: Arc<dyn TaskRepository>,
    executor_registry: Arc<dyn ExecutorRegistry>,
}

impl TaskService {
    /// 创建服务实例
    pub fn new(
        repository: Arc<dyn TaskRepository>,
        executor_registry: Arc<dyn ExecutorRegistry>,
    ) -> Self {
        Self {
            repository,
            executor_registry,
        }
    }
}

#[async_trait]
impl TaskControlService for TaskService {
    async fn trigger_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
        // 实现
    }
}
```

## 🧪 测试规范

### 测试文件命名

```text
// ✅ 单元测试
src/
  task_service.rs
  task_service_test.rs  // 或在同文件内 #[cfg(test)] mod tests

// ✅ 集成测试  
tests/
  integration_test_basic.rs
  integration_test_advanced.rs
```

### 测试函数命名

```rust
#[tokio::test]
async fn test_create_task_success() {
    // 成功案例测试
}

#[tokio::test]
async fn test_create_task_validation_error() {
    // 验证错误测试
}

#[tokio::test]
async fn test_create_task_database_error() {
    // 数据库错误测试
}
```

## 📦 模块组织规范

### 文件结构

```
crate/
├── src/
│   ├── lib.rs           // 公开API
│   ├── error.rs         // 错误定义
│   ├── models/          // 数据模型
│   │   ├── mod.rs
│   │   ├── task.rs
│   │   └── worker.rs
│   ├── services/        // 服务层
│   │   ├── mod.rs
│   │   └── task_service.rs
│   └── traits/          // 特征定义
│       ├── mod.rs
│       └── repository.rs
├── tests/               // 集成测试
└── examples/            // 示例代码
```

### 导入规范

```rust
// ✅ 标准库导入
use std::collections::HashMap;
use std::sync::Arc;

// ✅ 第三方依赖
use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, info, error};

// ✅ 项目内部导入
use crate::models::{Task, TaskRun};
use crate::errors::SchedulerError;
use crate::SchedulerResult;

// ✅ 父级模块导入
use super::TaskRepository;
```

## 🚀 性能规范

### 异步最佳实践

```rust
// ✅ 并发操作
let (result1, result2) = tokio::join!(
    operation1(),
    operation2()
);

// ✅ 并行处理
let results: Vec<_> = stream::iter(items)
    .map(|item| process_item(item))
    .buffer_unordered(10)
    .collect()
    .await;

// ✅ 超时控制
let result = tokio::time::timeout(
    Duration::from_secs(30),
    long_running_operation()
).await??;
```

### 内存优化

```rust
// ✅ 避免不必要的克隆
async fn process_data(data: &[u8]) -> SchedulerResult<()> {
    // 直接使用引用
}

// ✅ 使用 Arc 共享数据
struct Service {
    config: Arc<Config>,  // 多个地方共享配置
}
```

## ✅ 代码质量检查点

1. **编译检查**: `cargo check --all-targets`
2. **格式化**: `cargo fmt --check`
3. **静态分析**: `cargo clippy -- -D warnings`
4. **测试覆盖**: `cargo test --all`
5. **文档生成**: `cargo doc --no-deps`

---

*此规范将持续更新，以适应项目发展需求。*
