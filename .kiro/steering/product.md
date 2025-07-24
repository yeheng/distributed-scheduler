# Product Overview

## Distributed Task Scheduler System (分布式任务调度系统)

A high-performance, scalable distributed task scheduling system built with Rust. The system enables users to define scheduled tasks that are automatically dispatched by schedulers and reliably executed by worker nodes.

### Key Features

- **Dispatcher-Worker Architecture**: Centralized scheduling with distributed execution
- **CRON-based Scheduling**: Flexible time-based task scheduling using cron expressions
- **High Availability**: Multi-dispatcher support with automatic failover
- **Horizontal Scaling**: Dynamic worker node addition/removal
- **Reliable Execution**: Automatic retry mechanisms with exponential backoff
- **Task Dependencies**: Support for task dependency chains
- **Real-time Monitoring**: Task execution tracking and system health monitoring
- **RESTful API**: Complete task management through HTTP APIs

### Core Components

- **Dispatcher**: Task scheduling, dependency checking, and state management
- **Worker**: Stateless task execution nodes with pluggable executors
- **Message Queue**: RabbitMQ-based task distribution and status updates
- **Database**: PostgreSQL for persistent task definitions and execution history
- **API Server**: REST endpoints for task management and system monitoring

### Target Use Cases

- Batch job processing
- Data pipeline orchestration
- Scheduled maintenance tasks
- ETL workflows
- Automated reporting systems