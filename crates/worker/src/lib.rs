pub mod executors;
pub mod heartbeat;
pub mod service;

// New refactored components (SOLID principles)
pub mod components;
pub mod refactored_service;

#[cfg(test)]
mod service_test;

#[cfg(test)]
mod executors_test;

pub use executors::{HttpExecutor, HttpTaskParams, ShellExecutor, ShellTaskParams};
pub use service::{WorkerService, WorkerServiceBuilder};

// Re-export refactored service for new code
pub use refactored_service::{
    WorkerService as RefactoredWorkerService,
    WorkerServiceBuilder as RefactoredWorkerServiceBuilder,
};
