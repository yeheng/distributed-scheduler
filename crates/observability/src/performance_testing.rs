use crate::metrics_collector::{MetricsCollector, PerformanceMetrics};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct PerformanceBenchmark {
    pub name: String,
    pub description: String,
    pub target_duration_ms: u64,
    pub max_memory_mb: f64,
    pub max_cpu_percent: f64,
    pub success_criteria: Vec<PerformanceCriterion>,
}

#[derive(Debug, Clone)]
pub struct PerformanceCriterion {
    pub name: String,
    pub metric_name: String,
    pub condition: CriterionCondition,
    pub threshold: f64,
    pub weight: f64, // 0.0 to 1.0 for weighted scoring
}

#[derive(Debug, Clone)]
pub enum CriterionCondition {
    LessThan,
    GreaterThan,
    Equals,
    WithinPercent(f64), // Within percentage of threshold
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub benchmark_name: String,
    pub duration_ms: u64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub criterion_results: Vec<CriterionResult>,
    pub overall_score: f64,
    pub passed: bool,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct CriterionResult {
    pub criterion_name: String,
    pub metric_name: String,
    pub actual_value: f64,
    pub threshold: f64,
    pub passed: bool,
    pub score: f64,
    pub weight: f64,
    pub details: String,
}

pub struct PerformanceRegressionTester {
    benchmarks: Arc<RwLock<Vec<PerformanceBenchmark>>>,
    metrics_collector: Arc<MetricsCollector>,
    test_history: Arc<RwLock<Vec<BenchmarkResult>>>,
    regression_threshold: f64, // Performance degradation threshold (0.0-1.0)
}

impl PerformanceRegressionTester {
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self {
            benchmarks: Arc::new(RwLock::new(Self::default_benchmarks())),
            metrics_collector,
            test_history: Arc::new(RwLock::new(Vec::new())),
            regression_threshold: 0.15, // 15% degradation threshold
        }
    }

    fn default_benchmarks() -> Vec<PerformanceBenchmark> {
        vec![
            PerformanceBenchmark {
                name: "Task Scheduling Throughput".to_string(),
                description: "Test task scheduling performance under load".to_string(),
                target_duration_ms: 1000,
                max_memory_mb: 100.0,
                max_cpu_percent: 50.0,
                success_criteria: vec![
                    PerformanceCriterion {
                        name: "Throughput".to_string(),
                        metric_name: "tasks_per_second".to_string(),
                        condition: CriterionCondition::GreaterThan,
                        threshold: 100.0,
                        weight: 0.4,
                    },
                    PerformanceCriterion {
                        name: "Latency".to_string(),
                        metric_name: "avg_latency_ms".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 50.0,
                        weight: 0.3,
                    },
                    PerformanceCriterion {
                        name: "Success Rate".to_string(),
                        metric_name: "success_rate_percent".to_string(),
                        condition: CriterionCondition::GreaterThan,
                        threshold: 95.0,
                        weight: 0.3,
                    },
                ],
            },
            PerformanceBenchmark {
                name: "Database Query Performance".to_string(),
                description: "Test database query performance".to_string(),
                target_duration_ms: 2000,
                max_memory_mb: 50.0,
                max_cpu_percent: 30.0,
                success_criteria: vec![
                    PerformanceCriterion {
                        name: "Query Speed".to_string(),
                        metric_name: "avg_query_time_ms".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 100.0,
                        weight: 0.5,
                    },
                    PerformanceCriterion {
                        name: "Connection Pool Usage".to_string(),
                        metric_name: "connection_pool_usage_percent".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 80.0,
                        weight: 0.3,
                    },
                    PerformanceCriterion {
                        name: "Error Rate".to_string(),
                        metric_name: "query_error_rate_percent".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 1.0,
                        weight: 0.2,
                    },
                ],
            },
            PerformanceBenchmark {
                name: "Message Queue Throughput".to_string(),
                description: "Test message queue processing performance".to_string(),
                target_duration_ms: 1500,
                max_memory_mb: 75.0,
                max_cpu_percent: 40.0,
                success_criteria: vec![
                    PerformanceCriterion {
                        name: "Message Processing Rate".to_string(),
                        metric_name: "messages_per_second".to_string(),
                        condition: CriterionCondition::GreaterThan,
                        threshold: 500.0,
                        weight: 0.4,
                    },
                    PerformanceCriterion {
                        name: "Consumer Lag".to_string(),
                        metric_name: "consumer_lag_ms".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 100.0,
                        weight: 0.3,
                    },
                    PerformanceCriterion {
                        name: "Queue Depth".to_string(),
                        metric_name: "max_queue_depth".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 1000.0,
                        weight: 0.3,
                    },
                ],
            },
            PerformanceBenchmark {
                name: "API Response Time".to_string(),
                description: "Test API endpoint response times".to_string(),
                target_duration_ms: 3000,
                max_memory_mb: 25.0,
                max_cpu_percent: 25.0,
                success_criteria: vec![
                    PerformanceCriterion {
                        name: "Response Time P50".to_string(),
                        metric_name: "p50_response_time_ms".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 100.0,
                        weight: 0.3,
                    },
                    PerformanceCriterion {
                        name: "Response Time P95".to_string(),
                        metric_name: "p95_response_time_ms".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 300.0,
                        weight: 0.4,
                    },
                    PerformanceCriterion {
                        name: "Error Rate".to_string(),
                        metric_name: "api_error_rate_percent".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 2.0,
                        weight: 0.3,
                    },
                ],
            },
            PerformanceBenchmark {
                name: "Cache Performance".to_string(),
                description: "Test cache hit rates and operation speeds".to_string(),
                target_duration_ms: 500,
                max_memory_mb: 30.0,
                max_cpu_percent: 20.0,
                success_criteria: vec![
                    PerformanceCriterion {
                        name: "Cache Hit Rate".to_string(),
                        metric_name: "cache_hit_rate_percent".to_string(),
                        condition: CriterionCondition::GreaterThan,
                        threshold: 90.0,
                        weight: 0.5,
                    },
                    PerformanceCriterion {
                        name: "Cache Operation Time".to_string(),
                        metric_name: "avg_cache_operation_time_ms".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 5.0,
                        weight: 0.3,
                    },
                    PerformanceCriterion {
                        name: "Cache Memory Usage".to_string(),
                        metric_name: "cache_memory_usage_mb".to_string(),
                        condition: CriterionCondition::LessThan,
                        threshold: 50.0,
                        weight: 0.2,
                    },
                ],
            },
        ]
    }

    pub async fn run_benchmark(&self, benchmark_name: &str) -> Result<BenchmarkResult> {
        let benchmarks = self.benchmarks.read().await;
        let benchmark = benchmarks
            .iter()
            .find(|b| b.name == benchmark_name)
            .ok_or_else(|| anyhow::anyhow!("Benchmark '{}' not found", benchmark_name))?;

        info!("Running benchmark: {}", benchmark.name);

        let start_time = Instant::now();
        let start_metrics = self.collect_system_metrics().await?;

        // Execute benchmark-specific logic
        let benchmark_metrics = self.execute_benchmark_logic(benchmark).await?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let end_metrics = self.collect_system_metrics().await?;

        // Calculate resource usage during benchmark
        let memory_usage_mb = end_metrics.memory_usage_mb - start_metrics.memory_usage_mb;
        let cpu_usage_percent =
            (end_metrics.cpu_usage_percent + start_metrics.cpu_usage_percent) / 2.0;

        // Evaluate criteria
        let criterion_results = self
            .evaluate_criteria(benchmark, &benchmark_metrics)
            .await?;

        // Calculate overall score
        let overall_score = criterion_results
            .iter()
            .map(|cr| cr.score * cr.weight)
            .sum::<f64>();

        let passed = overall_score >= 0.7
            && duration_ms <= benchmark.target_duration_ms
            && memory_usage_mb <= benchmark.max_memory_mb
            && cpu_usage_percent <= benchmark.max_cpu_percent;

        let result = BenchmarkResult {
            benchmark_name: benchmark.name.clone(),
            duration_ms,
            memory_usage_mb,
            cpu_usage_percent,
            criterion_results,
            overall_score,
            passed,
            timestamp: std::time::SystemTime::now(),
        };

        // Store result
        let mut history = self.test_history.write().await;
        history.push(result.clone());

        // Keep only last 1000 results
        if history.len() > 1000 {
            history.remove(0);
        }

        info!(
            benchmark_name = %benchmark.name,
            duration_ms = result.duration_ms,
            overall_score = result.overall_score,
            passed = result.passed,
            "Benchmark completed"
        );

        Ok(result)
    }

    pub async fn run_all_benchmarks(&self) -> Result<Vec<BenchmarkResult>> {
        let benchmarks = self.benchmarks.read().await;
        let mut results = Vec::new();

        for benchmark in benchmarks.iter() {
            match self.run_benchmark(&benchmark.name).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Failed to run benchmark '{}': {}", benchmark.name, e);
                }
            }
        }

        Ok(results)
    }

    async fn collect_system_metrics(&self) -> Result<PerformanceMetrics> {
        // Placeholder for actual system metrics collection
        // In a real implementation, this would collect actual system metrics
        Ok(PerformanceMetrics {
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0.0,
            disk_usage_mb: 0.0,
            network_io_mb: 0.0,
            timestamp: std::time::SystemTime::now(),
        })
    }

    async fn execute_benchmark_logic(
        &self,
        benchmark: &PerformanceBenchmark,
    ) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();

        // Simulate benchmark execution based on benchmark type
        match benchmark.name.as_str() {
            "Task Scheduling Throughput" => {
                metrics.insert("tasks_per_second".to_string(), 150.0);
                metrics.insert("avg_latency_ms".to_string(), 25.0);
                metrics.insert("success_rate_percent".to_string(), 98.0);
            }
            "Database Query Performance" => {
                metrics.insert("avg_query_time_ms".to_string(), 45.0);
                metrics.insert("connection_pool_usage_percent".to_string(), 65.0);
                metrics.insert("query_error_rate_percent".to_string(), 0.1);
            }
            "Message Queue Throughput" => {
                metrics.insert("messages_per_second".to_string(), 750.0);
                metrics.insert("consumer_lag_ms".to_string(), 50.0);
                metrics.insert("max_queue_depth".to_string(), 500.0);
            }
            "API Response Time" => {
                metrics.insert("p50_response_time_ms".to_string(), 75.0);
                metrics.insert("p95_response_time_ms".to_string(), 200.0);
                metrics.insert("api_error_rate_percent".to_string(), 0.5);
            }
            "Cache Performance" => {
                metrics.insert("cache_hit_rate_percent".to_string(), 95.0);
                metrics.insert("avg_cache_operation_time_ms".to_string(), 2.0);
                metrics.insert("cache_memory_usage_mb".to_string(), 25.0);
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown benchmark: {}", benchmark.name));
            }
        }

        // Simulate some processing time
        tokio::time::sleep(Duration::from_millis(benchmark.target_duration_ms / 2)).await;

        Ok(metrics)
    }

    async fn evaluate_criteria(
        &self,
        benchmark: &PerformanceBenchmark,
        metrics: &HashMap<String, f64>,
    ) -> Result<Vec<CriterionResult>> {
        let mut results = Vec::new();

        for criterion in &benchmark.success_criteria {
            let actual_value = metrics.get(&criterion.metric_name).copied().unwrap_or(0.0);

            let (passed, score, details) = match criterion.condition {
                CriterionCondition::LessThan => {
                    let passed = actual_value < criterion.threshold;
                    let score = if passed {
                        1.0
                    } else {
                        criterion.threshold / actual_value
                    };
                    let details = format!(
                        "{} < {}: {}",
                        criterion.metric_name, criterion.threshold, actual_value
                    );
                    (passed, score.min(1.0), details)
                }
                CriterionCondition::GreaterThan => {
                    let passed = actual_value > criterion.threshold;
                    let score = if passed {
                        1.0
                    } else {
                        actual_value / criterion.threshold
                    };
                    let details = format!(
                        "{} > {}: {}",
                        criterion.metric_name, criterion.threshold, actual_value
                    );
                    (passed, score.min(1.0), details)
                }
                CriterionCondition::Equals => {
                    let passed = (actual_value - criterion.threshold).abs() < 0.001;
                    let score = if passed { 1.0 } else { 0.0 };
                    let details = format!(
                        "{} ≈ {}: {}",
                        criterion.metric_name, criterion.threshold, actual_value
                    );
                    (passed, score, details)
                }
                CriterionCondition::WithinPercent(percent) => {
                    let diff = (actual_value - criterion.threshold).abs();
                    let allowed_diff = criterion.threshold * percent / 100.0;
                    let passed = diff <= allowed_diff;
                    let score = if passed {
                        1.0
                    } else {
                        1.0 - (diff - allowed_diff) / allowed_diff
                    };
                    let details = format!(
                        "{} within {}% of {}: {}",
                        criterion.metric_name, percent, criterion.threshold, actual_value
                    );
                    (passed, score.max(0.0).min(1.0), details)
                }
            };

            results.push(CriterionResult {
                criterion_name: criterion.name.clone(),
                metric_name: criterion.metric_name.clone(),
                actual_value,
                threshold: criterion.threshold,
                passed,
                score,
                weight: criterion.weight,
                details,
            });
        }

        Ok(results)
    }

    pub async fn detect_regressions(&self) -> Result<Vec<RegressionReport>> {
        let history = self.test_history.read().await;
        let mut regressions = Vec::new();

        if history.len() < 5 {
            return Ok(regressions); // Not enough data
        }

        // Group results by benchmark name
        let mut benchmark_history: HashMap<String, Vec<&BenchmarkResult>> = HashMap::new();
        for result in history.iter() {
            benchmark_history
                .entry(result.benchmark_name.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }

        // Check each benchmark for regressions
        for (benchmark_name, results) in benchmark_history {
            if results.len() < 3 {
                continue;
            }

            // Sort by timestamp (newest first)
            let mut sorted_results = results.clone();
            sorted_results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

            let latest = &sorted_results[0];
            let previous = &sorted_results[1];

            // Check for significant performance degradation
            if self.is_regression(latest, previous) {
                regressions.push(RegressionReport {
                    benchmark_name: benchmark_name.clone(),
                    current_score: latest.overall_score,
                    previous_score: previous.overall_score,
                    score_change: latest.overall_score - previous.overall_score,
                    current_duration: latest.duration_ms,
                    previous_duration: previous.duration_ms,
                    duration_change: latest.duration_ms as i64 - previous.duration_ms as i64,
                    severity: self.calculate_regression_severity(latest, previous),
                    timestamp: latest.timestamp,
                });
            }
        }

        Ok(regressions)
    }

    fn is_regression(&self, current: &BenchmarkResult, previous: &BenchmarkResult) -> bool {
        // Check if score degraded beyond threshold
        let score_degradation =
            (previous.overall_score - current.overall_score) / previous.overall_score;
        if score_degradation > self.regression_threshold {
            return true;
        }

        // Check if duration increased significantly
        if current.duration_ms > previous.duration_ms * 2 {
            return true;
        }

        // Check if any critical criteria failed
        let current_critical_failures = current
            .criterion_results
            .iter()
            .filter(|cr| cr.weight > 0.4 && !cr.passed)
            .count();
        let previous_critical_failures = previous
            .criterion_results
            .iter()
            .filter(|cr| cr.weight > 0.4 && !cr.passed)
            .count();

        current_critical_failures > previous_critical_failures
    }

    fn calculate_regression_severity(
        &self,
        current: &BenchmarkResult,
        previous: &BenchmarkResult,
    ) -> RegressionSeverity {
        let score_degradation =
            (previous.overall_score - current.overall_score) / previous.overall_score;
        let duration_increase = (current.duration_ms as f64 - previous.duration_ms as f64)
            / previous.duration_ms as f64;

        if score_degradation > 0.3 || duration_increase > 1.0 {
            RegressionSeverity::Critical
        } else if score_degradation > 0.2 || duration_increase > 0.5 {
            RegressionSeverity::High
        } else if score_degradation > 0.1 || duration_increase > 0.25 {
            RegressionSeverity::Medium
        } else {
            RegressionSeverity::Low
        }
    }

    pub async fn get_benchmarks(&self) -> Vec<PerformanceBenchmark> {
        self.benchmarks.read().await.clone()
    }

    pub async fn get_performance_trends(
        &self,
        benchmark_name: &str,
    ) -> Result<Option<PerformanceTrend>> {
        let history = self.test_history.read().await;

        let benchmark_results: Vec<_> = history
            .iter()
            .filter(|r| r.benchmark_name == benchmark_name)
            .collect();

        if benchmark_results.len() < 3 {
            return Ok(None);
        }

        // Calculate trends
        let scores: Vec<f64> = benchmark_results.iter().map(|r| r.overall_score).collect();
        let durations: Vec<u64> = benchmark_results.iter().map(|r| r.duration_ms).collect();

        let avg_score = scores.iter().sum::<f64>() / scores.len() as f64;
        let avg_duration = durations.iter().sum::<u64>() / durations.len() as u64;

        // Calculate trend direction
        let recent_scores: Vec<f64> = scores.iter().rev().take(5).cloned().collect();
        let older_scores: Vec<f64> = scores.iter().rev().skip(5).take(5).cloned().collect();

        let score_trend = if older_scores.is_empty() {
            TrendDirection::Stable
        } else {
            let recent_avg = recent_scores.iter().sum::<f64>() / recent_scores.len() as f64;
            let older_avg = older_scores.iter().sum::<f64>() / older_scores.len() as f64;

            if (recent_avg - older_avg).abs() < 0.05 {
                TrendDirection::Stable
            } else if recent_avg > older_avg {
                TrendDirection::Improving
            } else {
                TrendDirection::Degrading
            }
        };

        Ok(Some(PerformanceTrend {
            benchmark_name: benchmark_name.to_string(),
            average_score: avg_score,
            average_duration_ms: avg_duration,
            trend_direction: score_trend,
            data_points: benchmark_results.len(),
            latest_result: (*benchmark_results.last().unwrap()).clone(),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct RegressionReport {
    pub benchmark_name: String,
    pub current_score: f64,
    pub previous_score: f64,
    pub score_change: f64,
    pub current_duration: u64,
    pub previous_duration: u64,
    pub duration_change: i64,
    pub severity: RegressionSeverity,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegressionSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct PerformanceTrend {
    pub benchmark_name: String,
    pub average_score: f64,
    pub average_duration_ms: u64,
    pub trend_direction: TrendDirection,
    pub data_points: usize,
    pub latest_result: BenchmarkResult,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrendDirection {
    Improving,
    Degrading,
    Stable,
}
