use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use scheduler_core::{
    models::Task,
    traits::{TaskRepository, TaskRunRepository},
    SchedulerError, SchedulerResult,
};

pub struct DependencyChecker {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
}

#[derive(Debug, Clone)]
pub struct DependencyCheckResult {
    pub can_execute: bool,
    pub blocking_dependencies: Vec<i64>,
    pub reason: Option<String>,
}

#[async_trait]
pub trait DependencyCheckService: Send + Sync {
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<DependencyCheckResult>;
    async fn validate_dependencies(
        &self,
        task_id: i64,
        dependencies: &[i64],
    ) -> SchedulerResult<()>;
    async fn detect_circular_dependency(
        &self,
        task_id: i64,
        dependencies: &[i64],
    ) -> SchedulerResult<bool>;
    async fn get_transitive_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<i64>>;
}

impl DependencyChecker {
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
    ) -> Self {
        Self {
            task_repo,
            task_run_repo,
        }
    }
    async fn check_single_dependency(&self, dep_task_id: i64) -> SchedulerResult<bool> {
        let recent_runs = self.task_run_repo.get_recent_runs(dep_task_id, 1).await?;

        match recent_runs.first() {
            Some(run) if run.is_successful() => {
                debug!("依赖任务 {} 最近执行成功", dep_task_id);
                Ok(true)
            }
            Some(run) => {
                debug!(
                    "依赖任务 {} 最近执行状态为 {:?}，不满足依赖条件",
                    dep_task_id, run.status
                );
                Ok(false)
            }
            None => {
                debug!("依赖任务 {} 从未执行过，不满足依赖条件", dep_task_id);
                Ok(false)
            }
        }
    }
    async fn build_dependency_graph(&self) -> SchedulerResult<HashMap<i64, Vec<i64>>> {
        let all_tasks = self.task_repo.get_active_tasks().await?;
        let mut graph = HashMap::new();

        for task in all_tasks {
            graph.insert(task.id, task.dependencies.clone());
        }

        Ok(graph)
    }
    fn topological_sort_cycle_detection(&self, graph: &HashMap<i64, Vec<i64>>) -> bool {
        let mut in_degree: HashMap<i64, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        for &node in graph.keys() {
            in_degree.entry(node).or_insert(0);
        }

        for dependencies in graph.values() {
            for &dep in dependencies {
                *in_degree.entry(dep).or_insert(0) += 1;
            }
        }
        for (&node, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node);
            }
        }

        let mut processed_count = 0;
        while let Some(node) = queue.pop_front() {
            processed_count += 1;

            if let Some(dependencies) = graph.get(&node) {
                for &dep in dependencies {
                    if let Some(degree) = in_degree.get_mut(&dep) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }
        processed_count < graph.len()
    }
    async fn get_transitive_dependencies_bfs(&self, task_id: i64) -> SchedulerResult<Vec<i64>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();
        let task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;

        for &dep_id in &task.dependencies {
            queue.push_back(dep_id);
            visited.insert(dep_id);
        }
        while let Some(current_id) = queue.pop_front() {
            result.push(current_id);

            let current_task = self.task_repo.get_by_id(current_id).await?;
            if let Some(current_task) = current_task {
                for &dep_id in &current_task.dependencies {
                    if !visited.contains(&dep_id) {
                        queue.push_back(dep_id);
                        visited.insert(dep_id);
                    }
                }
            }
        }

        Ok(result)
    }
}

#[async_trait]
impl DependencyCheckService for DependencyChecker {
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<DependencyCheckResult> {
        if task.dependencies.is_empty() {
            return Ok(DependencyCheckResult {
                can_execute: true,
                blocking_dependencies: Vec::new(),
                reason: None,
            });
        }

        debug!("检查任务 {} 的依赖关系", task.name);

        let mut blocking_dependencies = Vec::new();

        for &dep_task_id in &task.dependencies {
            if !self.check_single_dependency(dep_task_id).await? {
                blocking_dependencies.push(dep_task_id);
            }
        }

        let can_execute = blocking_dependencies.is_empty();
        let reason = if !can_execute {
            Some(format!(
                "以下依赖任务未满足执行条件: {blocking_dependencies:?}"
            ))
        } else {
            None
        };

        Ok(DependencyCheckResult {
            can_execute,
            blocking_dependencies,
            reason,
        })
    }
    async fn validate_dependencies(
        &self,
        task_id: i64,
        dependencies: &[i64],
    ) -> SchedulerResult<()> {
        if dependencies.contains(&task_id) {
            return Err(SchedulerError::CircularDependency);
        }
        for &dep_id in dependencies {
            let dep_task = self.task_repo.get_by_id(dep_id).await?;
            if dep_task.is_none() {
                return Err(SchedulerError::TaskNotFound { id: dep_id });
            }
        }
        if self
            .detect_circular_dependency(task_id, dependencies)
            .await?
        {
            return Err(SchedulerError::CircularDependency);
        }

        Ok(())
    }
    async fn detect_circular_dependency(
        &self,
        task_id: i64,
        new_dependencies: &[i64],
    ) -> SchedulerResult<bool> {
        let mut graph = self.build_dependency_graph().await?;
        graph.insert(task_id, new_dependencies.to_vec());
        let has_cycle = self.topological_sort_cycle_detection(&graph);

        if has_cycle {
            warn!(
                "检测到循环依赖，任务ID: {}, 新依赖: {:?}",
                task_id, new_dependencies
            );
        }

        Ok(has_cycle)
    }
    async fn get_transitive_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<i64>> {
        self.get_transitive_dependencies_bfs(task_id).await
    }
}
