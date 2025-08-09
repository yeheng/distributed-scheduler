use metrics::{counter, gauge, histogram};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RedisStreamMetrics {
    pub messages_published: Arc<AtomicU64>,
    pub messages_consumed: Arc<AtomicU64>,
    pub messages_acked: Arc<AtomicU64>,
    pub messages_nacked: Arc<AtomicU64>,
    pub connection_errors: Arc<AtomicU64>,
    pub active_connections: Arc<AtomicU32>,
}

impl Default for RedisStreamMetrics {
    fn default() -> Self {
        Self {
            messages_published: Arc::new(AtomicU64::new(0)),
            messages_consumed: Arc::new(AtomicU64::new(0)),
            messages_acked: Arc::new(AtomicU64::new(0)),
            messages_nacked: Arc::new(AtomicU64::new(0)),
            connection_errors: Arc::new(AtomicU64::new(0)),
            active_connections: Arc::new(AtomicU32::new(0)),
        }
    }
}

impl RedisStreamMetrics {
    pub fn record_message_published(&self) {
        self.messages_published.fetch_add(1, Ordering::Relaxed);
        counter!("redis_stream_messages_published_total").increment(1);
    }
    pub fn record_message_consumed(&self) {
        self.messages_consumed.fetch_add(1, Ordering::Relaxed);
        counter!("redis_stream_messages_consumed_total").increment(1);
    }
    pub fn record_message_acked(&self) {
        self.messages_acked.fetch_add(1, Ordering::Relaxed);
        counter!("redis_stream_messages_acked_total").increment(1);
    }
    pub fn record_message_nacked(&self) {
        self.messages_nacked.fetch_add(1, Ordering::Relaxed);
        counter!("redis_stream_messages_nacked_total").increment(1);
    }
    pub fn record_connection_error(&self) {
        self.connection_errors.fetch_add(1, Ordering::Relaxed);
        counter!("redis_stream_connection_errors_total").increment(1);
    }
    pub fn set_active_connections(&self, count: u32) {
        self.active_connections.store(count, Ordering::Relaxed);
        gauge!("redis_stream_active_connections").set(count as f64);
    }
    pub fn record_operation_duration(&self, operation: &str, duration_ms: f64) {
        histogram!(format!("redis_stream_{}_duration_ms", operation)).record(duration_ms);
    }
    pub fn get_stats(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            messages_published: self.messages_published.load(Ordering::Relaxed),
            messages_consumed: self.messages_consumed.load(Ordering::Relaxed),
            messages_acked: self.messages_acked.load(Ordering::Relaxed),
            messages_nacked: self.messages_nacked.load(Ordering::Relaxed),
            connection_errors: self.connection_errors.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub messages_published: u64,
    pub messages_consumed: u64,
    pub messages_acked: u64,
    pub messages_nacked: u64,
    pub connection_errors: u64,
    pub active_connections: u32,
}
