# 开发指南

## 🎯 开发环境

### 必需工具
- **Rust**: 1.70+ 
- **PostgreSQL**: 13+
- **RabbitMQ**: 3.8+
- **Docker**: 用于依赖服务
- **Git**: 版本控制

### 开发工具推荐
```bash
# 安装开发工具
cargo install cargo-watch cargo-expand
rustup component add rustfmt clippy

# IDE推荐
# - VS Code + rust-analyzer
# - IntelliJ IDEA + Rust plugin
```

## 🏗️ 项目结构

### 代码组织
```
crates/
├── api/              # HTTP API服务
├── dispatcher/       # 任务调度器  
├── worker/           # 任务执行器
├── application/      # 业务逻辑层
├── infrastructure/   # 基础设施层
├── domain/           # 领域模型
├── config/           # 配置管理
├── observability/    # 监控日志
└── errors/           # 错误处理
```

### 依赖关系
- **errors** → 无依赖，被所有模块使用
- **domain** → 依赖 errors
- **config** → 依赖 errors  
- **infrastructure** → 依赖 domain, config
- **application** → 依赖 domain, infrastructure
- **api/dispatcher/worker** → 依赖 application

## 💻 编码规范

### Rust代码风格

```rust
// ✅ 正确：使用snake_case命名
pub struct TaskRepository {
    pool: PgPool,
}

// ✅ 正确：函数命名清晰
pub async fn create_task(&self, request: CreateTaskRequest) -> SchedulerResult<Task> {
    // 实现逻辑
}

// ✅ 正确：错误处理
let task = self.repository.save(&task)
    .await
    .map_err(|e| SchedulerError::DatabaseError(e.to_string()))?;

// ❌ 错误：使用unwrap()
let task = self.repository.save(&task).await.unwrap();
```

### 异步编程规范

```rust
// ✅ 正确：使用async/await
#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn save(&self, task: &Task) -> SchedulerResult<Task> {
        let query = sqlx::query!(
            "INSERT INTO tasks (name, command) VALUES ($1, $2) RETURNING id",
            task.name,
            task.command
        );
        
        let row = query.fetch_one(&self.pool).await?;
        Ok(Task { id: Some(row.id), ..task.clone() })
    }
}

// ✅ 正确：并发处理
let tasks = futures::future::join_all(
    task_ids.iter().map(|id| self.execute_task(*id))
).await;
```

### 错误处理模式

```rust
// ✅ 正确：统一错误类型
#[derive(Debug)]
pub enum SchedulerError {
    ValidationError(String),
    DatabaseError(String),
    MessageQueueError(String),
    TaskExecutionError(String),
}

// ✅ 正确：错误转换
impl From<sqlx::Error> for SchedulerError {
    fn from(err: sqlx::Error) -> Self {
        SchedulerError::DatabaseError(err.to_string())
    }
}

// ✅ 正确：Result类型别名
pub type SchedulerResult<T> = Result<T, SchedulerError>;
```

## 🧪 测试策略

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_create_task() {
        // Arrange
        let repository = MockTaskRepository::new();
        let service = TaskService::new(repository);
        
        let request = CreateTaskRequest {
            name: "test_task".to_string(),
            executor: "shell".to_string(),
            command: "echo hello".to_string(),
        };
        
        // Act
        let result = service.create_task(request).await;
        
        // Assert
        assert!(result.is_ok());
        let task = result.unwrap();
        assert_eq!(task.name, "test_task");
    }
}
```

### 集成测试

```rust
// tests/integration/api_tests.rs
use testcontainers::{clients::Cli, images::postgres::Postgres, Container};

#[tokio::test]
async fn test_task_api_integration() {
    // 启动测试容器
    let docker = Cli::default();
    let postgres_container = docker.run(Postgres::default());
    
    // 设置测试数据库
    let database_url = format!(
        "postgres://postgres@localhost:{}/postgres",
        postgres_container.get_host_port_ipv4(5432)
    );
    
    // 运行集成测试
    let app = create_test_app(&database_url).await;
    let response = app
        .post("/api/tasks")
        .json(&create_task_request())
        .send()
        .await;
        
    assert_eq!(response.status(), 201);
}
```

### 性能基准测试

```rust
// benches/task_processing.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_task_creation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = rt.block_on(async { create_task_service().await });
    
    c.bench_function("create_task", |b| {
        b.to_async(&rt).iter(|| async {
            let request = black_box(create_task_request());
            service.create_task(request).await.unwrap()
        });
    });
}

criterion_group!(benches, bench_task_creation);
criterion_main!(benches);
```

## 📝 开发流程

### 分支管理

```bash
# 主分支
main          # 生产代码
develop       # 开发分支

# 功能分支
feature/task-dependencies
feature/worker-scaling
bugfix/memory-leak

# 发布分支
release/v1.0.0
hotfix/critical-fix
```

### 提交规范

```bash
# 提交消息格式
type(scope): description

# 类型
feat:     新功能
fix:      Bug修复  
docs:     文档更新
style:    代码格式调整
refactor: 重构
test:     测试相关
chore:    构建过程或工具变动

# 示例
feat(api): add task dependency management
fix(dispatcher): resolve memory leak in task scheduler
docs(readme): update installation instructions
```

### Code Review检查清单

**功能性**
- [ ] 功能按需求正确实现
- [ ] 边界条件处理完善
- [ ] 错误处理机制健全

**代码质量**  
- [ ] 代码风格符合项目规范
- [ ] 命名清晰有意义
- [ ] 函数和模块职责单一
- [ ] 无不必要的代码重复

**性能与安全**
- [ ] 无明显性能问题
- [ ] 内存使用合理
- [ ] 避免SQL注入等安全问题
- [ ] 敏感信息正确处理

**测试覆盖**
- [ ] 核心逻辑有单元测试
- [ ] 集成测试覆盖主要场景
- [ ] 测试用例充分

## 🔧 调试技巧

### 日志调试

```rust
// 使用结构化日志
use tracing::{info, warn, error, debug, span, Level};

pub async fn execute_task(&self, task_id: i64) -> SchedulerResult<TaskRun> {
    let span = span!(Level::INFO, "execute_task", task_id);
    let _enter = span.enter();
    
    info!("开始执行任务");
    
    match self.do_execute(task_id).await {
        Ok(result) => {
            info!(duration_ms = ?result.duration, "任务执行成功");
            Ok(result)
        }
        Err(e) => {
            error!(error = %e, "任务执行失败");
            Err(e)
        }
    }
}

// 运行时启用调试日志
RUST_LOG=scheduler=debug cargo run --bin api
```

### 性能分析

```bash
# 使用perf进行性能分析
cargo build --release
perf record --call-graph dwarf target/release/scheduler-api
perf report

# 使用valgrind检查内存
cargo build
valgrind --tool=memcheck target/debug/scheduler-api

# 基准测试
cargo bench
```

### 调试工具

```bash
# 代码格式检查
cargo fmt --check

# 静态分析
cargo clippy -- -D warnings

# 依赖检查
cargo audit

# 文档生成
cargo doc --no-deps --open

# 展开宏
cargo expand
```

## 🚀 部署与发布

### 本地构建

```bash
# 开发构建
cargo build

# 生产构建  
cargo build --release

# 交叉编译（如Linux目标）
cargo build --release --target x86_64-unknown-linux-gnu
```

### Docker构建

```dockerfile
# 多阶段构建
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/scheduler-api /usr/local/bin/
CMD ["scheduler-api"]
```

### 发布检查清单

**代码质量**
- [ ] 所有测试通过
- [ ] 代码审查完成
- [ ] 性能测试通过
- [ ] 安全扫描通过

**文档更新**
- [ ] API文档更新
- [ ] 配置说明更新  
- [ ] 部署文档更新
- [ ] 变更日志更新

**部署准备**
- [ ] 数据库迁移脚本准备
- [ ] 配置文件模板准备
- [ ] 监控告警配置
- [ ] 回滚方案准备

## 📚 学习资源

### Rust相关
- [Rust官方文档](https://doc.rust-lang.org/)
- [Async Rust](https://rust-lang.github.io/async-book/)
- [Tokio教程](https://tokio.rs/tokio/tutorial)

### 架构设计
- [领域驱动设计](https://www.domainlanguage.com/ddd/)
- [清洁架构](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
- [微服务模式](https://microservices.io/patterns/)

### 工具链
- [SQLx文档](https://docs.rs/sqlx/)
- [Axum指南](https://docs.rs/axum/)
- [Tracing指南](https://docs.rs/tracing/)