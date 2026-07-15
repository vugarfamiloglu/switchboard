//! Delivery handlers — config profiles (desired-state push) and firmware / OTA
//! rollout campaigns.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{err, ok};
use crate::auth::now;
use crate::state::AppState;

// ---- Config profiles --------------------------------------------------------

pub async fn list_profiles(State(st): State<AppState>) -> Response {
    ok(st.db.list_config_profiles())
}

#[derive(Deserialize)]
pub struct ProfileBody {
    pub name: String,
    #[serde(default)]
    pub values: Value,
}

pub async fn create_profile(State(st): State<AppState>, Json(b): Json<ProfileBody>) -> Response {
    if b.name.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "name is required");
    }
    let id = format!("cfg_{}", ulid::Ulid::new().to_string().to_lowercase());
    let values = if b.values.is_null() { json!({}) } else { b.values };
    match st.db.create_config_profile(&id, b.name.trim(), &values.to_string(), now()) {
        Ok(_) => ok(json!({ "id": id })),
        Err(e) => err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn delete_profile(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    match st.db.delete_config_profile(&id) {
        Ok(0) => err(StatusCode::NOT_FOUND, "profile not found"),
        Ok(_) => ok(json!({ "deleted": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct ApplyBody {
    pub device_id: String,
}

pub async fn apply_profile(State(st): State<AppState>, Path(id): Path<String>, Json(b): Json<ApplyBody>) -> Response {
    let values = match st.db.config_profile_values(&id) {
        Some(v) => v,
        None => return err(StatusCode::NOT_FOUND, "profile not found"),
    };
    match st.db.set_twin_desired(&b.device_id, &values, now()) {
        Ok(0) => err(StatusCode::NOT_FOUND, "device not found"),
        Ok(_) => ok(json!({ "applied": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ---- Firmware ---------------------------------------------------------------

pub async fn list_firmware(State(st): State<AppState>) -> Response {
    ok(st.db.list_firmware())
}

#[derive(Deserialize)]
pub struct FirmwareBody {
    pub model: String,
    pub version: String,
    #[serde(default)]
    pub size_kb: i64,
}

pub async fn create_firmware(State(st): State<AppState>, Json(b): Json<FirmwareBody>) -> Response {
    if b.model.trim().is_empty() || b.version.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "model and version are required");
    }
    let id = format!("fw_{}", ulid::Ulid::new().to_string().to_lowercase());
    let sha = format!("{:x}", Sha256::digest(format!("{}:{}", b.model.trim(), b.version.trim()).as_bytes()));
    let size = if b.size_kb > 0 { b.size_kb } else { 1024 };
    match st.db.create_firmware(&id, b.model.trim(), b.version.trim(), size, &sha[..16], now()) {
        Ok(_) => ok(json!({ "id": id })),
        Err(e) => err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn delete_firmware(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    match st.db.delete_firmware(&id) {
        Ok(0) => err(StatusCode::NOT_FOUND, "firmware not found"),
        Ok(_) => ok(json!({ "deleted": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ---- OTA campaigns ----------------------------------------------------------

pub async fn list_campaigns(State(st): State<AppState>) -> Response {
    ok(st.db.list_campaigns())
}

#[derive(Deserialize)]
pub struct CampaignBody {
    pub firmware_id: String,
    #[serde(default)]
    pub fleet_id: Option<String>,
    #[serde(default = "hundred")]
    pub canary_pct: i64,
}

fn hundred() -> i64 {
    100
}

pub async fn create_campaign(State(st): State<AppState>, Json(b): Json<CampaignBody>) -> Response {
    if b.firmware_id.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "firmware is required");
    }
    let fleet = b.fleet_id.as_deref().filter(|s| !s.is_empty());
    let devices = st.db.list_devices();
    let in_scope = devices
        .iter()
        .filter(|d| fleet.is_none() || d.fleet_id.as_deref() == fleet)
        .count() as i64;
    let canary = b.canary_pct.clamp(1, 100);
    let total = (in_scope * canary / 100).max(1);
    let id = format!("ota_{}", ulid::Ulid::new().to_string().to_lowercase());
    match st.db.create_campaign(&id, b.firmware_id.trim(), fleet, canary, total, now()) {
        Ok(_) => ok(json!({ "id": id, "total": total })),
        Err(e) => err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}
