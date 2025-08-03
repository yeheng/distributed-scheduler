use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::logging::log_level::LogLevel;

/// Event tracking and aggregation for monitoring
pub struct EventTracker {
    events: Arc<RwLock<VecDeque<EventRecord>>>,
    metrics: Arc<RwLock<EventMetrics>>,
    config: EventTrackerConfig,
}

/// Individual event record
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventRecord {
    /// Event timestamp
    pub timestamp: u64,
    /// Event type
    pub event_type: String,
    /// Event category
    pub category: EventCategory,
    /// Event severity
    pub severity: LogLevel,
    /// Source component
    pub source: String,
    /// Event message
    pub message: String,
    /// Additional event data
    pub data: HashMap<String, serde_json::Value>,
    /// Event ID (for correlation)
    pub event_id: Option<String>,
    /// Related event IDs
    pub related_events: Vec<String>,
}

/// Event category classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EventCategory {
    /// System events (startup, shutdown, etc.)
    System,
    /// Task-related events
    Task,
    /// Worker-related events
    Worker,
    /// Error events
    Error,
    /// Performance events
    Performance,
    /// Security events
    Security,
    /// Configuration events
    Configuration,
    /// Custom user-defined events
    Custom,
}

/// Event metrics and statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventMetrics {
    /// Total events tracked
    pub total_events: u64,
    /// Events by category
    pub events_by_category: HashMap<EventCategory, u64>,
    /// Events by severity
    pub events_by_severity: HashMap<LogLevel, u64>,
    /// Events by source
    pub events_by_source: HashMap<String, u64>,
    /// Recent event rate (events per minute)
    pub recent_event_rate: f64,
    /// Error rate (errors per minute)
    pub error_rate: f64,
}

/// Event tracker configuration
#[derive(Debug, Clone)]
pub struct EventTrackerConfig {
    /// Maximum number of events to keep in memory
    pub max_events: usize,
    /// Time window for rate calculations (in seconds)
    pub rate_window_seconds: u64,
    /// Whether to enable event aggregation
    pub enable_aggregation: bool,
    /// Aggregation interval (in seconds)
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
    /// Create new event tracker with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create event tracker with custom configuration
    pub fn with_config(config: EventTrackerConfig) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_events))),
            metrics: Arc::new(RwLock::new(EventMetrics::new())),
            config,
        }
    }

    /// Track a new event
    pub async fn track_event(&self, event: EventRecord) {
        let mut events = self.events.write().await;
        
        // Add new event
        events.push_back(event.clone());
        
        // Enforce maximum event limit
        while events.len() > self.config.max_events {
            events.pop_front();
        }
        
        // Update metrics
        self.update_metrics(&event).await;
    }

    /// Track a simple event with minimal information
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

    /// Get recent events within a time window
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

    /// Get events by category
    pub async fn get_events_by_category(&self, category: EventCategory) -> Vec<EventRecord> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|event| event.category == category)
            .cloned()
            .collect()
    }

    /// Get events by severity
    pub async fn get_events_by_severity(&self, severity: LogLevel) -> Vec<EventRecord> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|event| event.severity == severity)
            .cloned()
            .collect()
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> EventMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Clear all events and reset metrics
    pub async fn clear(&self) {
        let mut events = self.events.write().await;
        let mut metrics = self.metrics.write().await;
        
        events.clear();
        *metrics = EventMetrics::new();
    }

    /// Update metrics based on new event
    async fn update_metrics(&self, event: &EventRecord) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_events += 1;
        *metrics.events_by_category.entry(event.category).or_insert(0) += 1;
        *metrics.events_by_severity.entry(event.severity).or_insert(0) += 1;
        *metrics.events_by_source.entry(event.source.clone()).or_insert(0) += 1;
        
        // Update rate calculations
        self.update_event_rates(&mut metrics).await;
    }

    /// Update event rate calculations
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
        
        metrics.recent_event_rate = recent_events as f64 / self.config.rate_window_seconds as f64 * 60.0;
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
    /// Create new empty metrics
    pub fn new() -> Self {
        Self::default()
    }
}

impl EventRecord {
    /// Create new event record
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

    /// Add data field
    pub fn with_data(mut self, key: String, value: serde_json::Value) -> Self {
        self.data.insert(key, value);
        self
    }

    /// Set event ID
    pub fn with_event_id(mut self, event_id: String) -> Self {
        self.event_id = Some(event_id);
        self
    }

    /// Add related event
    pub fn with_related_event(mut self, related_event_id: String) -> Self {
        self.related_events.push(related_event_id);
        self
    }
}