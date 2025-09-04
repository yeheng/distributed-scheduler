# 项目文档索引

## 📖 文档概览

分布式任务调度系统的完整文档集合，涵盖系统架构、API 接口、部署指南、配置管理等所有方面。

## 🗂️ 文档结构

### 核心文档
- **[README.md](../README.md)** - 项目概览、快速开始、特性介绍
- **[CLAUDE.md](../CLAUDE.md)** - Claude Code 开发指南和项目规范

### 技术文档
- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - 系统架构设计、模块依赖、设计模式
- **[API_DOCUMENTATION.md](./API_DOCUMENTATION.md)** - REST API 完整接口文档
- **[DEPLOYMENT.md](./DEPLOYMENT.md)** - 部署指南、配置管理、运维手册

### 配置文档
- **[config/README.md](../config/README.md)** - 配置文件说明、环境变量、最佳实践
- **[docker/README.md](../docker/README.md)** - Docker 部署指南、容器化配置

### 数据库文档
- **[crates/infrastructure/src/database/README.md](../crates/infrastructure/src/database/README.md)** - 数据库架构、迁移管理

## 📊 快速导航

### 🎯 我想了解...

#### 项目概况
- **项目介绍**: [README.md](../README.md#项目概览)
- **核心特性**: [README.md](../README.md#核心特性)
- **技术栈**: [ARCHITECTURE.md](./ARCHITECTURE.md#技术栈)

#### 系统架构
- **整体架构**: [ARCHITECTURE.md](./ARCHITECTURE.md#系统架构)
- **模块设计**: [ARCHITECTURE.md](./ARCHITECTURE.md#模块架构)
- **设计模式**: [ARCHITECTURE.md](./ARCHITECTURE.md#核心设计模式)

#### API 使用
- **认证方式**: [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#认证)
- **任务管理**: [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#任务管理-api)
- **工作节点**: [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#工作节点管理-api)
- **错误处理**: [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#错误处理)

#### 部署运维
- **快速部署**: [DEPLOYMENT.md](./DEPLOYMENT.md#快速开始)
- **生产部署**: [DEPLOYMENT.md](./DEPLOYMENT.md#生产环境部署)
- **配置管理**: [DEPLOYMENT.md](./DEPLOYMENT.md#配置管理)
- **监控设置**: [DEPLOYMENT.md](./DEPLOYMENT.md#监控设置)

#### 开发指南
- **环境搭建**: [CLAUDE.md](../CLAUDE.md#开发命令)
- **代码规范**: [CLAUDE.md](../CLAUDE.md#代码标准)
- **测试运行**: [CLAUDE.md](../CLAUDE.md#代码质量)

## 🛠️ 开发者指南

### 新手入门路径
1. **了解项目** → [README.md](../README.md)
2. **架构学习** → [ARCHITECTURE.md](./ARCHITECTURE.md)
3. **环境搭建** → [DEPLOYMENT.md](./DEPLOYMENT.md#开发环境部署)
4. **API 测试** → [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#api-客户端示例)
5. **配置调整** → [config/README.md](../config/README.md)

### 运维人员路径
1. **部署架构** → [ARCHITECTURE.md](./ARCHITECTURE.md#部署架构)
2. **部署指南** → [DEPLOYMENT.md](./DEPLOYMENT.md)
3. **配置管理** → [config/README.md](../config/README.md)
4. **监控设置** → [DEPLOYMENT.md](./DEPLOYMENT.md#监控设置)
5. **故障排除** → [DEPLOYMENT.md](./DEPLOYMENT.md#故障排除)

### API 开发者路径
1. **API 概览** → [API_DOCUMENTATION.md](./API_DOCUMENTATION.md)
2. **认证方式** → [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#认证)
3. **接口详情** → [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#任务管理-api)
4. **客户端示例** → [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#api-客户端示例)
5. **错误处理** → [API_DOCUMENTATION.md](./API_DOCUMENTATION.md#错误处理)

## 📋 文档状态

### 完整性检查表
- [x] 项目概览和快速开始
- [x] 系统架构和设计文档
- [x] API 接口完整文档
- [x] 部署指南和配置说明
- [x] 错误处理和故障排除
- [x] 性能优化和监控指南
- [x] 安全配置和最佳实践
- [x] 开发流程和代码规范

### 文档维护
- **最后更新**: 2024-09-04
- **版本**: v1.0.0
- **维护者**: Scheduler Team
- **审核状态**: ✅ 已审核

## 🔄 文档更新流程

### 更新原则
1. **同步更新**: 代码变更时同步更新相关文档
2. **版本管理**: 文档版本与代码版本保持一致
3. **审核机制**: 文档变更需要代码审核
4. **用户友好**: 保持文档的易读性和实用性

### 贡献指南
1. **识别需求**: 发现文档缺失或过时内容
2. **创建分支**: 从 main 分支创建文档更新分支
3. **编写文档**: 遵循现有文档格式和风格
4. **本地验证**: 确保文档链接和格式正确
5. **提交审核**: 创建 Pull Request 并请求审核

## 🔍 搜索指南

### 按主题搜索
- **架构设计**: `ARCHITECTURE.md`
- **API 接口**: `API_DOCUMENTATION.md` + 搜索具体端点
- **配置选项**: `config/README.md` + `DEPLOYMENT.md`
- **错误排除**: `DEPLOYMENT.md#故障排除`
- **性能优化**: `ARCHITECTURE.md#性能优化`

### 按角色搜索
- **开发者**: `CLAUDE.md`, `ARCHITECTURE.md`
- **运维工程师**: `DEPLOYMENT.md`, `config/README.md`
- **API 用户**: `API_DOCUMENTATION.md`
- **系统管理员**: `DEPLOYMENT.md#监控设置`, `ARCHITECTURE.md#安全架构`

## 📝 文档反馈

如果您发现文档中有任何问题、缺失信息或改进建议，欢迎：

1. **提交 Issue**: [GitHub Issues](https://github.com/your-org/scheduler/issues)
2. **创建 PR**: 直接提交文档改进
3. **发起讨论**: [GitHub Discussions](https://github.com/your-org/scheduler/discussions)

---

**💡 提示**: 建议按照上述导航路径阅读文档，以便更好地理解系统设计和使用方法。