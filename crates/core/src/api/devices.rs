//! Device registry handlers — list, provision/enroll, inspect, update, retire,
//! and set the desired twin.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use super::{err, ok};
use crate::auth::now;
use crate::state::AppState;

pub async fn list(State(st): State<AppState>) -> Response {
    ok(st.db.list_devices())
}

pub async fn get_one(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    match st.db.get_device(&id) {
        Some(d) => ok(d),
        None => err(StatusCode::NOT_FOUND, "device not found"),
    }
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub fw_version: String,
    #[serde(default)]
    pub fleet_id: Option<String>,
    #[serde(default)]
    pub tags: String,
}

pub async fn create(State(st): State<AppState>, Json(b): Json<CreateBody>) -> Response {
    if b.name.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "name is required");
    }
    let id = format!("dev_{}", ulid::Ulid::new().to_string().to_lowercase());
    let claim = claim_code();
    let fleet = b.fleet_id.as_deref().filter(|s| !s.is_empty());
    match st
        .db
        .create_device(&id, b.name.trim(), &b.model, &b.fw_version, fleet, &claim, &b.tags, now())
    {
        Ok(_) => ok(json!({ "id": id, "claimCode": claim })),
        Err(e) => err(StatusCode::BAD_REQUEST, &format!("could not create device: {e}")),
    }
}

#[derive(Deserialize)]
pub struct UpdateBody {
    pub name: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub fleet_id: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub tags: String,
}

fn default_status() -> String {
    "active".into()
}

pub async fn update(State(st): State<AppState>, Path(id): Path<String>, Json(b): Json<UpdateBody>) -> Response {
    let fleet = b.fleet_id.as_deref().filter(|s| !s.is_empty());
    match st.db.update_device(&id, b.name.trim(), &b.model, fleet, &b.status, &b.tags, now()) {
        Ok(0) => err(StatusCode::NOT_FOUND, "device not found"),
        Ok(_) => ok(json!({ "updated": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    match st.db.delete_device(&id) {
        Ok(0) => err(StatusCode::NOT_FOUND, "device not found"),
        Ok(_) => ok(json!({ "deleted": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct TwinBody {
    pub desired: serde_json::Value,
}

pub async fn set_twin(State(st): State<AppState>, Path(id): Path<String>, Json(b): Json<TwinBody>) -> Response {
    match st.db.set_twin_desired(&id, &b.desired.to_string(), now()) {
        Ok(0) => err(StatusCode::NOT_FOUND, "device not found"),
        Ok(_) => ok(json!({ "updated": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct IngestBody {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub metrics: serde_json::Value,
}

/// Public device ingest — a device (or the reference agent) reports telemetry.
/// Registers the device if new, records the reported twin + last-seen, and
/// streams a log line so agent activity is visible live.
pub async fn ingest(State(st): State<AppState>, Path(id): Path<String>, Json(b): Json<IngestBody>) -> Response {
    let name = if b.name.trim().is_empty() { "Agent Device" } else { b.name.trim() };
    let model = if b.model.trim().is_empty() { "Agent" } else { b.model.trim() };
    let reported = if b.metrics.is_null() { "{}".to_string() } else { b.metrics.to_string() };
    let ts = now();
    let _ = st.db.ingest_device(&id, name, model, &reported, ts);
    let log_id = format!("log_{}", ulid::Ulid::new().to_string().to_lowercase());
    let _ = st.db.insert_log(&log_id, &id, ts, "info", "Telemetry received from device agent");
    st.hub.broadcast(
        json!({ "type": "log", "ts": ts, "data": {
            "id": log_id, "deviceId": id, "deviceName": name, "ts": ts, "level": "info", "msg": "Telemetry received from device agent"
        } })
        .to_string(),
    );
    ok(json!({ "accepted": true }))
}

/// A short human-typeable enrollment claim code (Crockford-ish alphabet).
pub fn claim_code() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    let s: String = (0..6).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect();
    format!("SW-{s}")
}
