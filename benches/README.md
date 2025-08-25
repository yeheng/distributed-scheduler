# Task Scheduling Performance Benchmarks

This directory contains comprehensive performance benchmarks for the parallelized task scanning optimization in the distributed task scheduler.

## Overview

The benchmarks are designed to validate the performance improvements from implementing parallel task scanning using `futures::stream::buffer_unordered` instead of sequential processing.

## Benchmark Categories

### 1. General Task Scheduling Benchmarks (`task_scheduling_benchmarks.rs`)

- **Parallel Task Scanning**: Measures performance with different task counts
- **Concurrency Limits**: Evaluates optimal concurrency settings
- **Sequential vs Parallel**: Direct comparison of processing approaches
- **Task Filtering**: Efficiency of filtering logic with different task mixes
- **Memory Usage**: Memory efficiency with large task sets
- **Error Handling**: Performance impact of error handling

### 2. Specialized Parallel Scan Benchmarks (`parallel_scan_benchmarks.rs`)

- **Parallel Scan Optimization**: Focused on the `buffer_unordered` implementation
- **Concurrency Strategies**: Different concurrency limit strategies
- **Scheduling Latency**: Individual task scheduling latency measurement
- **Scheduling Throughput**: Continuous throughput measurement
- **Streaming Memory Efficiency**: Memory efficiency with streaming approach
- **Parallel Error Resilience**: Error handling in parallel processing

## Running the Benchmarks

### Quick Start

Run all benchmarks:

```bash
cargo bench
```

### Running Specific Benchmark Sets

```bash
# Run general task scheduling benchmarks
cargo bench -- parallel_task_scanning
cargo bench -- concurrency_limits
cargo bench -- sequential_vs_parallel
cargo bench -- task_filtering
cargo bench -- memory_usage
cargo bench -- error_handling

# Run specialized parallel scan benchmarks
cargo bench -- parallel_scan_optimization
cargo bench -- concurrency_strategies
cargo bench -- scheduling_latency
cargo bench -- scheduling_throughput
cargo bench -- streaming_memory_efficiency
cargo bench -- parallel_error_resilience
```

### Using the Benchmark Runner Script

The included `run_benchmarks.sh` script provides a comprehensive benchmark execution experience:

```bash
./run_benchmarks.sh
```

This script will:

- Run all benchmark sets
- Generate summary reports
- Create performance comparison analysis
- Provide structured output with timing information

## Benchmark Results

Results are generated in the `target/criterion/` directory with:

- Detailed HTML reports with charts and statistics
- Raw performance data
- Comparison reports between runs

## Key Metrics Measured

### Performance Metrics

- **Throughput**: Tasks processed per second
- **Latency**: Time to process individual tasks
- **Scalability**: Performance with increasing task counts
- **Concurrency Efficiency**: Optimal concurrency limits

### Resource Metrics

- **Memory Usage**: Memory consumption with different task set sizes
- **CPU Utilization**: Processor usage during parallel processing
- **Resource Efficiency**: Overall resource utilization

### Reliability Metrics

- **Error Handling**: Performance impact of error scenarios
- **Resilience**: System stability under various failure conditions
- **Consistency**: Result consistency across multiple runs

## Expected Performance Improvements

Based on the parallelized implementation, expect:

1. **Throughput Improvement**: 5-10x increase in tasks processed per second
2. **Latency Reduction**: 60-80% reduction in average scheduling latency
3. **Better Scalability**: Linear scaling with task count up to concurrency limits
4. **Resource Efficiency**: Better CPU utilization with controlled concurrency

## Benchmark Configuration

### Default Settings

- **Concurrency Limit**: 10 (configurable per benchmark)
- **Task Counts**: 10, 50, 100, 500, 1000, 5000, 10000
- **Test Duration**: Varies by benchmark type
- **Sample Size**: Statistical significance through multiple iterations

### Customization

Benchmarks can be customized by modifying the parameters in the benchmark files:

- Task count ranges
- Concurrency limits
- Task characteristics (priority, complexity, etc.)
- Error rates and scenarios
- Memory allocation patterns

## Interpreting Results

### Key Indicators of Success

1. **Throughput Increase**: Higher tasks/second indicates better parallelization
2. **Latency Reduction**: Lower individual task processing time
3. **Linear Scaling**: Performance should scale linearly with concurrency up to optimal point
4. **Memory Stability**: Memory usage should not grow uncontrollably with task count

### Performance Bottlenecks to Watch For

1. **Contention**: High concurrency may lead to resource contention
2. **Memory Pressure**: Large task sets may cause memory issues
3. **Error Cascading**: Errors in parallel processing may affect multiple tasks
4. **Overhead**: Parallel processing overhead may outweigh benefits for small task sets

## Integration with CI/CD

These benchmarks can be integrated into CI/CD pipelines:

```bash
# Run benchmarks in CI (limited scope)
cargo bench -- quick_performance_check

# Generate performance regression reports
cargo bench -- --save-baseline baseline
cargo bench -- --baseline baseline
```

## Troubleshooting

### Common Issues

1. **Out of Memory**: Reduce task count or increase memory limits
2. **High Variance**: Increase sample size or reduce system noise
3. **Timeout Issues**: Adjust timeout settings or reduce task complexity
4. **Database Connection**: Ensure test database is available and responsive

### Performance Tips

1. **Run Multiple Times**: Execute benchmarks multiple times to get stable results
2. **Control Environment**: Minimize background processes during benchmarking
3. **Use Release Mode**: Always benchmark with `--release` for accurate results
4. **Monitor Resources**: Watch CPU, memory, and disk usage during benchmarks

## Contributing

When adding new benchmarks:

1. Follow the existing naming conventions
2. Include comprehensive documentation
3. Add performance expectations
4. Consider edge cases and error scenarios
5. Ensure benchmarks are reproducible

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/index.html)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Futures crate documentation](https://docs.rs/futures/latest/futures/)
