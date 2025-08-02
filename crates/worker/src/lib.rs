pub mod components;
pub mod composed_service;
pub mod executors;
pub mod heartbeat;
// Legacy modules for backward compatibility
pub mod refactored_service;
pub mod service;

#[cfg(test)]
mod service_test;

#[cfg(test)]
mod executors_test;

// Primary exports - use the composed service as the main implementation
pub use components::*;
pub use composed_service::{WorkerService, WorkerServiceBuilder, WorkerServiceTrait};
pub use executors::{HttpExecutor, HttpTaskParams, ShellExecutor, ShellTaskParams};

// Legacy exports - marked for deprecation
#[deprecated(note = "Use WorkerService from composed_service instead")]
pub use refactored_service::{
    WorkerService as RefactoredWorkerService,
    WorkerServiceBuilder as RefactoredWorkerServiceBuilder,
};

#[deprecated(note = "Use WorkerService from composed_service instead")]
pub use service::{
    WorkerService as LegacyWorkerService, WorkerServiceBuilder as LegacyWorkerServiceBuilder,
};
