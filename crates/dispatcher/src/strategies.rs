use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use scheduler_core::{
    models::{Task, WorkerInfo},
    traits::TaskDispatchStrategy,
    Result,
};

/// 轮询策略 - 按顺序轮流分配任务给Worker
pub struct RoundRobinStrategy {
    counter: AtomicUsize,
}

/// 负载均衡策略 - 选择负载最低的Worker
pub struct LoadBasedStrategy;

/// 任务类型亲和策略 - 优先选择支持特定任务类型的Worker
pub struct TaskTypeAffinityStrategy;

impl RoundRobinStrategy {
    /// 创建新的轮询策略
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
    ) -> Result<Option<String>> {
        if available_workers.is_empty() {
            debug!("没有可用的Worker节点");
            return Ok(None);
        }

        // 过滤出能够执行该任务类型的Worker
        let suitable_workers: Vec<&WorkerInfo> = available_workers
            .iter()
            .filter(|worker| worker.can_accept_task(&task.task_type))
            .collect();

        if suitable_workers.is_empty() {
            debug!("没有支持任务类型 {} 的可用Worker", task.task_type);
            return Ok(None);
        }

        // 使用轮询算法选择Worker
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
    /// 创建新的负载均衡策略
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
    ) -> Result<Option<String>> {
        if available_workers.is_empty() {
            debug!("没有可用的Worker节点");
            return Ok(None);
        }

        // 过滤出能够执行该任务类型的Worker
        let suitable_workers: Vec<&WorkerInfo> = available_workers
            .iter()
            .filter(|worker| worker.can_accept_task(&task.task_type))
            .collect();

        if suitable_workers.is_empty() {
            debug!("没有支持任务类型 {} 的可用Worker", task.task_type);
            return Ok(None);
        }

        // 选择负载最低的Worker
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
    /// 创建新的任务类型亲和策略
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
    ) -> Result<Option<String>> {
        if available_workers.is_empty() {
            debug!("没有可用的Worker节点");
            return Ok(None);
        }

        // 过滤出能够执行该任务类型的Worker
        let suitable_workers: Vec<&WorkerInfo> = available_workers
            .iter()
            .filter(|worker| worker.can_accept_task(&task.task_type))
            .collect();

        if suitable_workers.is_empty() {
            debug!("没有支持任务类型 {} 的可用Worker", task.task_type);
            return Ok(None);
        }

        // 优先选择专门支持该任务类型的Worker（只支持一种任务类型）
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

        // 在目标Worker中选择负载最低的
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

/// 组合策略 - 可以组合多个策略
pub struct CompositeStrategy {
    strategies: Vec<Arc<dyn TaskDispatchStrategy>>,
}

impl CompositeStrategy {
    /// 创建新的组合策略
    pub fn new(strategies: Vec<Arc<dyn TaskDispatchStrategy>>) -> Self {
        Self { strategies }
    }

    /// 添加策略
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
    ) -> Result<Option<String>> {
        // 依次尝试每个策略，直到找到合适的Worker
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
