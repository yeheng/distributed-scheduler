# 分布式任务调度系统需求文档

## 简介

本文档定义了一个分布式任务调度系统的功能需求。该系统允许用户定义定时任务，由调度器（Dispatcher）负责任务调度，多个工作节点（Worker）负责任务执行。

## 需求

### 需求 1 - 核心任务调度与执行 (MVP)

**用户故事:** 作为系统，我需要能够根据预设时间表自动调度任务，并由 Worker 节点可靠地执行它们。

#### 验收标准

1. **WHEN** 一个 `tasks` 表中的记录其 `schedule` (cron) 到达执行时间且 `status` 为 `ACTIVE` **THEN** 系统 **SHALL** 创建一条新的 `task_executions` 记录，状态为 `PENDING`，并将任务消息发送到 消息队列。
2. **WHEN** 任务消息成功发送到 消息队列 **THEN** 系统 **SHALL** 将对应的 `task_executions` 记录状态更新为 `DISPATCHED`。
3. **WHEN** Worker 从 消息队列 消费到一条任务消息 **THEN** 它 **SHALL** 向 消息队列 发送 `ack` 确认，并立即向状态更新队列发送一条 `RUNNING` 状态的消息。
4. **WHEN** Worker 成功执行任务 **THEN** 系统 **SHALL** 向状态更新队列发送 `COMPLETED` 状态及执行结果。
5. **WHEN** Worker 执行任务失败 **THEN** 系统 **SHALL** 向状态更新队列发送 `FAILED` 状态及错误信息。
6. **WHEN** Dispatcher 的 State Listener 收到状态更新消息 **THEN** 系统 **SHALL** 在 RDBMS 的 `task_executions` 表中原子性地更新对应的状态、结束时间和结果。
7. **WHEN** 任务执行超过预设的超时时间 **THEN** 系统 **SHALL** 将任务状态标记为 `TIMEOUT` 并终止执行。

### 需求 2 - Worker 节点生命周期管理 (MVP)

**用户故事:** 作为系统管理员，我希望 Worker 节点可以动态加入和退出系统，系统应能自动感知其变化。

#### 验收标准

1. **WHEN** 一个新的 Worker 进程启动 **THEN** 它 **SHALL** 向 Dispatcher 的 `/api/workers` 接口发送 HTTP POST 请求进行注册，包含 Worker ID、主机信息和支持的任务类型。
2. **WHEN** Worker 成功注册 **THEN** Dispatcher **SHALL** 在其内部的 Worker Manager 中记录该 Worker 为 `ALIVE` 状态，并返回注册确认。
3. **WHEN** Worker 正在运行 **THEN** 它 **SHALL** 定期（例如每30秒）向 Dispatcher 的 `/api/workers/{id}/heartbeat` 接口发送 HTTP POST 心跳，包含当前负载信息。
4. **WHEN** Dispatcher 在预设阈值（例如90秒）内未收到某个 Worker 的心跳 **THEN** 系统 **SHALL** 将该 Worker 在 Worker Manager 中标记为 `DOWN`。
5. **WHEN** Worker 正常关闭 **THEN** 它 **SHALL** 向 Dispatcher 发送注销请求，完成当前任务后优雅退出。
6. **WHEN** 一个 Worker 被标记为 `DOWN` 且有正在执行的任务 **THEN** 系统 **SHALL** 将这些任务重新调度到其他可用的 Worker。

### 需求 3 - 基本任务管理 API (MVP)

**用户故事:** 作为开发者或管理员，我希望能通过 API 来创建、查看、更新和删除任务。

#### 验收标准

1. **WHEN** 用户向 `/api/tasks` 发送包含任务定义（名称, schedule, 参数, 超时时间等）的 POST 请求 **THEN** 系统 **SHALL** 验证输入参数，在 RDBMS 中创建一条新的任务记录，并返回 `201 Created` 及任务详情。
2. **WHEN** 用户向 `/api/tasks/{id}` 发送 GET 请求 **THEN** 系统 **SHALL** 返回该任务的详细信息，包括其最近的执行历史和统计信息。
3. **WHEN** 用户向 `/api/tasks` 发送 GET 请求 **THEN** 系统 **SHALL** 返回分页的任务列表，支持按状态、名称等条件过滤。
4. **WHEN** 用户向 `/api/tasks/{id}` 发送 PUT 请求 **THEN** 系统 **SHALL** 更新任务定义，并在下次调度时生效。
5. **WHEN** 用户向 `/api/tasks/{id}` 发送 DELETE 请求 **THEN** 系统 **SHALL** 将任务状态设为 `INACTIVE`，停止后续调度。
6. **WHEN** 用户向 `/api/tasks/{id}/trigger` 发送 POST 请求 **THEN** 系统 **SHALL** 立即触发一次任务执行，不受 schedule 限制。
7. **WHEN** API 请求包含无效参数 **THEN** 系统 **SHALL** 返回 `400 Bad Request` 及详细的错误信息。

### 需求 4 - 失败重试机制 (MVP)

**用户故事:** 作为开发者，我希望对于可能间歇性失败的任务，系统能自动进行重试。

#### 验收标准

1. **WHEN** 任务定义中 `max_retries > 0` **THEN** 系统 **SHALL** 启用重试逻辑。
2. **WHEN** Dispatcher 收到一个任务的 `FAILED` 状态更新，且其当前重试次数小于 `max_retries` **THEN** 系统 **SHALL** 创建新的执行记录，增加重试计数，并设置一个未来的 `next_run_time`（使用指数退避算法）。
3. **WHEN** 任务失败且已达到 `max_retries` **THEN** 系统 **SHALL** 将任务的最终状态置为 `FAILED` 且不再重试。
4. **WHEN** 系统进行重试调度 **THEN** 重试间隔 **SHALL** 使用指数退避策略，基础间隔可配置（如 2^retry_count * base_interval）。
5. **WHEN** Worker 因为系统故障（如网络断开、进程崩溃）无法完成任务 **THEN** 系统 **SHALL** 在检测到 Worker 下线后自动重试该任务。

### 需求 5 - 数据持久化与一致性 (MVP)

**用户故事:** 作为系统，我需要确保任务数据的持久化存储和状态一致性。

#### 验收标准

1. **WHEN** 系统创建或更新任务相关数据 **THEN** 所有操作 **SHALL** 在数据库事务中执行，确保 ACID 特性。
2. **WHEN** 多个组件同时访问同一任务数据 **THEN** 系统 **SHALL** 使用适当的锁机制防止竞态条件。
3. **WHEN** 系统重启 **THEN** 所有 `DISPATCHED` 和 `RUNNING` 状态的任务 **SHALL** 被重新评估和处理。
4. **WHEN** 数据库连接失败 **THEN** 系统 **SHALL** 实现重连机制，并在连接恢复后继续正常工作。

### 需求 6 - 错误处理与日志记录 (MVP)

**用户故事:** 作为开发者和运维人员，我需要详细的错误信息和日志来诊断问题。

#### 验收标准

1. **WHEN** 系统发生任何错误 **THEN** 系统 **SHALL** 记录结构化日志，包含时间戳、组件、错误级别和详细信息。
2. **WHEN** 任务执行失败 **THEN** 系统 **SHALL** 在 `task_executions` 表中记录详细的错误信息和堆栈跟踪。
3. **WHEN** 系统组件之间通信失败 **THEN** 系统 **SHALL** 记录通信错误并实施重试机制。
4. **WHEN** 用户查询执行历史 **THEN** 系统 **SHALL** 提供任务执行的完整日志和错误详情。

### 需求 7 - 可观测性集成 (Post-MVP)

**用户故事:** 作为运维人员，我希望能追踪任务的完整执行链路并监控系统的健康状况。

#### 验收标准

1. **WHEN** 一个任务从调度到执行完成的全过程 **THEN** 系统 **SHALL** 生成一个包含多个 Span 的 OpenTelemetry Trace，并能发送到 Jaeger 等后端。
2. **WHEN** 系统运行时 **THEN** Dispatcher 和 Worker **SHALL** 暴露一个 `/metrics` 端点，供 Prometheus 采集关键性能指标。
3. **WHEN** 系统运行时 **THEN** 关键指标 **SHALL** 包括任务执行成功率、平均执行时间、队列长度、Worker 健康状态等。

### 需求 8 - 安全性与权限控制 (Post-MVP)

**用户故事:** 作为系统管理员，我需要确保系统的安全性和访问控制。

#### 验收标准

1. **WHEN** 用户访问 API 端点 **THEN** 系统 **SHALL** 验证用户身份和权限。
2. **WHEN** 任务包含敏感参数 **THEN** 系统 **SHALL** 对敏感数据进行加密存储。
3. **WHEN** 系统组件间通信 **THEN** 系统 **SHALL** 使用 TLS 加密和身份验证。

### 需求 9 - 任务优先级与依赖 (Post-MVP)

**用户故事:** 作为高级用户，我希望能定义任务的执行优先级，并设置任务间的依赖关系。

#### 验收标准

1. **WHEN** 一个高优先级任务被调度 **THEN** 系统 **SHALL** 将其放入高优先级 MQ 队列或设置更高的消息优先级，使其能被 Worker 优先处理。
2. **WHEN** 一个任务被定义为依赖另一个任务 B **THEN** 系统 **SHALL** 仅在任务 B 的最近一次执行状态为 `COMPLETED` 后，才开始调度该任务。
3. **WHEN** 用户尝试删除一个被其他任务依赖的任务 **THEN** 系统 **SHALL** 拒绝该操作并返回错误信息。
4. **WHEN** 依赖任务执行失败 **THEN** 系统 **SHALL** 根据配置决定是否跳过或延迟依赖它的任务。

### 需求 10 - 性能与扩展性 (Post-MVP)

**用户故事:** 作为系统架构师，我需要系统能够处理大量任务并支持水平扩展。

#### 验收标准

1. **WHEN** 系统负载增加 **THEN** 系统 **SHALL** 支持添加更多 Worker 节点来水平扩展处理能力。
2. **WHEN** 任务队列积压 **THEN** 系统 **SHALL** 提供负载均衡机制，将任务分发到最适合的 Worker。
3. **WHEN** 系统处理大量并发任务 **THEN** 调度器 **SHALL** 维持稳定的性能，不出现明显的延迟增长。
4. **WHEN** 数据库查询量增大 **THEN** 系统 **SHALL** 使用适当的索引和查询优化来保持响应速度。
