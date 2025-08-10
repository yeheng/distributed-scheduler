use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 1,
    Normal = 5,
    High = 10,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub timeout_seconds: Option<u32>,
    pub max_retries: u32,
    pub retry_delay_seconds: u32,
    pub priority: TaskPriority,
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: Some(300), // 5 minutes
            max_retries: 3,
            retry_delay_seconds: 60,
            priority: TaskPriority::Normal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCapacity {
    pub max_concurrent_tasks: u32,
    pub current_load: u32,
}

impl WorkerCapacity {
    pub fn new(max_concurrent_tasks: u32) -> Self {
        Self {
            max_concurrent_tasks,
            current_load: 0,
        }
    }

    pub fn is_available(&self) -> bool {
        self.current_load < self.max_concurrent_tasks
    }

    pub fn available_slots(&self) -> u32 {
        self.max_concurrent_tasks.saturating_sub(self.current_load)
    }

    pub fn utilization_rate(&self) -> f64 {
        if self.max_concurrent_tasks == 0 {
            0.0
        } else {
            self.current_load as f64 / self.max_concurrent_tasks as f64
        }
    }
}
