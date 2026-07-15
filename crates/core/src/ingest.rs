//! Shared telemetry ingest — used by both the HTTP endpoint and the embedded
//! MQTT broker consumer. Upserts the device, records the reported twin and
//! last-seen, and streams a log line so ingest activity is visible live.

use serde_json::{json, Value};

use crate::auth::now;
use crate::db::Db;
use crate::ws::Hub;

pub fn apply(db: &Db, hub: &Hub, device: &str, name: &str, model: &str, reported: &str, source: &str) {
    let ts = now();
    let _ = db.ingest_device(device, name, model, reported, ts);
    let log_id = format!("log_{}", ulid::Ulid::new().to_string().to_lowercase());
    let msg = format!("Telemetry received via {source}");
    let _ = db.insert_log(&log_id, device, ts, "info", &msg);
    hub.broadcast(
        json!({ "type": "log", "ts": ts, "data": {
            "id": log_id, "deviceId": device, "deviceName": name, "ts": ts, "level": "info", "msg": msg
        } })
        .to_string(),
    );
}

/// Normalize a telemetry payload — either `{ name?, model?, metrics }` or a raw
/// metrics object — into (name, model, reported-json-string).
pub fn parse_payload(v: &Value) -> (String, String, String) {
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("Agent Device").to_string();
    let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("Agent").to_string();
    let metrics = v.get("metrics").cloned().unwrap_or_else(|| v.clone());
    (name, model, metrics.to_string())
}
