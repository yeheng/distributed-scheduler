//! Worker service components - Refactored for better separation of concerns
//!
//! This module contains focused components that each handle a single responsibility:
//! - TaskExecutionManager: Handles task execution logic
//! - DispatcherClient: Manages communication with dispatcher
//! - HeartbeatManager: Handles heartbeat and status reporting
//! - WorkerLifecycle: Manages service start/stop lifecycle

pub mod dispatcher_client;
pub mod heartbeat_manager;
pub mod task_execution;
pub mod worker_lifecycle;

pub use dispatcher_client::DispatcherClient;
pub use heartbeat_manager::HeartbeatManager;
pub use task_execution::TaskExecutionManager;
pub use worker_lifecycle::WorkerLifecycle;
