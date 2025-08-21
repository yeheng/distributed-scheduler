use crate::metrics_collector::MetricsCollector;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub duration_seconds: u64,
    pub severity: AlertSeverity,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    Equals,
    NotEquals,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub rule_name: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold: f64,
    pub severity: AlertSeverity,
    pub timestamp: std::time::SystemTime,
    pub message: String,
}

pub struct AlertManager {
    rules: Arc<RwLock<Vec<AlertRule>>>,
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    metrics_collector: Arc<MetricsCollector>,
    notification_channels: Vec<Arc<dyn NotificationChannel>>,
}

impl AlertManager {
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self {
            rules: Arc::new(RwLock::new(Self::default_rules())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector,
            notification_channels: Vec::new(),
        }
    }

    fn default_rules() -> Vec<AlertRule> {
        vec![
            AlertRule {
                name: "High CPU Usage".to_string(),
                metric_name: "scheduler_system_cpu_usage_percent".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 80.0,
                duration_seconds: 300, // 5 minutes
                severity: AlertSeverity::Warning,
                enabled: true,
            },
            AlertRule {
                name: "Critical CPU Usage".to_string(),
                metric_name: "scheduler_system_cpu_usage_percent".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 95.0,
                duration_seconds: 60, // 1 minute
                severity: AlertSeverity::Critical,
                enabled: true,
            },
            AlertRule {
                name: "High Memory Usage".to_string(),
                metric_name: "scheduler_system_memory_usage_mb".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 4000.0, // 4GB
                duration_seconds: 300,
                severity: AlertSeverity::Warning,
                enabled: true,
            },
            AlertRule {
                name: "High Queue Depth".to_string(),
                metric_name: "scheduler_queue_depth".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 1000.0,
                duration_seconds: 60,
                severity: AlertSeverity::Warning,
                enabled: true,
            },
            AlertRule {
                name: "Database Connection Failures".to_string(),
                metric_name: "scheduler_database_connection_failures_total".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 5.0,
                duration_seconds: 60,
                severity: AlertSeverity::Error,
                enabled: true,
            },
            AlertRule {
                name: "High Task Failure Rate".to_string(),
                metric_name: "scheduler_task_failures_total".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 10.0,
                duration_seconds: 300,
                severity: AlertSeverity::Warning,
                enabled: true,
            },
            AlertRule {
                name: "Worker Failures".to_string(),
                metric_name: "scheduler_worker_failures_total".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 3.0,
                duration_seconds: 300,
                severity: AlertSeverity::Error,
                enabled: true,
            },
            AlertRule {
                name: "API Error Rate".to_string(),
                metric_name: "scheduler_api_error_rate_total".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 20.0,
                duration_seconds: 300,
                severity: AlertSeverity::Warning,
                enabled: true,
            },
            AlertRule {
                name: "Low Cache Hit Rate".to_string(),
                metric_name: "scheduler_cache_hit_rate_percent".to_string(),
                condition: AlertCondition::LessThan,
                threshold: 80.0,
                duration_seconds: 600,
                severity: AlertSeverity::Warning,
                enabled: true,
            },
            AlertRule {
                name: "High Consumer Lag".to_string(),
                metric_name: "scheduler_message_queue_consumer_lag_seconds".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 30.0,
                duration_seconds: 120,
                severity: AlertSeverity::Warning,
                enabled: true,
            },
        ]
    }

    pub fn add_notification_channel(&mut self, channel: Arc<dyn NotificationChannel>) {
        self.notification_channels.push(channel);
    }

    pub async fn add_rule(&self, rule: AlertRule) {
        let mut rules = self.rules.write().await;
        rules.push(rule);
    }

    pub async fn remove_rule(&self, rule_name: &str) {
        let mut rules = self.rules.write().await;
        rules.retain(|rule| rule.name != rule_name);
    }

    pub async fn enable_rule(&self, rule_name: &str) {
        let mut rules = self.rules.write().await;
        if let Some(rule) = rules.iter_mut().find(|r| r.name == rule_name) {
            rule.enabled = true;
        }
    }

    pub async fn disable_rule(&self, rule_name: &str) {
        let mut rules = self.rules.write().await;
        if let Some(rule) = rules.iter_mut().find(|r| r.name == rule_name) {
            rule.enabled = false;
        }
    }

    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let alerts = self.active_alerts.read().await;
        alerts.values().cloned().collect()
    }

    pub async fn start_monitoring(&self) {
        let _rules = self.rules.clone();
        let _active_alerts = self.active_alerts.clone();
        let _notification_channels = self.notification_channels.clone();

        let mut interval = interval(Duration::from_secs(30)); // Check every 30 seconds

        loop {
            interval.tick().await;

            if let Err(e) = self.check_and_update_alerts().await {
                error!("Error checking alerts: {}", e);
            }
        }
    }

    async fn check_and_update_alerts(&self) -> Result<()> {
        let rules = self.rules.read().await;
        let mut active_alerts = self.active_alerts.write().await;
        let mut new_alerts = Vec::new();

        for rule in rules.iter() {
            if !rule.enabled {
                continue;
            }

            if let Some(current_value) = self.get_metric_value(&rule.metric_name).await {
                let condition_met =
                    self.check_condition(&rule.condition, current_value, rule.threshold);
                let alert_key = format!("{}:{}", rule.name, rule.metric_name);

                if condition_met {
                    let alert = Alert {
                        rule_name: rule.name.clone(),
                        metric_name: rule.metric_name.clone(),
                        current_value,
                        threshold: rule.threshold,
                        severity: rule.severity.clone(),
                        timestamp: std::time::SystemTime::now(),
                        message: self.format_alert_message(&rule, current_value),
                    };

                    // Check if this is a new alert or update existing
                    if !active_alerts.contains_key(&alert_key) {
                        new_alerts.push(alert.clone());
                        active_alerts.insert(alert_key, alert);
                    }
                } else {
                    // Remove alert if condition is no longer met
                    if active_alerts.remove(&alert_key).is_some() {
                        info!("Alert resolved: {} - {}", rule.name, rule.metric_name);
                    }
                }
            }
        }

        // Send notifications for new alerts
        for alert in new_alerts {
            self.send_alert_notification(alert.clone()).await;
        }

        Ok(())
    }

    async fn get_metric_value(&self, metric_name: &str) -> Option<f64> {
        // This is a simplified implementation
        // In a real system, you would query the metrics exporter or use a metrics registry
        match metric_name {
            "scheduler_system_cpu_usage_percent" => Some(50.0), // Placeholder
            "scheduler_system_memory_usage_mb" => Some(2000.0), // Placeholder
            "scheduler_queue_depth" => Some(100.0),             // Placeholder
            "scheduler_database_connection_failures_total" => Some(0.0), // Placeholder
            "scheduler_task_failures_total" => Some(5.0),       // Placeholder
            "scheduler_worker_failures_total" => Some(0.0),     // Placeholder
            "scheduler_api_error_rate_total" => Some(2.0),      // Placeholder
            "scheduler_cache_hit_rate_percent" => Some(95.0),   // Placeholder
            "scheduler_message_queue_consumer_lag_seconds" => Some(5.0), // Placeholder
            _ => None,
        }
    }

    fn check_condition(&self, condition: &AlertCondition, current: f64, threshold: f64) -> bool {
        match condition {
            AlertCondition::GreaterThan => current > threshold,
            AlertCondition::LessThan => current < threshold,
            AlertCondition::Equals => (current - threshold).abs() < 0.001,
            AlertCondition::NotEquals => (current - threshold).abs() >= 0.001,
        }
    }

    fn format_alert_message(&self, rule: &AlertRule, current_value: f64) -> String {
        let condition_str = match rule.condition {
            AlertCondition::GreaterThan => ">",
            AlertCondition::LessThan => "<",
            AlertCondition::Equals => "==",
            AlertCondition::NotEquals => "!=",
        };

        format!(
            "{}: {} {} {} (threshold: {})",
            rule.name, rule.metric_name, condition_str, current_value, rule.threshold
        )
    }

    async fn send_alert_notification(&self, alert: Alert) {
        let severity_str = match alert.severity {
            AlertSeverity::Info => "INFO",
            AlertSeverity::Warning => "WARNING",
            AlertSeverity::Error => "ERROR",
            AlertSeverity::Critical => "CRITICAL",
        };

        warn!(
            rule_name = %alert.rule_name,
            metric_name = %alert.metric_name,
            current_value = alert.current_value,
            threshold = alert.threshold,
            severity = severity_str,
            message = %alert.message,
            "ALERT TRIGGERED"
        );

        // Send to all notification channels
        for channel in &self.notification_channels {
            if let Err(e) = channel.send_alert(alert.clone()) {
                error!("Failed to send alert to notification channel: {}", e);
            }
        }
    }

    pub async fn get_alert_summary(&self) -> Result<String> {
        let alerts = self.active_alerts.read().await;
        let rules = self.rules.read().await;

        let active_count = alerts.len();
        let enabled_rules_count = rules.iter().filter(|r| r.enabled).count();

        let mut by_severity = HashMap::new();
        for alert in alerts.values() {
            *by_severity.entry(&alert.severity).or_insert(0) += 1;
        }

        Ok(format!(
            "Alert Summary:\n- Active Alerts: {}\n- Enabled Rules: {}\n- By Severity: {:?}",
            active_count, enabled_rules_count, by_severity
        ))
    }
}

pub trait NotificationChannel: Send + Sync {
    fn send_alert(&self, alert: Alert) -> Result<()>;
}

pub struct LogNotificationChannel {
    name: String,
}

impl LogNotificationChannel {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl NotificationChannel for LogNotificationChannel {
    fn send_alert(&self, alert: Alert) -> Result<()> {
        info!(
            channel = %self.name,
            rule_name = %alert.rule_name,
            severity = ?alert.severity,
            message = %alert.message,
            "Alert notification sent"
        );
        Ok(())
    }
}

pub struct EmailNotificationChannel {
    smtp_server: String,
    sender_email: String,
    recipient_emails: Vec<String>,
}

impl EmailNotificationChannel {
    pub fn new(smtp_server: String, sender_email: String, recipient_emails: Vec<String>) -> Self {
        Self {
            smtp_server,
            sender_email,
            recipient_emails,
        }
    }
}

impl NotificationChannel for EmailNotificationChannel {
    fn send_alert(&self, alert: Alert) -> Result<()> {
        // Placeholder for email sending logic
        info!(
            channel = "email",
            recipients = ?self.recipient_emails,
            rule_name = %alert.rule_name,
            message = %alert.message,
            "Email alert notification would be sent"
        );
        Ok(())
    }
}

pub struct WebhookNotificationChannel {
    webhook_url: String,
    headers: HashMap<String, String>,
}

impl WebhookNotificationChannel {
    pub fn new(webhook_url: String, headers: HashMap<String, String>) -> Self {
        Self {
            webhook_url,
            headers,
        }
    }
}

impl NotificationChannel for WebhookNotificationChannel {
    fn send_alert(&self, alert: Alert) -> Result<()> {
        // Placeholder for webhook sending logic
        info!(
            channel = "webhook",
            url = %self.webhook_url,
            rule_name = %alert.rule_name,
            message = %alert.message,
            "Webhook alert notification would be sent"
        );
        Ok(())
    }
}
