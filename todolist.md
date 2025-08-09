# 项目重构 TODO 列表

记录时间: 2025-08-09

## 已完成

- [x] 将 core/models 中的领域实体移动到 crates/domain/src/entities.rs，并更新引用 (id: todo-1)

## 进行中

- [~] 将 core/traits/repository.rs 中的仓储接口移动到 crates/domain/src/repositories.rs，并调整依赖 (id: todo-2)

## 待办

- [ ] 将基础设施接口（如 message_queue trait）从 core 移到新的 application 接口层或单独的 application Crate (id: todo-3)
- [ ] 删除 core/services 与 core/service_interfaces.rs，将实际服务实现移至 dispatcher 与 worker Crate，并更新接口定义 (id: todo-4)
- [ ] 移除 core/error_handling.rs 的过度设计，保留 SchedulerError 与每个 Config 的 validate() 方法；删除 ValidatorRegistry 框架 (id: todo-5)
- [ ] 更新所有 Crate 的 Cargo.toml 以反映新的依赖关系（domain 取代 core 的职责，infrastructure 依赖 domain） (id: todo-6)
- [ ] 在 worker Crate 中完成重构：确定最终实现（composed_service.rs 或 refactored_service.rs）、删除旧代码、更新并运行相关测试 (id: todo-7)
- [ ] 将 infrastructure/src/database/postgres/task_query_builder.rs 的逻辑提取并移动到 dispatcher Crate，同时保留 PostgresTaskRepository 作为实现者 (id: todo-8)
- [ ] 将 infrastructure/src/database/postgres/task_dependency_checker.rs 的业务逻辑抽出到 dispatcher Crate，postgres 实现仅调用该逻辑 (id: todo-9)
- [ ] 创建 crates/testing-utils Crate，并将共享 Mock（MockTaskRepository 等）与测试辅助（DatabaseTestContainer）迁移至该 Crate (id: todo-10)
- [ ] 更新所有测试模块的依赖以使用 testing-utils，重构重复 Mock 并统一测试命名规范 (id: todo-11)
- [ ] 修复 api/handlers/auth.rs 中硬编码的凭证：替换为配置驱动实现并在代码/注释中强烈警告（不可用于生产） (id: todo-12)
- [ ] 精简配置验证：移除 validation.rs 中的通用 ValidatorRegistry，保留每个 Config 的 validate() 并更新调用点 (id: todo-13)
- [ ] 同步并更新文档：ARCHITECTURE.md、API_DOCUMENTATION.md 与 config 示例（开发/生产），确保与重构后实现一致 (id: todo-14)
- [ ] 运行并修复代码质量与 CI：cargo fmt --check、cargo clippy、cargo test、更新 ci.yml 如有需要，确保所有检查通过 (id: todo-15)

---

如需我将此文件加入 git 提交或继续执行下一项（例如完成 todo-2 或运行 cargo check），请指示。
