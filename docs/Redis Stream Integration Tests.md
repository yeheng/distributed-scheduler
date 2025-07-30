# Redis Stream Integration Tests

This document describes the comprehensive integration tests for the Redis Stream message queue implementation.

## Test Overview

The integration tests are designed to verify that the Redis Stream implementation meets all requirements specified in the Redis Stream queue specification, particularly:

- **需求 2.2**: 与现有系统兼容 (Compatibility with existing systems)
- **需求 4.1**: 高并发处理消息时保持稳定吞吐量 (Stable throughput under high concurrency)
- **需求 4.2**: Redis服务器重启时未确认消息能够被重新消费 (Message recovery after Redis restart)
- **需求 4.3**: 网络分区时能够检测并处理连接恢复 (Network partition handling)
- **需求 4.4**: 消息队列积压时能够通过增加消费者实例来水平扩展 (Horizontal scaling with multiple consumers)

## Test Categories

### 1. Basic Functionality Tests

These tests verify the core Redis Stream operations work correctly:

- `test_redis_stream_publish_message()` - Basic message publishing
- `test_redis_stream_publish_with_retry()` - Publishing with retry mechanism
- `test_redis_stream_consume_messages()` - Message consumption
- `test_redis_stream_consumer_groups()` - Consumer group functionality
- `test_redis_stream_ack_message()` - Message acknowledgment
- `test_redis_stream_nack_message_with_requeue()` - Message rejection with requeue
- `test_redis_stream_nack_message_without_requeue()` - Message rejection without requeue
- `test_redis_stream_queue_management()` - Queue management operations

### 2. Concurrency Tests (需求 4.1, 4.4)

#### `test_redis_stream_concurrent_operations()`
Tests concurrent publishing and consuming operations:
- **Publishers**: 5 concurrent publishers, each publishing 10 messages
- **Consumer**: 1 consumer processing messages concurrently
- **Verification**: Ensures messages are not lost or duplicated
- **Success Criteria**: At least 80% success rate in concurrent operations

#### `test_redis_stream_multiple_consumers()`
Tests horizontal scaling with multiple consumers:
- **Setup**: 3 consumers competing for 15 messages
- **Verification**: Messages are distributed among consumers
- **Success Criteria**: At least one consumer processes messages, no message duplication

### 3. Failure Recovery Tests (需求 4.2, 4.3)

#### `test_redis_stream_failure_recovery()`
Tests two failure recovery scenarios:

**Scenario 1: Consumer Failure Recovery**
- Publishes messages and consumes them without acknowledgment
- Simulates consumer crash (no ACK sent)
- Verifies that messages are recovered as "pending" on next consume
- Ensures no message loss during consumer failures

**Scenario 2: Retry Mechanism**
- Tests message retry with nack and requeue
- Verifies retry count incrementation
- Tests final acknowledgment after retries

### 4. Equivalence Tests (需求 2.2)

#### `test_redis_stream_rabbitmq_equivalence()`
Compares Redis Stream implementation with RabbitMQ implementation:

**Test 1: Basic Operations Equivalence**
- Publishes same messages to both Redis Stream and RabbitMQ
- Consumes messages from both implementations
- Verifies both consume the same number of messages
- Compares message type serialization for consistency

**Test 2: Queue Management Equivalence**
- Tests queue creation, size checking, and deletion
- Ensures both implementations behave consistently

### 5. Performance Tests

#### `test_redis_stream_performance_benchmark()`
Benchmarks Redis Stream performance:
- **Load**: 100 messages for publish/consume cycle
- **Metrics**: Messages per second for publishing and consuming
- **Success Criteria**: At least 10 msg/sec for both operations
- **Purpose**: Ensures performance meets production requirements

### 6. Error Handling Tests

#### `test_redis_stream_error_handling()`
Tests various error conditions:
- Empty queue names
- Queue names with invalid characters
- Very long queue names
- Queue names with special characters

#### `test_message_serialization_edge_cases()`
Tests message serialization with edge cases:
- Special characters (Unicode, emojis)
- Quotes and escape characters
- Large message payloads

## Running the Tests

### Prerequisites

1. **Redis Server**: Tests require a running Redis instance on `localhost:6379`
2. **RabbitMQ Server** (for equivalence tests): Required for `test_redis_stream_rabbitmq_equivalence()`

### Running Individual Test Categories

```bash
# Run all integration tests (requires Redis)
cargo test --test redis_stream_integration_test

# Run specific test
cargo test test_redis_stream_concurrent_operations --test redis_stream_integration_test

# Run tests that don't require Redis
cargo test test_message_serialization_edge_cases --test redis_stream_integration_test
```

### Test Execution Notes

- Most tests are marked with `#[ignore]` and require Redis to be running
- Tests include timeout mechanisms to avoid hanging when Redis is unavailable
- Tests gracefully skip when Redis/RabbitMQ services are not available
- Each test includes proper cleanup to avoid interference between tests

## Test Results Interpretation

### Success Criteria

1. **Concurrency Tests**: 
   - At least 80% message success rate under concurrent load
   - No message duplication or loss

2. **Failure Recovery Tests**:
   - All pending messages recovered after consumer failure
   - Retry mechanism works correctly with proper count incrementation

3. **Equivalence Tests**:
   - Redis Stream and RabbitMQ produce equivalent results
   - Same number of messages processed by both implementations

4. **Performance Tests**:
   - Minimum 10 messages/second throughput
   - Reasonable latency for typical workloads

### Common Issues and Troubleshooting

1. **Redis Connection Failures**:
   - Ensure Redis is running on `localhost:6379`
   - Check Redis configuration allows connections
   - Verify no firewall blocking port 6379

2. **RabbitMQ Connection Failures** (equivalence tests):
   - Ensure RabbitMQ is running on `localhost:5672`
   - Default credentials: `guest/guest`
   - Check RabbitMQ management interface is accessible

3. **Test Timeouts**:
   - Tests include generous timeouts but may need adjustment for slow systems
   - Network latency can affect test timing

4. **Flaky Tests**:
   - Concurrency tests may occasionally fail due to timing issues
   - Re-run tests if occasional failures occur
   - Consistent failures indicate real issues

## Integration with CI/CD

For continuous integration, consider:

1. **Docker Compose**: Use containerized Redis/RabbitMQ for consistent test environments
2. **Test Isolation**: Each test uses unique queue names to avoid conflicts
3. **Cleanup**: Tests include cleanup logic to prevent state leakage
4. **Conditional Execution**: Tests skip gracefully when services are unavailable

## Future Enhancements

Potential additional tests to consider:

1. **Load Testing**: Higher volume tests with thousands of messages
2. **Memory Usage**: Tests for memory consumption under load
3. **Network Partition Simulation**: More sophisticated failure scenarios
4. **Cross-Version Compatibility**: Tests with different Redis versions
5. **Security Testing**: Authentication and authorization scenarios