# 系统优化与重构执行方案

## **系统优化与重构执行方案（优化版）**

本方案基于系统分析报告，旨在将发现的问题转化为具体的、可执行的任务列表。方案分为两个主要阶段：

* **第一阶段：关键问题修复与核心功能实现** - 优先解决阻碍系统正常运行的严重问题，确保系统稳定可用。
* **第二阶段：架构重构与设计原则对齐** - 专注于提升代码质量、可维护性和长期扩展性，使系统架构更加清晰和健壮。

每个任务都包含**优先级**、**问题分析**、**待修改文件**、**优化后执行方案**、**验收标准**和**测试验收标准**。

---

### **第一阶段：关键问题修复与核心功能实现**

此阶段的目标是确保系统核心功能完整、稳定且可靠。

#### **任务 1.1: 實現 Worker Service 核心业务逻辑**

* **优先级**: **严重 (Critical)**
* **问题分析**: `WorkerServiceImpl` 及其核心组件 `TaskExecutionManager` 目前是占位符实现，无法实际执行任务，导致 Worker 节点形同虚设。这是系统的核心功能缺失。
* **待修改文件**:
  * `crates/worker/src/components/task_execution.rs`
  * `crates/worker/src/components/worker_lifecycle.rs`
  * `crates/application/src/use_cases/worker_service.rs`
* **优化后执行方案**:
    1. **在 `TaskExecutionManager::handle_task_execution` 中**:
        * 根据 `TaskExecutionMessage` 中的 `task_type`，从 `executor_registry` 查找对应的 `TaskExecutor`。
        * **错误处理**: 若找不到执行器，应立即调用 `status_callback` 将任务状态更新为 `Failed`，并附上明确的错误信息 "Unsupported task type"。
        * **并发执行**: 将任务的实际执行过程（调用 `executor.execute_task` 及后续处理）包裹在 `tokio::spawn` 中，以确保 Worker 的主轮询循环不会被单个任务阻塞。
        * 在 `tokio::spawn` 任务内部：
            * 首先，调用 `status_callback` 将任务状态更新为 `Running`。
            * 然后，安全地执行 `executor.execute_task(&context).await`。
            * 执行完成后，从 `running_tasks` 集合中移除该任务的记录。
            * 最后，根据 `execute_task` 的返回结果 (`Ok` 或 `Err`)，再次调用 `status_callback` 将最终状态（`Completed` 或 `Failed`）连同任务产出或错误信息发送出去。
    2. **在 `WorkerLifecycle::poll_and_execute_tasks` 中**:
        * 细化消息确认（ACK）逻辑：只有当任务成功分派给 `TaskExecutionManager` 后，才对消息进行 ACK。若因 Worker 满载等原因导致分派失败，应考虑不确认（NACK）并让消息能被重新投递。
    3. **在 `worker_service.rs` 中**:
        * 移除所有占位符逻辑，确认其职责仅作为 `worker` crate 中各个组件的协调者和外部接口。
* **验收标准**:
  * `TaskExecutionManager` 能够根据任务类型动态选择并调用正确的执行器。
  * 任务执行过程完全异步，不会阻塞 Worker 主循环。
  * 任务的 `Running`, `Completed`, `Failed` 状态能被准确地通过回调更新。
* **测试验收标准**:
  * **单元测试**: 为 `TaskExecutionManager` 添加单元测试，覆盖成功、失败和找不到执行器的场景，验证 `status_callback` 的调用是否符合预期。
  * **集成测试**: 编写一个端到端的集成测试：
        1. 通过 API 创建一个 Shell 任务。
        2. 手动触发该任务。
        3. 验证 Worker 能消费到任务消息。
        4. 验证数据库中任务状态的流转：`Pending` -> `Dispatched` -> `Running` -> `Completed`。
        5. 验证任务执行结果被正确记录。
        6. 创建一个不支持的任务类型，触发后验证其状态最终变为 `Failed`。

#### **任务 1.2: 修复内存消息队列中的信号量泄漏**

* **优先级**: **高 (High)**
* **问题分析**: `InMemoryMessageQueue` 中 `permit.forget()` 的使用方式存在缺陷，当消费者未能及时处理消息时，会导致信号量许可永久性泄漏，最终使队列无法接收新消息，造成服务中断。
* **待修改文件**:
  * `crates/infrastructure/src/in_memory_queue.rs`
* **优化后执行方案**:
    1. **首选方案（推荐）**：将 `mpsc::UnboundedSender` 和 `Semaphore` 的组合替换为 `mpsc::channel`（有界通道），这是 Tokio 中处理背压的标准且更安全的方式。
        * 修改 `QueueChannels` 结构体，将 `sender` 类型改为 `mpsc::Sender<Message>`，并彻底移除 `backpressure_semaphore`。
        * 在 `get_or_create_queue` 方法中，使用 `mpsc::channel(self.config.max_queue_size)` 来创建有界通道。
        * 修改 `publish_message` 方法，移除所有与 `Semaphore` 相关的逻辑。将原有的 `sender.send(...)` 替换为 `sender.send(...).await`，并使用 `tokio::time::timeout` 包裹以处理背压超时，确保发送方不会无限期阻塞。
    2. 在 `consume_messages` 中，移除 `semaphore.add_permits(1)` 逻辑，因为有界通道会自动管理容量。
* **验收标准**:
  * 代码中不再使用 `tokio::sync::Semaphore` 和 `permit.forget()` 来实现背压。
  * `InMemoryMessageQueue` 内部使用 `mpsc::channel`（有界通道）实现队列。
  * 当队列满时，发布消息的操作能够正确地异步等待或在超时后返回错误。
* **测试验收标准**:
  * 编写一个专门的单元测试来验证新的背压机制：
        1. 创建一个容量极小（如 5）的内存队列。
        2. 连续发送 5 条消息，应全部立即成功。
        3. 在一个 `tokio::spawn` 中尝试发送第 6 条消息，验证该操作被阻塞。
        4. 在主测试流程中消费 1 条消息。
        5. 验证第 6 条消息的发送操作现在能够成功完成。
        6. 再次填满队列，然后使用带超时（如 100ms）的发布操作，验证其在超时后会返回错误。

#### **任务 1.3: 實現优雅的任务取消机制**

* **优先级**: **中 (Medium)**
* **问题分析**: 任务取消机制（`handle.abort()` 和 `kill` 命令）过于粗暴。直接终止可能导致子进程成为孤儿进程，或使资源（如临时文件）得不到清理，存在资源泄漏和数据不一致的风险。
* **待修改文件**:
  * `crates/application/src/ports/executor.rs` (修改 `TaskExecutor` trait)
  * `crates/worker/src/executors.rs`
  * `crates/worker/src/components/worker_lifecycle.rs`
* **优化后执行方案**:
    1. 修改 `TaskExecutor` trait 中的 `cancel` 方法签名为 `async fn cancel(&self, task_run_id: i64) -> SchedulerResult<()>`，使其成为异步操作。
    2. **在 `ShellExecutor::cancel` 中**:
        * 对于 `unix` 平台，首先通过 `tokio::process::Command` 发送 `SIGTERM` 信号，请求进程优雅退出。
        * 使用 `tokio::time::timeout` 结合 `child.wait().await` 等待一个宽限期（例如 2-5 秒）。
        * 如果超时，说明进程未能优雅退出，再发送 `SIGKILL` 信号进行强制终止。
    3. **在 `HttpExecutor::cancel` 中**：
        * `reqwest` 的 `Future` 在被丢弃时会自动取消底层的 HTTP 请求，`handle.abort()` 的效果与此类似。此处的实现可以保持不变，但要确保在文档中注明其行为。
    4. **在 `WorkerLifecycle::stop` 中**:
        * 获取所有正在运行的任务列表。
        * 遍历任务列表，为每个任务的取消操作创建一个异步 future。
        * 使用 `futures::future::join_all` 并发地等待所有 `cancel` 操作完成，并处理可能出现的错误。
* **验收标准**:
  * `TaskExecutor::cancel` 是异步方法。
  * `ShellExecutor` 实现了先 `SIGTERM` 后 `SIGKILL` 的两阶段终止逻辑。
  * `WorkerLifecycle::stop` 能够等待所有正在运行的任务完成其取消流程。
* **测试验收标准**:
  * 编写集成测试：
        1. 启动一个长时间运行的 Shell 任务（例如 `sleep 20`）。
        2. 在任务运行期间，调用 `TaskExecutionManager::cancel_task`。
        3. 验证 `sleep` 进程被成功终止。
        4. 验证数据库中对应的 `TaskRun` 状态被更新为 `Failed` 或一个新的 `Cancelled` 状态，并记录了取消原因。

#### **任务 1.4: 集成真实的用户认证与管理**

* **优先级**: **高 (High)**
* **问题分析**: API 认证目前依赖硬编码的用户凭据 (`validate_user_credentials`)，这在任何生产或共享环境中都是严重的安全漏洞，且缺乏可管理性。
* **待修改文件**:
  * `crates/api/src/handlers/auth.rs`
  * `crates/api/src/routes.rs`
  * `src/app.rs` 和 `src/embedded/app.rs`
  * `crates/infrastructure/src/database/manager.rs`
* **优化后执行方案**:
    1. 从 `crates/api/src/handlers/auth.rs` 中彻底移除 `validate_user_credentials` 函数。
    2. 在 `AppState` 结构体中添加 `authentication_service: Arc<AuthenticationService>`。
    3. 在 `Application::new` 和 `EmbeddedApplication::start` 的初始化流程中：
        * 根据数据库类型（Postgres 或 SQLite），创建对应的 `UserRepository` 实例 (`PostgresUserRepository` 或 `SqliteUserRepository`)。
        * 使用此 `UserRepository` 实例来构建 `AuthenticationService`。
        * 将 `Arc` 包装的 `AuthenticationService` 注入到 `AppState` 中。
    4. 修改 `login` API 处理器，使其调用 `state.authentication_service.authenticate_user(...)` 来完成用户验证。
* **验收标准**:
  * API 层代码中不再包含任何硬编码的用户凭据。
  * 登录逻辑完全委托给应用层的 `AuthenticationService`。
  * 系统能根据配置自动选择并实例化正确的 `UserRepository` 实现。
* **测试验收标准**:
  * 重构 `auth_tests.rs` 和 `auth_extended_tests.rs`。
  * 在测试的初始化阶段，通过 `UserRepository` 接口在测试数据库中创建一个临时测试用户。
  * 测试用例应覆盖：
    * 使用正确凭证登录，成功返回 JWT。
    * 使用错误密码登录，返回 401 Unauthorized。
    * 使用不存在的用户名登录，返回 401 Unauthorized。

---

### **第二阶段：架构重构与设计原则对齐**

此阶段的目标是优化代码结构，使其更符合六边形架构，提升代码的长期可维护性。

#### **任务 2.1: 澄清 Crate 职责，强化六边形架构**

* **优先级**: **中 (Medium)**
* **问题分析**: `dispatcher` 和 `worker` crate 的职责边界模糊，混合了应用层业务逻辑（如调度算法）和基础设施层代码（如HTTP客户端），违反了六边形架构的依赖规则。
* **待修改文件**:
  * `crates/dispatcher/**`
  * `crates/worker/**`
  * `crates/application/src/use_cases/**`
  * `crates/infrastructure/**`
* **优化后执行方案**:
    1. 将 `crates/dispatcher` 中的核心业务逻辑（如 `scheduler.rs`, `retry_service.rs`, `strategies.rs`）移动到 `crates/application/src/use_cases/` 下，并重命名以反映其应用服务职责（如 `SchedulerService`）。
    2. 将 `crates/worker/src/components/dispatcher_client.rs`（一个具体的 HTTP 客户端实现）移动到 `crates/infrastructure/src/clients/dispatcher_client.rs`。
    3. 在 `crates/application/src/ports/` 中定义新的 `trait DispatcherApiClient`，抽象 Worker 与 Dispatcher 之间的通信接口（如 `register`, `send_heartbeat`）。
    4. 让 `infrastructure` 中的 `DispatcherClient` 实现 `DispatcherApiClient` trait。
    5. 修改 `worker` crate 中的服务，使其依赖 `Arc<dyn DispatcherApiClient>` 这一抽象接口，而不是具体的 `DispatcherClient` 实现。
    6. 将 `dispatcher` 和 `worker` 两个 crate 的职责简化为应用的“组合根”（Composition Root），即负责组装 `application` 层的服务和 `infrastructure` 层的适配器，并包含 `main.rs` 作为启动入口。
* **验收标准**:
  * `application` 层不再直接依赖 `reqwest` 等任何具体的基础设施库。
  * `dispatcher` 和 `worker` crate 的代码量显著减少，主要负责组件的组装和启动。
  * 核心调度、重试和策略算法位于 `application` crate。
* **测试验收标准**:
  * **无需编写新的功能测试**。
  * 重构后，所有现有的集成测试必须全部通过，以确保没有引入回归性缺陷。

#### **任务 2.2: 统一并简化启动逻辑 (DRY)**

* **优先级**: **中 (Medium)**
* **问题分析**: `src/bin/*.rs` 中的多个二进制入口文件包含了大量重复的 `clap` 参数配置和应用启动样板代码，违反了 DRY (Don't Repeat Yourself) 原则。
* **待修改文件**:
  * `src/bin/*.rs`
  * `src/common.rs`
* **优化后执行方案**:
    1. 在 `src/common.rs` 中，创建一个 `CliBuilder` 或扩展 `start_application` 函数，用于统一处理通用的命令行参数，如 `--config`, `--log-level`, `--log-format`。
    2. 重构每个 `src/bin/*.rs` 中的 `main` 函数：
        * 调用公共函数来创建基础的 `clap::Command`。
        * 仅添加该二进制文件特有的参数（例如 `worker` 的 `--worker-id`）。
        * 调用统一的 `run_cli` 函数来完成参数解析和应用启动。
* **验收标准**:
  * `src/bin/*.rs` 文件中的代码量大幅减少。
  * 添加或修改一个通用参数（如 `--verbose`）只需在 `src/common.rs` 中修改一处。
* **测试验收标准**:
  * 手动执行每个二进制文件（如 `cargo run --bin scheduler-api -- --help`），验证帮助信息仍然正确、完整。
  * 确保所有依赖于二进制启动的测试（若有）仍然通过。

#### **任务 2.3: 分解大型服务 (SRP)**

* **优先级**: **低 (Low)** (此项优化可以在前述重构稳定后再进行)
* **问题分析**: `TaskScheduler` 服务的职责过于宽泛，包括扫描任务、检查依赖、创建运行实例和分发到队列，违反了单一职责原则（SRP），使其难以测试和维护。
* **待修改文件**:
  * `crates/application/src/use_cases/scheduler_service.rs`
* **优化后执行方案**:
    1. 将 `TaskScheduler` 拆分为几个职责更单一的服务：
        * `TaskScannerService`: 仅负责从数据库中查询符合调度条件的任务。
        * `TaskPlanningService`: 接收任务列表，负责检查依赖关系和并发限制，生成一个可执行的任务计划。
        * `TaskDispatchService`: 接收任务计划，负责创建 `TaskRun` 记录，并将任务消息发布到消息队列。
    2. 重构原有的 `run_scheduler_loop` 逻辑，使其成为一个协调者，按顺序调用这三个新服务。
* **验收标准**:
  * `TaskScheduler` 结构体被移除或其职责被大幅简化为一个协调者。
  * 创建了新的、职责单一的服务，并分别实现了原有功能。
  * 每个新服务的公共接口清晰地反映其单一职责。
* **测试验收标准**:
  * 为 `TaskScannerService`, `TaskPlanningService`, `TaskDispatchService` 分别编写单元测试。
  * 修改现有的调度器集成测试，验证这三个服务协同工作时，其最终效果与重构前保持一致。
