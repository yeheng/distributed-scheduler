# 分布式任务调度系统 - 文档中心

## 📖 文档导航

| 文档 | 描述 | 适用对象 |
|------|------|----------|
| [快速开始](GETTING_STARTED.md) | 项目概览和环境搭建 | 新用户、开发者 |
| [系统架构](ARCHITECTURE.md) | 核心架构设计 | 架构师、开发者 |
| [API参考](API.md) | REST API文档 | 前端开发者、集成开发者 |
| [配置指南](CONFIGURATION.md) | 配置管理和部署 | 运维人员、开发者 |
| [开发指南](DEVELOPMENT.md) | 开发规范和最佳实践 | 开发团队 |

## 🎯 项目概述

基于Rust构建的高性能分布式任务调度系统，支持：

- ✅ 任务创建、调度、执行和监控
- ✅ 分布式Worker节点管理
- ✅ 多种消息队列支持（RabbitMQ、Redis Stream）
- ✅ 故障自动恢复和重试机制
- ✅ 完整的监控和可观测性

## 🚀 快速开始

```bash
# 启动依赖服务
docker-compose up -d

# 运行数据库迁移
cargo run --bin migrate

# 启动服务
cargo run --bin api &
cargo run --bin dispatcher &
cargo run --bin worker

# 验证安装
curl http://localhost:8080/health
```

## 🏗️ 技术栈

- **语言**: Rust 1.70+
- **Web框架**: Axum 0.8
- **数据库**: PostgreSQL 13+ / SQLite
- **消息队列**: RabbitMQ 3.8+ / Redis Stream
- **监控**: Prometheus + OpenTelemetry

## 📞 获取帮助

- 查看 [快速开始指南](GETTING_STARTED.md) 了解基本使用
- 参考 [开发指南](DEVELOPMENT.md) 进行二次开发
- 查看项目根目录的 `README.md` 获取更多信息