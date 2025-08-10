use crate::entities::{Task, TaskRunStatus};
use scheduler_errors::{SchedulerError, SchedulerResult};
use tracing::debug;

/// Domain service for task dependency checking business logic
/// This contains the core algorithms for dependency validation
pub struct TaskDependencyService;

impl TaskDependencyService {
    /// Check if all dependencies of a task are satisfied
    pub fn check_dependencies_satisfied(
        task_dependencies: &[i64], 
        dependency_statuses: &[(i64, Option<TaskRunStatus>)]
    ) -> SchedulerResult<bool> {
        if task_dependencies.is_empty() {
            debug!("Task has no dependencies");
            return Ok(true);
        }

        debug!("Checking {} dependencies", task_dependencies.len());
        
        for &dep_id in task_dependencies {
            let dependency_satisfied = dependency_statuses
                .iter()
                .find(|(id, _)| *id == dep_id)
                .map(|(_, status)| status.as_ref() == Some(&TaskRunStatus::Completed))
                .unwrap_or(false);

            if !dependency_satisfied {
                debug!("Dependency {} is not satisfied", dep_id);
                return Ok(false);
            }
        }

        debug!("All dependencies are satisfied");
        Ok(true)
    }

    /// Filter tasks to get their dependency IDs
    pub fn extract_dependency_ids(tasks: &[Task]) -> Vec<i64> {
        tasks.iter()
            .flat_map(|task| &task.dependencies)
            .copied()
            .collect()
    }

    /// Validate that all dependencies exist and are in valid states
    pub fn validate_task_dependencies(
        task: &Task, 
        dependency_tasks: &[Task]
    ) -> SchedulerResult<()> {
        for &dep_id in &task.dependencies {
            let dependency_exists = dependency_tasks.iter().any(|dep| dep.id == dep_id);
            if !dependency_exists {
                return Err(SchedulerError::InvalidDependency { 
                    task_id: task.id, 
                    dependency_id: dep_id,
                    reason: "Dependency task does not exist".to_string()
                });
            }
        }

        // Check for circular dependencies (simplified check)
        if task.dependencies.contains(&task.id) {
            return Err(SchedulerError::InvalidDependency {
                task_id: task.id,
                dependency_id: task.id, 
                reason: "Circular dependency: task cannot depend on itself".to_string()
            });
        }

        Ok(())
    }

    /// Check if a dependency relationship would create a cycle
    pub fn would_create_cycle(
        task_id: i64,
        new_dependency_id: i64,
        all_dependencies: &[(i64, Vec<i64>)]
    ) -> bool {
        if task_id == new_dependency_id {
            return true; // Direct self-reference
        }

        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![new_dependency_id];

        while let Some(current) = stack.pop() {
            if current == task_id {
                return true; // Found a cycle
            }

            if visited.contains(&current) {
                continue;
            }

            visited.insert(current);

            if let Some((_, deps)) = all_dependencies.iter().find(|(id, _)| *id == current) {
                stack.extend(deps);
            }
        }

        false
    }

    /// Build dependency graph for topological sorting
    pub fn build_dependency_graph(tasks: &[Task]) -> Vec<(i64, Vec<i64>)> {
        tasks.iter()
            .map(|task| (task.id, task.dependencies.clone()))
            .collect()
    }

    /// Perform topological sort on tasks based on dependencies
    pub fn topological_sort(tasks: &[Task]) -> SchedulerResult<Vec<i64>> {
        let graph = Self::build_dependency_graph(tasks);
        let mut in_degree: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
        let mut adj_list: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();

        // Initialize in-degree count and adjacency list
        for (task_id, dependencies) in &graph {
            in_degree.entry(*task_id).or_insert(0);
            for &dep_id in dependencies {
                adj_list.entry(dep_id).or_insert_with(Vec::new).push(*task_id);
                *in_degree.entry(*task_id).or_insert(0) += 1;
            }
        }

        // Kahn's algorithm
        let mut queue: std::collections::VecDeque<i64> = std::collections::VecDeque::new();
        let mut result = Vec::new();

        // Find all nodes with no incoming edges
        for (&task_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(task_id);
            }
        }

        while let Some(current) = queue.pop_front() {
            result.push(current);

            if let Some(dependents) = adj_list.get(&current) {
                for &dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(&dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent);
                        }
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != tasks.len() {
            return Err(SchedulerError::InvalidDependency {
                task_id: 0,
                dependency_id: 0,
                reason: "Circular dependency detected in task graph".to_string()
            });
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{TaskStatus};
    use chrono::Utc;

    fn create_test_task(id: i64, dependencies: Vec<i64>) -> Task {
        Task {
            id,
            name: format!("test_task_{}", id),
            task_type: "test".to_string(),
            schedule: "0 0 * * *".to_string(),
            parameters: serde_json::json!({}),
            timeout_seconds: 60,
            max_retries: 3,
            status: TaskStatus::Active,
            dependencies,
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_check_dependencies_satisfied_no_dependencies() {
        let dependencies = vec![];
        let statuses = vec![];
        
        let result = TaskDependencyService::check_dependencies_satisfied(&dependencies, &statuses);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test] 
    fn test_check_dependencies_satisfied_all_completed() {
        let dependencies = vec![1, 2, 3];
        let statuses = vec![
            (1, Some(TaskRunStatus::Completed)),
            (2, Some(TaskRunStatus::Completed)), 
            (3, Some(TaskRunStatus::Completed)),
        ];

        let result = TaskDependencyService::check_dependencies_satisfied(&dependencies, &statuses);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_check_dependencies_satisfied_some_not_completed() {
        let dependencies = vec![1, 2, 3];
        let statuses = vec![
            (1, Some(TaskRunStatus::Completed)),
            (2, Some(TaskRunStatus::Failed)),
            (3, Some(TaskRunStatus::Completed)),
        ];

        let result = TaskDependencyService::check_dependencies_satisfied(&dependencies, &statuses);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_would_create_cycle_direct() {
        let all_deps = vec![(1, vec![2]), (2, vec![3])];
        assert!(TaskDependencyService::would_create_cycle(1, 1, &all_deps));
    }

    #[test]
    fn test_would_create_cycle_indirect() {
        let all_deps = vec![(1, vec![2]), (2, vec![3]), (3, vec![])];
        assert!(TaskDependencyService::would_create_cycle(3, 1, &all_deps));
    }

    #[test]
    fn test_topological_sort_simple() {
        let tasks = vec![
            create_test_task(1, vec![]),
            create_test_task(2, vec![1]),
            create_test_task(3, vec![1, 2]),
        ];

        let result = TaskDependencyService::topological_sort(&tasks);
        assert!(result.is_ok());
        
        let sorted = result.unwrap();
        let pos1 = sorted.iter().position(|&x| x == 1).unwrap();
        let pos2 = sorted.iter().position(|&x| x == 2).unwrap(); 
        let pos3 = sorted.iter().position(|&x| x == 3).unwrap();

        assert!(pos1 < pos2); // 1 should come before 2
        assert!(pos1 < pos3); // 1 should come before 3
        assert!(pos2 < pos3); // 2 should come before 3
    }

    #[test]
    fn test_extract_dependency_ids() {
        let tasks = vec![
            create_test_task(1, vec![]),
            create_test_task(2, vec![1]),
            create_test_task(3, vec![1, 2]),
        ];

        let dep_ids = TaskDependencyService::extract_dependency_ids(&tasks);
        let mut sorted_deps = dep_ids;
        sorted_deps.sort();
        assert_eq!(sorted_deps, vec![1, 1, 2]); // 1 appears twice
    }
}