use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::logging::log_level::LogLevel;

pub struct EventTracker {
    events: Arc<RwLock<VecDeque<EventRecord>>>,
    metrics: Arc<RwLock<EventMetrics>>,
    config: EventTrackerConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventRecord {
    pub timestamp: u64,
    pub event_type: String,
    pub category: EventCategory,
    pub severity: LogLevel,
    pub source: String,
    pub message: String,
    pub data: HashMap<String, serde_json::Value>,
    pub event_id: Option<String>,
    pub related_events: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EventCategory {
    System,
    Task,
    Worker,
    Error,
    Performance,
    Security,
    Configuration,
    Custom,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventMetrics {
    pub total_events: u64,
    pub events_by_category: HashMap<EventCategory, u64>,
    pub events_by_severity: HashMap<LogLevel, u64>,
    pub events_by_source: HashMap<String, u64>,
    pub recent_event_rate: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone)]
pub struct EventTrackerConfig {
    pub max_events: usize,
    pub rate_window_seconds: u64,
    pub enable_aggregation: bool,
    pub aggregation_interval: u64,
}

impl Default for EventTrackerConfig {
    fn default() -> Self {
        Self {
            max_events: 10000,
            rate_window_seconds: 60, // 1 minute
            enable_aggregation: true,
            aggregation_interval: 30, // 30 seconds
        }
    }
}

impl Default for EventTracker {
    fn default() -> Self {
        Self::with_config(EventTrackerConfig::default())
    }
}

impl EventTracker {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_config(config: EventTrackerConfig) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_events))),
            metrics: Arc::new(RwLock::new(EventMetrics::new())),
            config,
        }
    }
    pub async fn track_event(&self, event: EventRecord) {
        let mut events = self.events.write().await;
        events.push_back(event.clone());
        while events.len() > self.config.max_events {
            events.pop_front();
        }
        self.update_metrics(&event).await;
    }
    pub async fn track_simple_event(
        &self,
        event_type: String,
        category: EventCategory,
        severity: LogLevel,
        source: String,
        message: String,
    ) {
        let event = EventRecord {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            event_type,
            category,
            severity,
            source,
            message,
            data: HashMap::new(),
            event_id: None,
            related_events: Vec::new(),
        };

        self.track_event(event).await;
    }
    pub async fn get_recent_events(&self, seconds: u64) -> Vec<EventRecord> {
        let events = self.events.read().await;
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            - seconds;

        events
            .iter()
            .filter(|event| event.timestamp >= cutoff)
            .cloned()
            .collect()
    }
    pub async fn get_events_by_category(&self, category: EventCategory) -> Vec<EventRecord> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|event| event.category == category)
            .cloned()
            .collect()
    }
    pub async fn get_events_by_severity(&self, severity: LogLevel) -> Vec<EventRecord> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|event| event.severity == severity)
            .cloned()
            .collect()
    }
    pub async fn get_metrics(&self) -> EventMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }
    pub async fn clear(&self) {
        let mut events = self.events.write().await;
        let mut metrics = self.metrics.write().await;

        events.clear();
        *metrics = EventMetrics::new();
    }
    async fn update_metrics(&self, event: &EventRecord) {
        let mut metrics = self.metrics.write().await;

        metrics.total_events += 1;
        *metrics
            .events_by_category
            .entry(event.category)
            .or_insert(0) += 1;
        *metrics
            .events_by_severity
            .entry(event.severity)
            .or_insert(0) += 1;
        *metrics
            .events_by_source
            .entry(event.source.clone())
            .or_insert(0) += 1;
        self.update_event_rates(&mut metrics).await;
    }
    async fn update_event_rates(&self, metrics: &mut EventMetrics) {
        let events = self.events.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cutoff = now - self.config.rate_window_seconds;

        let recent_events: u64 = events
            .iter()
            .filter(|event| event.timestamp >= cutoff)
            .count() as u64;

        let recent_errors: u64 = events
            .iter()
            .filter(|event| event.timestamp >= cutoff && event.severity >= LogLevel::Error)
            .count() as u64;

        metrics.recent_event_rate =
            recent_events as f64 / self.config.rate_window_seconds as f64 * 60.0;
        metrics.error_rate = recent_errors as f64 / self.config.rate_window_seconds as f64 * 60.0;
    }
}

impl Default for EventMetrics {
    fn default() -> Self {
        Self {
            total_events: 0,
            events_by_category: HashMap::new(),
            events_by_severity: HashMap::new(),
            events_by_source: HashMap::new(),
            recent_event_rate: 0.0,
            error_rate: 0.0,
        }
    }
}

impl EventMetrics {
    pub fn new() -> Self {
        Self::default()
    }
}

impl EventRecord {
    pub fn new(
        event_type: String,
        category: EventCategory,
        severity: LogLevel,
        source: String,
        message: String,
    ) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            event_type,
            category,
            severity,
            source,
            message,
            data: HashMap::new(),
            event_id: None,
            related_events: Vec::new(),
        }
    }
    pub fn with_data(mut self, key: String, value: serde_json::Value) -> Self {
        self.data.insert(key, value);
        self
    }
    pub fn with_event_id(mut self, event_id: String) -> Self {
        self.event_id = Some(event_id);
        self
    }
    pub fn with_related_event(mut self, related_event_id: String) -> Self {
        self.related_events.push(related_event_id);
        self
    }
}
