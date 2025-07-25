#[cfg(test)]
mod strategies_tests {
    use std::sync::Arc;

    use crate::strategies::*;
    use chrono::Utc;
    use scheduler_core::*;
    use serde_json::json;

    fn create_test_task(task_type: &str) -> Task {
        Task {
            id: 1,
            name: "test_task".to_string(),
            task_type: task_type.to_string(),
            schedule: "0 0 0 * * *".to_string(),
            parameters: json!({}),
            timeout_seconds: 300,
            max_retries: 0,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_worker(
        id: &str,
        task_types: Vec<&str>,
        current_tasks: i32,
        max_tasks: i32,
    ) -> WorkerInfo {
        WorkerInfo {
            id: id.to_string(),
            hostname: format!("host-{id}"),
            ip_address: "127.0.0.1".to_string(),
            supported_task_types: task_types.iter().map(|s| s.to_string()).collect(),
            max_concurrent_tasks: max_tasks,
            current_task_count: current_tasks,
            status: WorkerStatus::Alive,
            last_heartbeat: Utc::now(),
            registered_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_round_robin_strategy() {
        let strategy = RoundRobinStrategy::new();
        let task = create_test_task("shell");

        let workers = vec![
            create_test_worker("worker1", vec!["shell"], 0, 5),
            create_test_worker("worker2", vec!["shell"], 1, 5),
            create_test_worker("worker3", vec!["shell"], 2, 5),
        ];

        // 测试轮询选择
        let selected1 = strategy.select_worker(&task, &workers).await.unwrap();
        let selected2 = strategy.select_worker(&task, &workers).await.unwrap();
        let selected3 = strategy.select_worker(&task, &workers).await.unwrap();
        let selected4 = strategy.select_worker(&task, &workers).await.unwrap();

        assert!(selected1.is_some());
        assert!(selected2.is_some());
        assert!(selected3.is_some());
        assert!(selected4.is_some());

        // 第四次选择应该回到第一个Worker（轮询）
        assert_eq!(selected1, selected4);
    }

    #[tokio::test]
    async fn test_round_robin_strategy_no_workers() {
        let strategy = RoundRobinStrategy::new();
        let task = create_test_task("shell");

        let result = strategy.select_worker(&task, &[]).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_round_robin_strategy_no_suitable_workers() {
        let strategy = RoundRobinStrategy::new();
        let task = create_test_task("python");

        let workers = vec![
            create_test_worker("worker1", vec!["shell"], 0, 5),
            create_test_worker("worker2", vec!["http"], 1, 5),
        ];

        let result = strategy.select_worker(&task, &workers).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_load_based_strategy() {
        let strategy = LoadBasedStrategy::new();
        let task = create_test_task("shell");

        let workers = vec![
            create_test_worker("worker1", vec!["shell"], 4, 5), // 80% 负载
            create_test_worker("worker2", vec!["shell"], 1, 5), // 20% 负载
            create_test_worker("worker3", vec!["shell"], 3, 5), // 60% 负载
        ];

        let selected = strategy.select_worker(&task, &workers).await.unwrap();
        assert_eq!(selected, Some("worker2".to_string())); // 应该选择负载最低的
    }

    #[tokio::test]
    async fn test_load_based_strategy_full_workers() {
        let strategy = LoadBasedStrategy::new();
        let task = create_test_task("shell");

        let workers = vec![
            create_test_worker("worker1", vec!["shell"], 5, 5), // 100% 负载
            create_test_worker("worker2", vec!["shell"], 5, 5), // 100% 负载
        ];

        let result = strategy.select_worker(&task, &workers).await.unwrap();
        assert!(result.is_none()); // 所有Worker都满载，应该返回None
    }

    #[tokio::test]
    async fn test_task_type_affinity_strategy() {
        let strategy = TaskTypeAffinityStrategy::new();
        let task = create_test_task("shell");

        let workers = vec![
            create_test_worker("worker1", vec!["shell", "http"], 1, 5), // 通用Worker
            create_test_worker("worker2", vec!["shell"], 2, 5),         // 专门的shell Worker
            create_test_worker("worker3", vec!["http"], 0, 5),          // 不支持shell
        ];

        let selected = strategy.select_worker(&task, &workers).await.unwrap();
        assert_eq!(selected, Some("worker2".to_string())); // 应该选择专门的shell Worker
    }

    #[tokio::test]
    async fn test_task_type_affinity_strategy_no_specialized() {
        let strategy = TaskTypeAffinityStrategy::new();
        let task = create_test_task("shell");

        let workers = vec![
            create_test_worker("worker1", vec!["shell", "http"], 3, 5), // 60% 负载
            create_test_worker("worker2", vec!["shell", "python"], 1, 5), // 20% 负载
        ];

        let selected = strategy.select_worker(&task, &workers).await.unwrap();
        assert_eq!(selected, Some("worker2".to_string())); // 应该选择负载较低的
    }

    #[tokio::test]
    async fn test_composite_strategy() {
        let mut composite = CompositeStrategy::new(vec![]);

        // 添加策略：先尝试任务类型亲和，再尝试负载均衡
        composite.add_strategy(Arc::new(TaskTypeAffinityStrategy::new()));
        composite.add_strategy(Arc::new(LoadBasedStrategy::new()));

        let task = create_test_task("shell");

        let workers = vec![
            create_test_worker("worker1", vec!["http"], 0, 5), // 不支持shell
            create_test_worker("worker2", vec!["shell", "http"], 1, 5), // 支持shell，低负载
            create_test_worker("worker3", vec!["shell"], 3, 5), // 专门shell，高负载
        ];

        let selected = composite.select_worker(&task, &workers).await.unwrap();
        assert_eq!(selected, Some("worker3".to_string())); // 任务类型亲和策略应该选择专门的Worker
    }

    #[tokio::test]
    async fn test_composite_strategy_fallback() {
        let strategies: Vec<Arc<dyn TaskDispatchStrategy>> = vec![
            Arc::new(TaskTypeAffinityStrategy::new()),
            Arc::new(LoadBasedStrategy::new()),
        ];
        let composite = CompositeStrategy::new(strategies);

        let task = create_test_task("python");

        let workers = vec![
            create_test_worker("worker1", vec!["shell"], 0, 5), // 不支持python
            create_test_worker("worker2", vec!["http"], 1, 5),  // 不支持python
        ];

        let selected = composite.select_worker(&task, &workers).await.unwrap();
        assert!(selected.is_none()); // 没有Worker支持python，应该返回None
    }

    #[tokio::test]
    async fn test_strategy_names() {
        let round_robin = RoundRobinStrategy::new();
        let load_based = LoadBasedStrategy::new();
        let affinity = TaskTypeAffinityStrategy::new();
        let composite = CompositeStrategy::new(vec![]);

        assert_eq!(round_robin.name(), "RoundRobin");
        assert_eq!(load_based.name(), "LoadBased");
        assert_eq!(affinity.name(), "TaskTypeAffinity");
        assert_eq!(composite.name(), "Composite");
    }
}
