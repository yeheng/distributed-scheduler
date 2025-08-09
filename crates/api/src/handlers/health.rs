use axum::Json;
use serde_json::{json, Value};

pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "distributed-task-scheduler",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
