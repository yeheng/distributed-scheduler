use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use scheduler_core::{
    models::{Task, WorkerInfo},
    traits::TaskDispatchStrategy,
    SchedulerResult,
};

pub struct RoundRobinStrategy {
    counter: AtomicUsize,
}

pub struct LoadBasedStrategy;

pub struct TaskTypeAffinityStrategy;

impl RoundRobinStrategy {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobinStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskDispatchStrategy for RoundRobinStrategy {
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>> {
        if available_workers.is_empty() {
            debug!("没有可用的Worker节点");
            return Ok(None);
        }
        let suitable_workers: Vec<&WorkerInfo> = available_workers
            .iter()
            .filter(|worker| worker.can_accept_task(&task.task_type))
            .collect();

        if suitable_workers.is_empty() {
            debug!("没有支持任务类型 {} 的可用Worker", task.task_type);
            return Ok(None);
        }
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % suitable_workers.len();
        let selected_worker = suitable_workers[index];

        debug!(
            "轮询策略选择Worker: {} (索引: {}/{})",
            selected_worker.id,
            index,
            suitable_workers.len()
        );

        Ok(Some(selected_worker.id.clone()))
    }

    fn name(&self) -> &str {
        "RoundRobin"
    }
}

impl LoadBasedStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LoadBasedStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskDispatchStrategy for LoadBasedStrategy {
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>> {
        if available_workers.is_empty() {
            debug!("没有可用的Worker节点");
            return Ok(None);
        }
        let suitable_workers: Vec<&WorkerInfo> = available_workers
            .iter()
            .filter(|worker| worker.can_accept_task(&task.task_type))
            .collect();

        if suitable_workers.is_empty() {
            debug!("没有支持任务类型 {} 的可用Worker", task.task_type);
            return Ok(None);
        }
        let selected_worker = suitable_workers
            .iter()
            .min_by(|a, b| {
                a.load_percentage()
                    .partial_cmp(&b.load_percentage())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        debug!(
            "负载均衡策略选择Worker: {} (负载: {:.1}%)",
            selected_worker.id,
            selected_worker.load_percentage()
        );

        Ok(Some(selected_worker.id.clone()))
    }

    fn name(&self) -> &str {
        "LoadBased"
    }
}

impl TaskTypeAffinityStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskTypeAffinityStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskDispatchStrategy for TaskTypeAffinityStrategy {
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>> {
        if available_workers.is_empty() {
            debug!("没有可用的Worker节点");
            return Ok(None);
        }
        let suitable_workers: Vec<&WorkerInfo> = available_workers
            .iter()
            .filter(|worker| worker.can_accept_task(&task.task_type))
            .collect();

        if suitable_workers.is_empty() {
            debug!("没有支持任务类型 {} 的可用Worker", task.task_type);
            return Ok(None);
        }
        let specialized_workers: Vec<&WorkerInfo> = suitable_workers
            .iter()
            .filter(|worker| {
                worker.supported_task_types.len() == 1
                    && worker.supported_task_types[0] == task.task_type
            })
            .copied()
            .collect();

        let target_workers = if !specialized_workers.is_empty() {
            debug!("找到专门支持任务类型 {} 的Worker", task.task_type);
            specialized_workers
        } else {
            suitable_workers
        };
        let selected_worker = target_workers
            .iter()
            .min_by(|a, b| {
                a.load_percentage()
                    .partial_cmp(&b.load_percentage())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        debug!(
            "任务类型亲和策略选择Worker: {} (负载: {:.1}%, 支持类型: {:?})",
            selected_worker.id,
            selected_worker.load_percentage(),
            selected_worker.supported_task_types
        );

        Ok(Some(selected_worker.id.clone()))
    }

    fn name(&self) -> &str {
        "TaskTypeAffinity"
    }
}

pub struct CompositeStrategy {
    strategies: Vec<Arc<dyn TaskDispatchStrategy>>,
}

impl CompositeStrategy {
    pub fn new(strategies: Vec<Arc<dyn TaskDispatchStrategy>>) -> Self {
        Self { strategies }
    }
    pub fn add_strategy(&mut self, strategy: Arc<dyn TaskDispatchStrategy>) {
        self.strategies.push(strategy);
    }
}

#[async_trait]
impl TaskDispatchStrategy for CompositeStrategy {
    async fn select_worker(
        &self,
        task: &Task,
        available_workers: &[WorkerInfo],
    ) -> SchedulerResult<Option<String>> {
        for strategy in &self.strategies {
            match strategy.select_worker(task, available_workers).await? {
                Some(worker_id) => {
                    debug!(
                        "组合策略使用 {} 策略选择了Worker: {}",
                        strategy.name(),
                        worker_id
                    );
                    return Ok(Some(worker_id));
                }
                None => {
                    debug!(
                        "策略 {} 未找到合适的Worker，尝试下一个策略",
                        strategy.name()
                    );
                    continue;
                }
            }
        }

        warn!("所有策略都未能找到合适的Worker");
        Ok(None)
    }

    fn name(&self) -> &str {
        "Composite"
    }
}
