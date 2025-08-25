use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use serde_json::Value;
use chrono::Utc;
use scheduler_domain::entities::{TaskRun, Message, MessageType};
use crate::ports::executor::{TaskExecutionContext, ResourceLimits};

/// 高性能对象池实现，用于重用TaskExecutionContext对象
pub struct TaskExecutionContextPool {
    pool: Arc<Mutex<VecDeque<TaskExecutionContext>>>,
    max_size: usize,
    created_count: Arc<Mutex<usize>>,
    reused_count: Arc<Mutex<usize>>,
}

impl TaskExecutionContextPool {
    /// 创建新的对象池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
            created_count: Arc::new(Mutex::new(0)),
            reused_count: Arc::new(Mutex::new(0)),
        }
    }

    /// 从池中获取TaskExecutionContext，如果池为空则创建新的
    pub fn get(&self, 
        task_run: TaskRun,
        task_type: String,
        parameters: HashMap<String, Value>,
        timeout_seconds: u64,
        environment: HashMap<String, String>,
        working_directory: Option<String>,
        resource_limits: ResourceLimits,
    ) -> TaskExecutionContext {
        // 尝试从池中获取
        if let Ok(mut pool) = self.pool.lock() {
            if let Some(mut context) = pool.pop_front() {
                // 重置并重用对象
                context.task_run = task_run;
                context.task_type = task_type;
                context.parameters = parameters;
                context.timeout_seconds = timeout_seconds;
                context.environment = environment;
                context.working_directory = working_directory;
                context.resource_limits = resource_limits;
                
                // 更新重用计数
                if let Ok(mut count) = self.reused_count.lock() {
                    *count += 1;
                }
                
                return context;
            }
        }

        // 池为空，创建新对象
        if let Ok(mut count) = self.created_count.lock() {
            *count += 1;
        }

        TaskExecutionContext {
            task_run,
            task_type,
            parameters,
            timeout_seconds,
            environment,
            working_directory,
            resource_limits,
        }
    }

    /// 将TaskExecutionContext归还到池中
    pub fn return_to_pool(&self, mut context: TaskExecutionContext) {
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < self.max_size {
                // 清理对象以准备重用
                context.parameters.clear();
                context.environment.clear();
                context.working_directory = None;
                context.resource_limits = ResourceLimits::default();
                
                pool.push_back(context);
            }
            // 如果池满了，就让对象被drop
        }
    }

    /// 获取池的统计信息
    pub fn stats(&self) -> PoolStats {
        let pool_size = self.pool.lock().map(|p| p.len()).unwrap_or(0);
        let created = self.created_count.lock().map(|c| *c).unwrap_or(0);
        let reused = self.reused_count.lock().map(|c| *c).unwrap_or(0);
        
        PoolStats {
            pool_size,
            max_size: self.max_size,
            created_count: created,
            reused_count: reused,
            hit_rate: if created + reused > 0 {
                reused as f64 / (created + reused) as f64
            } else {
                0.0
            },
        }
    }

    /// 清空池
    pub fn clear(&self) {
        if let Ok(mut pool) = self.pool.lock() {
            pool.clear();
        }
    }
}

/// 对象池统计信息
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub pool_size: usize,
    pub max_size: usize,
    pub created_count: usize,
    pub reused_count: usize,
    pub hit_rate: f64,
}

/// RAII守卫，自动归还对象到池
pub struct PooledTaskExecutionContext {
    context: Option<TaskExecutionContext>,
    pool: Arc<TaskExecutionContextPool>,
}

impl PooledTaskExecutionContext {
    pub fn new(context: TaskExecutionContext, pool: Arc<TaskExecutionContextPool>) -> Self {
        Self {
            context: Some(context),
            pool,
        }
    }

    /// 获取内部的TaskExecutionContext引用
    pub fn context(&self) -> &TaskExecutionContext {
        self.context.as_ref().expect("Context should be available")
    }

    /// 获取内部的TaskExecutionContext可变引用
    pub fn context_mut(&mut self) -> &mut TaskExecutionContext {
        self.context.as_mut().expect("Context should be available")
    }

    /// 提取内部的TaskExecutionContext，绕过自动归还
    pub fn into_inner(mut self) -> TaskExecutionContext {
        self.context.take().expect("Context should be available")
    }
}

impl Drop for PooledTaskExecutionContext {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            self.pool.return_to_pool(context);
        }
    }
}

/// 全局TaskExecutionContext池管理器
pub struct GlobalTaskExecutionContextPool {
    pool: TaskExecutionContextPool,
}

impl GlobalTaskExecutionContextPool {
    /// 创建全局池实例
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: TaskExecutionContextPool::new(max_size),
        }
    }

    /// 获取池化的TaskExecutionContext
    pub fn get_pooled(&self,
        task_run: TaskRun,
        task_type: String, 
        parameters: HashMap<String, Value>,
        timeout_seconds: u64,
        environment: HashMap<String, String>,
        working_directory: Option<String>,
        resource_limits: ResourceLimits,
    ) -> PooledTaskExecutionContext {
        let context = self.pool.get(
            task_run,
            task_type,
            parameters,
            timeout_seconds,
            environment,
            working_directory,
            resource_limits,
        );
        
        PooledTaskExecutionContext::new(context, Arc::new(self.pool.clone()))
    }

    /// 获取池统计信息
    pub fn stats(&self) -> PoolStats {
        self.pool.stats()
    }

    /// 清空池
    pub fn clear(&self) {
        self.pool.clear()
    }
}

// 为TaskExecutionContextPool实现Clone以支持Arc包装
impl Clone for TaskExecutionContextPool {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
            created_count: Arc::clone(&self.created_count),
            reused_count: Arc::clone(&self.reused_count),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scheduler_domain::entities::{TaskRunStatus, TaskRun};
    use chrono::Utc;

    fn create_test_task_run() -> TaskRun {
        TaskRun {
            id: 1,
            task_id: 1,
            status: TaskRunStatus::Pending,
            worker_id: None,
            retry_count: 0,
            scheduled_at: Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error_message: None,
            created_at: Utc::now(),
            shard_index: None,
            shard_total: None,
        }
    }

    #[test]
    fn test_pool_creation() {
        let pool = TaskExecutionContextPool::new(10);
        let stats = pool.stats();
        assert_eq!(stats.pool_size, 0);
        assert_eq!(stats.max_size, 10);
        assert_eq!(stats.created_count, 0);
        assert_eq!(stats.reused_count, 0);
    }

    #[test]
    fn test_pool_get_and_return() {
        let pool = TaskExecutionContextPool::new(5);
        
        // 首次获取，应该创建新对象
        let context1 = pool.get(
            create_test_task_run(),
            "test".to_string(),
            HashMap::new(),
            30,
            HashMap::new(),
            None,
            ResourceLimits::default(),
        );
        
        let stats = pool.stats();
        assert_eq!(stats.created_count, 1);
        assert_eq!(stats.reused_count, 0);
        
        // 归还到池
        pool.return_to_pool(context1);
        let stats = pool.stats();
        assert_eq!(stats.pool_size, 1);
        
        // 再次获取，应该重用对象
        let _context2 = pool.get(
            create_test_task_run(),
            "test2".to_string(),
            HashMap::new(),
            60,
            HashMap::new(),
            Some("/tmp".to_string()),
            ResourceLimits::default(),
        );
        
        let stats = pool.stats();
        assert_eq!(stats.created_count, 1);
        assert_eq!(stats.reused_count, 1);
        assert_eq!(stats.pool_size, 0);
    }

    #[test]
    fn test_pooled_context_raii() {
        let pool = Arc::new(TaskExecutionContextPool::new(5));
        
        {
            let _pooled = PooledTaskExecutionContext::new(
                TaskExecutionContext {
                    task_run: create_test_task_run(),
                    task_type: "test".to_string(),
                    parameters: HashMap::new(),
                    timeout_seconds: 30,
                    environment: HashMap::new(),
                    working_directory: None,
                    resource_limits: ResourceLimits::default(),
                },
                pool.clone(),
            );
            
            // pooled在这里还存在，池应该为空
            assert_eq!(pool.stats().pool_size, 0);
        }
        
        // pooled被drop，对象应该归还到池
        assert_eq!(pool.stats().pool_size, 1);
    }

    #[test]
    fn test_global_pool() {
        let global_pool = GlobalTaskExecutionContextPool::new(5);
        
        let pooled = global_pool.get_pooled(
            create_test_task_run(),
            "test".to_string(),
            HashMap::new(),
            30,
            HashMap::new(),
            None,
            ResourceLimits::default(),
        );
        
        // 验证context可以访问
        assert_eq!(pooled.context().task_type, "test");
        assert_eq!(pooled.context().timeout_seconds, 30);
        
        // 统计信息
        let stats = global_pool.stats();
        assert_eq!(stats.created_count, 1);
        
        // pooled会在drop时自动归还
    }
}

/// Message对象池，用于减少消息队列中的内存分配
pub struct MessagePool {
    pool: Arc<Mutex<VecDeque<Message>>>,
    max_size: usize,
    created_count: Arc<Mutex<usize>>,
    reused_count: Arc<Mutex<usize>>,
}

impl MessagePool {
    /// 创建新的Message对象池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
            created_count: Arc::new(Mutex::new(0)),
            reused_count: Arc::new(Mutex::new(0)),
        }
    }

    /// 从池中获取Message，如果池为空则创建新的
    pub fn get(&self,
        id: String,
        message_type: MessageType,
        payload: serde_json::Value,
        correlation_id: Option<String>,
    ) -> Message {
        // 尝试从池中获取
        if let Ok(mut pool) = self.pool.lock() {
            if let Some(mut message) = pool.pop_front() {
                // 重置并重用对象
                message.id = id;
                message.message_type = message_type;
                message.payload = payload;
                message.timestamp = Utc::now();
                message.retry_count = 0;
                message.correlation_id = correlation_id;
                message.trace_headers = None;
                
                // 更新重用计数
                if let Ok(mut count) = self.reused_count.lock() {
                    *count += 1;
                }
                
                return message;
            }
        }

        // 池为空，创建新对象
        if let Ok(mut count) = self.created_count.lock() {
            *count += 1;
        }

        Message {
            id,
            message_type,
            payload,
            timestamp: Utc::now(),
            retry_count: 0,
            correlation_id,
            trace_headers: None,
        }
    }

    /// 将Message归还到池中
    pub fn return_to_pool(&self, mut message: Message) {
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < self.max_size {
                // 清理对象以准备重用
                message.id.clear();
                message.payload = serde_json::Value::Null;
                message.retry_count = 0;
                message.correlation_id = None;
                message.trace_headers = None;
                
                pool.push_back(message);
            }
            // 如果池满了，就让对象被drop
        }
    }

    /// 获取池的统计信息
    pub fn stats(&self) -> PoolStats {
        let pool_size = self.pool.lock().map(|p| p.len()).unwrap_or(0);
        let created = self.created_count.lock().map(|c| *c).unwrap_or(0);
        let reused = self.reused_count.lock().map(|c| *c).unwrap_or(0);
        
        PoolStats {
            pool_size,
            max_size: self.max_size,
            created_count: created,
            reused_count: reused,
            hit_rate: if created + reused > 0 {
                reused as f64 / (created + reused) as f64
            } else {
                0.0
            },
        }
    }

    /// 清空池
    pub fn clear(&self) {
        if let Ok(mut pool) = self.pool.lock() {
            pool.clear();
        }
    }
}

// 为MessagePool实现Clone以支持Arc包装
impl Clone for MessagePool {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
            created_count: Arc::clone(&self.created_count),
            reused_count: Arc::clone(&self.reused_count),
        }
    }
}

/// RAII守卫，自动归还Message到池
pub struct PooledMessage {
    message: Option<Message>,
    pool: Arc<MessagePool>,
}

impl PooledMessage {
    pub fn new(message: Message, pool: Arc<MessagePool>) -> Self {
        Self {
            message: Some(message),
            pool,
        }
    }

    /// 获取内部的Message引用
    pub fn message(&self) -> &Message {
        self.message.as_ref().expect("Message should be available")
    }

    /// 获取内部的Message可变引用
    pub fn message_mut(&mut self) -> &mut Message {
        self.message.as_mut().expect("Message should be available")
    }

    /// 提取内部的Message，绕过自动归还
    pub fn into_inner(mut self) -> Message {
        self.message.take().expect("Message should be available")
    }
}

impl Drop for PooledMessage {
    fn drop(&mut self) {
        if let Some(message) = self.message.take() {
            self.pool.return_to_pool(message);
        }
    }
}

/// 全局Message池管理器
pub struct GlobalMessagePool {
    pool: MessagePool,
}

impl GlobalMessagePool {
    /// 创建全局Message池实例
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: MessagePool::new(max_size),
        }
    }

    /// 获取池化的Message
    pub fn get_pooled(&self,
        id: String,
        message_type: MessageType,
        payload: serde_json::Value,
        correlation_id: Option<String>,
    ) -> PooledMessage {
        let message = self.pool.get(id, message_type, payload, correlation_id);
        PooledMessage::new(message, Arc::new(self.pool.clone()))
    }

    /// 获取池统计信息
    pub fn stats(&self) -> PoolStats {
        self.pool.stats()
    }

    /// 清空池
    pub fn clear(&self) {
        self.pool.clear();
    }
}