use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use scheduler_core::{
    models::Task,
    traits::{TaskRepository, TaskRunRepository},
    SchedulerResult, SchedulerError,
};

/// 任务依赖检查器
pub struct DependencyChecker {
    task_repo: Arc<dyn TaskRepository>,
    task_run_repo: Arc<dyn TaskRunRepository>,
}

/// 依赖检查结果
#[derive(Debug, Clone)]
pub struct DependencyCheckResult {
    pub can_execute: bool,
    pub blocking_dependencies: Vec<i64>,
    pub reason: Option<String>,
}

/// 依赖检查服务接口
#[async_trait]
pub trait DependencyCheckService: Send + Sync {
    /// 检查任务依赖关系
    async fn check_dependencies(&self, task: &Task) -> SchedulerResult<DependencyCheckResult>;

    /// 验证任务依赖关系（检查循环依赖）
    async fn validate_dependencies(&self, task_id: i64, dependencies: &[i64]) -> SchedulerResult<()>;

    /// 检测循环依赖
    async fn detect_circular_dependency(&self, task_id: i64, dependencies: &[i64]) -> SchedulerResult<bool>;

    /// 获取任务的所有传递依赖
    async fn get_transitive_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<i64>>;
}

impl DependencyChecker {
    /// 创建新的依赖检查器
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        task_run_repo: Arc<dyn TaskRunRepository>,
    ) -> Self {
        Self {
            task_repo,
            task_run_repo,
        }
    }

    /// 检查单个依赖任务的状态
    async fn check_single_dependency(&self, dep_task_id: i64) -> SchedulerResult<bool> {
        // 获取依赖任务的最近执行记录
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

    /// 构建任务依赖图
    async fn build_dependency_graph(&self) -> SchedulerResult<HashMap<i64, Vec<i64>>> {
        let all_tasks = self.task_repo.get_active_tasks().await?;
        let mut graph = HashMap::new();

        for task in all_tasks {
            graph.insert(task.id, task.dependencies.clone());
        }

        Ok(graph)
    }

    // 使用深度优先搜索检测循环依赖
    // fn dfs_cycle_detection(
    //     &self,
    //     graph: &HashMap<i64, Vec<i64>>,
    //     node: i64,
    //     visited: &mut HashSet<i64>,
    //     rec_stack: &mut HashSet<i64>,
    // ) -> bool {
    //     visited.insert(node);
    //     rec_stack.insert(node);

    //     if let Some(neighbors) = graph.get(&node) {
    //         for &neighbor in neighbors {
    //             if !visited.contains(&neighbor) {
    //                 if self.dfs_cycle_detection(graph, neighbor, visited, rec_stack) {
    //                     return true;
    //                 }
    //             } else if rec_stack.contains(&neighbor) {
    //                 return true;
    //             }
    //         }
    //     }

    //     rec_stack.remove(&node);
    //     false
    // }

    /// 使用拓扑排序检测循环依赖
    fn topological_sort_cycle_detection(&self, graph: &HashMap<i64, Vec<i64>>) -> bool {
        let mut in_degree: HashMap<i64, usize> = HashMap::new();
        let mut queue = VecDeque::new();

        // 计算每个节点的入度
        for &node in graph.keys() {
            in_degree.entry(node).or_insert(0);
        }

        for dependencies in graph.values() {
            for &dep in dependencies {
                *in_degree.entry(dep).or_insert(0) += 1;
            }
        }

        // 将入度为0的节点加入队列
        for (&node, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node);
            }
        }

        let mut processed_count = 0;

        // 处理队列中的节点
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

        // 如果处理的节点数少于总节点数，说明存在循环
        processed_count < graph.len()
    }

    /// 获取任务的传递依赖（使用广度优先搜索）
    async fn get_transitive_dependencies_bfs(&self, task_id: i64) -> SchedulerResult<Vec<i64>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // 获取直接依赖
        let task = self
            .task_repo
            .get_by_id(task_id)
            .await?
            .ok_or_else(|| SchedulerError::TaskNotFound { id: task_id })?;

        for &dep_id in &task.dependencies {
            queue.push_back(dep_id);
            visited.insert(dep_id);
        }

        // 广度优先搜索获取所有传递依赖
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
    /// 检查任务依赖关系
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

    /// 验证任务依赖关系（检查循环依赖）
    async fn validate_dependencies(&self, task_id: i64, dependencies: &[i64]) -> SchedulerResult<()> {
        // 检查是否存在自依赖
        if dependencies.contains(&task_id) {
            return Err(SchedulerError::CircularDependency);
        }

        // 检查依赖的任务是否存在
        for &dep_id in dependencies {
            let dep_task = self.task_repo.get_by_id(dep_id).await?;
            if dep_task.is_none() {
                return Err(SchedulerError::TaskNotFound { id: dep_id });
            }
        }

        // 检查是否会形成循环依赖
        if self
            .detect_circular_dependency(task_id, dependencies)
            .await?
        {
            return Err(SchedulerError::CircularDependency);
        }

        Ok(())
    }

    /// 检测循环依赖
    async fn detect_circular_dependency(
        &self,
        task_id: i64,
        new_dependencies: &[i64],
    ) -> SchedulerResult<bool> {
        // 构建当前的依赖图
        let mut graph = self.build_dependency_graph().await?;

        // 添加新的依赖关系
        graph.insert(task_id, new_dependencies.to_vec());

        // 使用拓扑排序检测循环
        let has_cycle = self.topological_sort_cycle_detection(&graph);

        if has_cycle {
            warn!(
                "检测到循环依赖，任务ID: {}, 新依赖: {:?}",
                task_id, new_dependencies
            );
        }

        Ok(has_cycle)
    }

    /// 获取任务的所有传递依赖
    async fn get_transitive_dependencies(&self, task_id: i64) -> SchedulerResult<Vec<i64>> {
        self.get_transitive_dependencies_bfs(task_id).await
    }
}
