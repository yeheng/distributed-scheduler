use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use futures::{stream, StreamExt};
use scheduler_domain::entities::{ShardConfig, Task, TaskStatus};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Specialized benchmark for the parallelized task scanning optimization
/// This focuses specifically on the buffer_unordered implementation
fn bench_parallel_scan_optimization(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("parallel_scan_optimization");

    // Test different task set sizes
    let task_counts = vec![10, 25, 50, 100, 250, 500];

    for task_count in task_counts {
        group.throughput(Throughput::Elements(task_count as u64));

        group.bench_with_input(
            format!("parallel_scan_{}", task_count),
            &task_count,
            |b, &count| {
                b.iter(|| {
                    // Setup: Create test tasks
                    let tasks = create_realistic_test_tasks(count);

                    // Benchmark the parallel scanning operation
                    let result = runtime.block_on(benchmark_parallel_scan(tasks));
                    std::hint::black_box(result);
                })
            },
        );
    }

    group.finish();
}

/// Benchmark comparing different concurrency strategies
fn bench_concurrency_strategies(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrency_strategies");

    let task_count = 100;
    let concurrency_strategies = vec![
        ("sequential", 1),
        ("low_concurrency", 5),
        ("medium_concurrency", 10),
        ("high_concurrency", 20),
        ("very_high_concurrency", 50),
    ];

    for (strategy_name, concurrency_limit) in concurrency_strategies {
        group.bench_with_input(strategy_name, &concurrency_limit, |b, &limit| {
            b.iter(|| {
                let tasks = create_realistic_test_tasks(task_count);
                let result = runtime.block_on(benchmark_with_concurrency(tasks, limit));
                std::hint::black_box(result);
            })
        });
    }

    group.finish();
}

/// Benchmark for task scheduling latency
fn bench_scheduling_latency(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("scheduling_latency");

    // Measure individual task scheduling latency
    let task_counts = vec![1, 5, 10, 25];

    for task_count in task_counts {
        group.bench_with_input(
            format!("latency_{}_tasks", task_count),
            &task_count,
            |b, &count| {
                b.iter(|| {
                    let tasks = create_latency_test_tasks(count);
                    let start = tokio::time::Instant::now();
                    let _result = runtime.block_on(benchmark_parallel_scan(tasks));
                    let duration = start.elapsed();
                    std::hint::black_box(duration);
                })
            },
        );
    }

    group.finish();
}

/// Benchmark for throughput measurement
fn bench_scheduling_throughput(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("scheduling_throughput");

    let durations = vec![Duration::from_secs(1), Duration::from_secs(3)];

    for duration in durations {
        group.bench_with_input(
            format!("throughput_{}s", duration.as_secs()),
            &duration,
            |b, &dur| {
                b.iter(|| {
                    let tasks = create_continuous_task_stream();
                    let start = tokio::time::Instant::now();
                    let mut scheduled_count = 0;

                    // Run for the specified duration
                    let mut task_stream = tasks.into_iter().cycle();
                    while start.elapsed() < dur {
                        let batch: Vec<_> = task_stream.by_ref().take(50).collect();
                        if batch.is_empty() {
                            break;
                        }

                        let result = runtime.block_on(benchmark_parallel_scan(batch));
                        scheduled_count += result.len();
                    }

                    let actual_duration = start.elapsed();
                    let throughput = scheduled_count as f64 / actual_duration.as_secs_f64();
                    std::hint::black_box(throughput);
                })
            },
        );
    }

    group.finish();
}

/// Benchmark for memory efficiency with streaming
fn bench_streaming_memory_efficiency(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("streaming_memory_efficiency");

    let large_task_counts = vec![1000, 5000, 10000];

    for task_count in large_task_counts {
        group.bench_with_input(
            format!("streaming_{}_tasks", task_count),
            &task_count,
            |b, &count| {
                b.iter(|| {
                    let tasks = create_large_task_set(count);
                    // Benchmark streaming approach to avoid loading all tasks at once
                    let result = runtime.block_on(benchmark_streaming_scan(tasks));
                    std::hint::black_box(result);
                })
            },
        );
    }

    group.finish();
}

/// Benchmark for error resilience in parallel processing
fn bench_parallel_error_resilience(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("parallel_error_resilience");

    let error_rates = vec![0.0, 0.05, 0.1, 0.2, 0.3];
    let task_count = 100;

    for error_rate in error_rates {
        group.bench_with_input(
            format!("error_resilience_{}%", (error_rate * 100.0) as i32),
            &error_rate,
            |b, &rate| {
                b.iter(|| {
                    let tasks = create_tasks_with_failure_scenarios(task_count, rate);
                    let result = runtime.block_on(benchmark_error_handling(tasks));
                    std::hint::black_box(result);
                })
            },
        );
    }

    group.finish();
}

// Helper implementations for benchmarking

async fn benchmark_parallel_scan(tasks: Vec<Task>) -> Vec<Task> {
    // Simulate the parallel scanning logic from the actual implementation
    let concurrency_limit = 10;

    let scheduled_tasks: Vec<Task> = stream::iter(tasks)
        .map(|task| {
            async move {
                // Simulate task scheduling logic
                tokio::time::sleep(Duration::from_micros(100)).await; // Simulate work
                task // Return scheduled task
            }
        })
        .buffer_unordered(concurrency_limit)
        .collect()
        .await;

    scheduled_tasks
}

async fn benchmark_with_concurrency(tasks: Vec<Task>, concurrency_limit: usize) -> Vec<Task> {
    let scheduled_tasks: Vec<Task> = stream::iter(tasks)
        .map(|task| async move {
            tokio::time::sleep(Duration::from_micros(100)).await;
            task
        })
        .buffer_unordered(concurrency_limit)
        .collect()
        .await;

    scheduled_tasks
}

async fn benchmark_streaming_scan(tasks: Vec<Task>) -> Vec<Task> {
    // Simulate streaming approach to process large task sets efficiently
    let batch_size = 100;
    let concurrency_limit = 10;

    let mut results = Vec::new();

    for chunk in tasks.chunks(batch_size) {
        let chunk_results: Vec<Task> = stream::iter(chunk.to_vec())
            .map(|task| async move {
                tokio::time::sleep(Duration::from_micros(50)).await;
                task
            })
            .buffer_unordered(concurrency_limit)
            .collect()
            .await;

        results.extend(chunk_results);
    }

    results
}

async fn benchmark_error_handling(tasks: Vec<Task>) -> Vec<Task> {
    let concurrency_limit = 10;

    let scheduled_tasks: Vec<Task> = stream::iter(tasks)
        .map(|task| {
            async move {
                // Simulate potential errors
                if task.name.contains("error") {
                    tokio::time::sleep(Duration::from_millis(10)).await; // Error handling is faster
                    return task; // Continue processing despite errors
                }

                tokio::time::sleep(Duration::from_micros(100)).await;
                task
            }
        })
        .buffer_unordered(concurrency_limit)
        .collect()
        .await;

    scheduled_tasks
}

// Helper functions for creating test data

fn create_realistic_test_tasks(count: usize) -> Vec<Task> {
    let mut tasks = Vec::new();
    let now = chrono::Utc::now();

    for i in 0..count {
        let task = Task {
            id: i as i64 + 1,
            name: format!("realistic_task_{}", i),
            task_type: "scheduled".to_string(),
            schedule: "0 */5 * * * *".to_string(), // Every 5 minutes
            parameters: serde_json::json!({
                "task_id": i,
                "payload": format!("data_{}", i),
                "complexity": if i % 5 == 0 { "high" } else { "low" }
            }),
            timeout_seconds: if i % 7 == 0 { 600 } else { 300 },
            max_retries: 3,
            status: TaskStatus::Active,
            dependencies: if i > 10 && i % 4 == 0 {
                vec![(i - 1) as i64]
            } else {
                vec![]
            },
            shard_config: if i % 3 == 0 {
                Some(ShardConfig {
                    enabled: true,
                    shard_count: 5,
                    shard_key: format!("shard_{}", i % 5),
                })
            } else {
                None
            },
            created_at: now - chrono::Duration::hours(i as i64 % 24),
            updated_at: now - chrono::Duration::minutes(i as i64 % 60),
        };
        tasks.push(task);
    }

    tasks
}

fn create_latency_test_tasks(count: usize) -> Vec<Task> {
    let mut tasks = Vec::new();
    let now = chrono::Utc::now();

    for i in 0..count {
        let task = Task {
            id: i as i64 + 1,
            name: format!("latency_test_{}", i),
            task_type: "immediate".to_string(),
            schedule: "* * * * * *".to_string(), // Every minute
            parameters: serde_json::json!({"latency_test": true}),
            timeout_seconds: 30,
            max_retries: 1,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: now,
            updated_at: now,
        };
        tasks.push(task);
    }

    tasks
}

fn create_continuous_task_stream() -> Vec<Task> {
    create_realistic_test_tasks(1000) // Large pool for continuous testing
}

fn create_large_task_set(count: usize) -> Vec<Task> {
    create_realistic_test_tasks(count)
}

fn create_tasks_with_failure_scenarios(count: usize, error_rate: f64) -> Vec<Task> {
    let mut tasks = Vec::new();
    let now = chrono::Utc::now();

    for i in 0..count {
        let should_fail = (i as f64 / count as f64) < error_rate;

        let task = Task {
            id: i as i64 + 1,
            name: if should_fail {
                format!("error_task_{}", i)
            } else {
                format!("normal_task_{}", i)
            },
            task_type: "failure_test".to_string(),
            schedule: "0 */10 * * * *".to_string(), // Every 10 minutes
            parameters: if should_fail {
                serde_json::json!({"trigger_error": true})
            } else {
                serde_json::json!({"normal_task": true})
            },
            timeout_seconds: 300,
            max_retries: 3,
            status: TaskStatus::Active,
            dependencies: vec![],
            shard_config: None,
            created_at: now,
            updated_at: now,
        };
        tasks.push(task);
    }

    tasks
}

criterion_group!(
    benches,
    bench_parallel_scan_optimization,
    bench_concurrency_strategies,
    bench_scheduling_latency,
    bench_scheduling_throughput,
    bench_streaming_memory_efficiency,
    bench_parallel_error_resilience
);

criterion_main!(benches);
