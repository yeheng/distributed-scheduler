# 分布式任务调度系统实施计划

- [x] 1. 项目结构和核心接口设置
  - 创建Cargo项目结构，包含core、dispatcher、worker、api等模块
  - 定义核心数据结构（Task、TaskRun、Worker、Message等）
  - 实现基础错误类型和Result类型定义
  - _需求: 1.1, 1.2, 1.4_

- [x] 2. 配置管理
  - 实现配置文件解析（TOML格式）
  - 实现环境变量覆盖机制
  - 实现配置验证逻辑
  - _需求: 所有需求的配置支持_

- [x] 3. 数据库层实现
  - [x] 3.1 数据库迁移和表结构
    - 创建PostgreSQL迁移文件，定义所有表结构
    - 实现数据库连接池配置
    - 添加必要的索引优化
    - _需求: 5.1, 5.2, 5.3_

  - [x] 3.2 Repository抽象接口
    - 定义TaskRepository和TaskRunRepository trait
    - 定义WorkerRepository trait
    - 实现基础的CRUD操作接口
    - _需求: 1.1, 1.2, 1.3, 1.4_

  - [x] 3.3 PostgreSQL Repository实现
    - 实现PostgresTaskRepository的所有方法
    - 实现PostgresTaskRunRepository的所有方法
    - 实现PostgresWorkerRepository的所有方法
    - 添加单元测试验证数据库操作
    - _需求: 1.1, 1.2, 1.3, 1.4_

- [-] 4. 消息队列抽象层
  - [x] 4.1 MessageQueue trait定义
    - 定义统一的Message结构体和MessageType枚举
    - 定义消息发布和消费接口
    - 实现消息序列化和反序列化
    - _需求: 1.1, 1.2, 1.6_

  - [x] 3.2 RabbitMQ MessageQueue实现
    - 实现基于RabbitMQ的消息队列
    - 实现任务分发和状态更新队列
    - 实现消息持久化和重试机制
    - 添加MessageQueue的集成测试
    - _需求: 1.1, 1.2, 1.6_

- [x] 5. 任务调度核心逻辑
  - [x] 5.1 CRON调度器实现
    - 集成cron库解析调度表达式
    - 实现任务到期检测逻辑
    - 实现任务实例创建机制
    - _需求: 1.1_

  - [x] 5.2 任务依赖检查
    - 实现任务依赖关系验证
    - 实现依赖任务完成状态检查
    - 实现循环依赖检测
    - 添加依赖逻辑的单元测试
    - _需求: 9.2, 9.3, 9.4_

  - [x] 5.3 任务分派策略
    - 实现RoundRobinStrategy轮询策略
    - 实现LoadBasedStrategy负载均衡策略
    - 实现TaskTypeAffinityStrategy任务类型亲和策略
    - 添加分派策略的单元测试
    - _需求: 10.2_

- [x] 6. Dispatcher服务实现
  - [x] 6.1 TaskScheduler服务
    - 实现TaskSchedulerService trait和具体实现
    - 实现定时任务扫描循环
    - 实现任务创建和分发到消息队列
    - _需求: 1.1, 1.2_

  - [x] 6.2 StateListener服务
    - 实现StateListenerService trait和具体实现
    - 实现状态更新消息监听
    - 实现任务状态数据库更新
    - _需求: 1.4, 1.6_

  - [x] 6.3 TaskController服务
    - 实现TaskControlService trait和具体实现
    - 实现任务触发、暂停、恢复、重启、中止功能
    - 实现任务控制的API接口
    - _需求: 3.6_

- [-] 7. Worker服务实现
  - [x] 7.1 WorkerService核心结构
    - 实现WorkerServiceTrait和具体实现
    - 实现Worker服务启动和停止逻辑
    - 实现从消息队列轮询任务
    - _需求: 1.3, 2.1, 2.2_

  - [x] 7.2 任务执行器框架
    - 定义TaskExecutor trait
    - 实现Shell任务执行器
    - 实现HTTP任务执行器
    - _需求: 1.4, 1.5_

  - [x] 7.3 任务执行和状态上报
    - 实现任务并发执行管理
    - 实现任务状态更新到消息队列
    - 实现任务超时处理
    - 添加任务执行的集成测试
    - _需求: 1.4, 1.5, 1.7_

- [x] 8. 失败重试机制
  - [x] 8.1 重试逻辑实现
    - 实现基于max_retries的重试机制
    - 实现指数退避重试策略
    - 实现Worker失效时的任务重新分配
    - _需求: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [x] 8.2 故障恢复机制
    - 实现系统重启后的任务状态恢复
    - 实现数据库连接失败的重连机制
    - 实现消息队列连接失败的重连机制
    - _需求: 5.3, 5.4_

- [-] 9. REST API接口
  - [x] 9.1 基础API框架
    - 使用axum创建HTTP服务器
    - 实现基础的路由和中间件
    - 实现错误处理和响应格式化
    - _需求: 3.7_

  - [x] 9.2 任务管理API
    - 实现任务CRUD操作的REST端点
    - 实现任务列表查询和过滤
    - 实现任务执行历史查询
    - 实现任务触发API端点
    - 添加API端点的集成测试
    - _需求: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

  - [x] 9.3 系统监控API
    - 实现Worker状态查询API
    - 实现任务执行统计API
    - 实现系统健康检查端点
    - _需求: 2.1, 2.2, 2.3, 2.4, 2.5_

- [x] 10. OpenTelemetry可观测性
  - [x] 10.1 Metrics指标收集
    - 集成OpenTelemetry metrics
    - 实现任务执行指标收集
    - 实现Worker负载指标收集
    - 实现系统性能指标收集
    - _需求: 7.1, 7.2, 7.3_

  - [x] 10.2 分布式追踪
    - 集成OpenTelemetry tracing
    - 实现任务生命周期追踪
    - 实现跨组件调用追踪
    - _需求: 7.1_

  - [x] 10.3 结构化日志
    - 集成tracing库实现结构化日志
    - 实现与OpenTelemetry的日志关联
    - 实现不同级别的日志输出
    - _需求: 6.1, 6.2, 6.3, 6.4_

- [x] 11. 启动逻辑
  - 实现main函数和CLI参数解析
  - 实现Dispatcher/Worker模式选择
  - 实现优雅关闭机制
  - _需求: 2.5_

- [x] 12. 集成测试和端到端测试
  - [x] 12.1 数据库集成测试
    - 创建测试数据库设置
    - 实现Repository层的集成测试
    - 实现数据库迁移的测试
    - _需求: 所有数据库相关需求_

  - [x] 12.2 Dispatcher-Worker集成测试
    - 实现Dispatcher和Worker协作的集成测试
    - 测试任务分发和执行的完整流程
    - 测试故障转移和恢复机制
    - _需求: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2_

  - [x] 12.3 端到端功能测试
    - 实现完整任务生命周期的测试
    - 测试任务依赖和重试功能
    - 测试API接口的端到端流程
    - _需求: 所有功能需求_

- [ ] 13. 部署配置
  - [-] 13.1 Docker化
    - 创建Dockerfile进行应用打包
    - 创建docker-compose.yml用于本地开发
    - 实现多阶段构建优化镜像大小
    - _需求: 部署相关_

  - [ ] 13.2 生产部署准备
    - 创建Kubernetes部署配置
    - 实现健康检查端点
    - 创建部署文档和运维指南
    - _需求: 生产环境部署_
