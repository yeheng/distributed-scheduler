use scheduler_infrastructure::in_memory_queue::{InMemoryMessageQueue, InMemoryQueueConfig};
use scheduler_domain::entities::{Message, TaskExecutionMessage};
use scheduler_domain::messaging::MessageQueue;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    println!("=== 内存队列精确内存计算演示 ===\n");
    
    // 创建配置
    let config = InMemoryQueueConfig {
        memory_limit_mb: 10, // 10MB限制用于演示
        ..Default::default()
    };
    
    let queue = InMemoryMessageQueue::with_config(config);
    let queue_name = "demo_queue";
    
    // 创建队列
    queue.create_queue(queue_name, false).await?;
    println!("✓ 创建队列: {}", queue_name);
    
    // 获取初始内存信息
    let initial_memory = queue.get_memory_info().await;
    println!("初始内存使用: {:.2} MB", initial_memory.total_memory_mb);
    println!("基础开销: {} 字节", initial_memory.base_overhead_bytes);
    
    // 创建不同大小的消息
    let messages = vec![
        create_small_message(1),
        create_medium_message(2),
        create_large_message(3),
    ];
    
    println!("\n=== 发布消息并跟踪内存使用 ===");
    for (i, message) in messages.iter().enumerate() {
        queue.publish_message(queue_name, message).await?;
        
        let memory_info = queue.get_memory_info().await;
        let stats = queue.get_queue_stats().await;
        
        println!("消息 {} 发布后:", i + 1);
        println!("  - 总内存: {:.2} MB ({} 字节)", memory_info.total_memory_mb, memory_info.total_memory_bytes);
        println!("  - 队列消息数: {}", stats.total_messages);
        
        if let Some(queue_detail) = memory_info.queue_details.first() {
            println!("  - 队列内存: {} 字节", queue_detail.memory_bytes);
            println!("  - 平均消息大小: {} 字节", queue_detail.average_message_size);
        }
        println!();
    }
    
    println!("=== 详细内存分析 ===");
    let final_memory = queue.get_memory_info().await;
    println!("总内存使用: {:.2} MB", final_memory.total_memory_mb);
    println!("消息内存: {} 字节", final_memory.queue_memory_bytes);
    println!("基础开销: {} 字节", final_memory.base_overhead_bytes);
    println!("内存效率: {:.1}%", final_memory.memory_efficiency() * 100.0);
    println!("开销比例: {:.1}%", final_memory.overhead_ratio() * 100.0);
    
    // 消费消息并观察内存变化
    println!("\n=== 消费消息并观察内存释放 ===");
    let consumed = queue.consume_messages(queue_name).await?;
    println!("消费了 {} 条消息", consumed.len());
    
    let after_consume_memory = queue.get_memory_info().await;
    println!("消费后内存使用: {:.2} MB", after_consume_memory.total_memory_mb);
    
    let memory_freed = final_memory.total_memory_bytes.saturating_sub(after_consume_memory.total_memory_bytes);
    println!("释放内存: {} 字节 ({:.2} MB)", memory_freed, memory_freed as f64 / (1024.0 * 1024.0));
    
    // 执行垃圾回收
    println!("\n=== 执行垃圾回收 ===");
    let gc_stats = queue.force_gc().await;
    println!("GC 统计:");
    println!("  - 清理队列数: {}", gc_stats.idle_queues_cleaned);
    println!("  - 释放内存: {} MB", gc_stats.memory_freed_mb);
    println!("  - 耗时: {:?}", gc_stats.duration);
    
    Ok(())
}

fn create_small_message(id: i64) -> Message {
    let exec_msg = TaskExecutionMessage {
        task_run_id: id,
        task_id: id,
        task_name: "small_task".to_string(),
        task_type: "shell".to_string(),
        parameters: json!({"cmd": "echo hello"}),
        timeout_seconds: 300,
        retry_count: 0,
        shard_index: None,
        shard_total: None,
    };
    Message::task_execution(exec_msg)
}

fn create_medium_message(id: i64) -> Message {
    let exec_msg = TaskExecutionMessage {
        task_run_id: id,
        task_id: id,
        task_name: "medium_task_with_longer_name".to_string(),
        task_type: "python".to_string(),
        parameters: json!({
            "script": "print('Hello, World!')",
            "args": ["--verbose", "--output", "/tmp/result.txt"],
            "env": {
                "PYTHONPATH": "/usr/local/lib/python3.9",
                "DEBUG": "true"
            }
        }),
        timeout_seconds: 600,
        retry_count: 3,
        shard_index: Some(1),
        shard_total: Some(4),
    };
    Message::task_execution(exec_msg)
}

fn create_large_message(id: i64) -> Message {
    let large_data = (0..1000).map(|i| format!("data_item_{}", i)).collect::<Vec<_>>();
    
    let exec_msg = TaskExecutionMessage {
        task_run_id: id,
        task_id: id,
        task_name: "large_task_with_very_long_descriptive_name_that_takes_up_more_memory".to_string(),
        task_type: "complex_processing".to_string(),
        parameters: json!({
            "input_data": large_data,
            "processing_options": {
                "algorithm": "advanced_ml_processing",
                "parameters": {
                    "learning_rate": 0.001,
                    "batch_size": 128,
                    "epochs": 100,
                    "layers": [256, 128, 64, 32, 16]
                },
                "features": {
                    "normalization": true,
                    "augmentation": true,
                    "validation_split": 0.2
                }
            },
            "output_config": {
                "format": "json",
                "compression": "gzip",
                "encryption": "aes256"
            }
        }),
        timeout_seconds: 3600,
        retry_count: 5,
        shard_index: Some(2),
        shard_total: Some(8),
    };
    Message::task_execution(exec_msg)
}