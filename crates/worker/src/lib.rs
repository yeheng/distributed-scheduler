pub mod components;
pub mod composed_service;
pub mod executors;
pub mod heartbeat;
pub mod refactored_service;
pub mod service;

#[cfg(test)]
mod service_test;

#[cfg(test)]
mod executors_test;

pub use components::*;
pub use composed_service::{WorkerService, WorkerServiceBuilder, WorkerServiceTrait};
pub use executors::{HttpExecutor, HttpTaskParams, ShellExecutor, ShellTaskParams};

// Legacy exports - consider removing in future versions
pub use refactored_service::{
    WorkerService as RefactoredWorkerService,
    WorkerServiceBuilder as RefactoredWorkerServiceBuilder,
};
pub use service::{
    WorkerService as LegacyWorkerService, WorkerServiceBuilder as LegacyWorkerServiceBuilder,
};
