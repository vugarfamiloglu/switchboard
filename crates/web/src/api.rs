//! Typed client for the Switchboard JSON API. Every response uses the
//! `{ ok, data } | { ok, error }` envelope, which these helpers unwrap.

use std::collections::HashMap;

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Envelope<T> {
    ok: bool,
    #[serde(default = "none")]
    data: Option<T>,
    #[serde(default)]
    error: Option<String>,
}

fn none<T>() -> Option<T> {
    None
}

pub async fn get_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let resp = Request::get(path).send().await.map_err(|e| e.to_string())?;
    let env: Envelope<T> = resp.json().await.map_err(|e| e.to_string())?;
    if env.ok {
        env.data.ok_or_else(|| "empty response".to_string())
    } else {
        Err(env.error.unwrap_or_else(|| "request failed".into()))
    }
}

pub async fn post_json<B: Serialize, T: DeserializeOwned>(path: &str, body: &B) -> Result<T, String> {
    let resp = Request::post(path)
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let env: Envelope<T> = resp.json().await.map_err(|e| e.to_string())?;
    if env.ok {
        env.data.ok_or_else(|| "empty response".to_string())
    } else {
        Err(env.error.unwrap_or_else(|| "request failed".into()))
    }
}

// ---- Types (mirror the core DTOs) ------------------------------------------

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub authenticated: bool,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub model: String,
    pub fw_version: String,
    pub fleet_id: Option<String>,
    pub fleet_name: Option<String>,
    pub status: String,
    pub tags: String,
    pub twin_version: i64,
    pub last_seen: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDetail {
    #[serde(flatten)]
    pub device: Device,
    #[serde(default)]
    pub desired: serde_json::Value,
    #[serde(default)]
    pub reported: serde_json::Value,
    pub claim_code: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fleet {
    pub id: String,
    pub name: String,
    pub description: String,
    pub device_count: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLive {
    pub online: bool,
    pub last_seen: i64,
    #[serde(default)]
    pub metrics: HashMap<String, f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aggregate {
    pub msg_rate: f64,
    pub online: u32,
    pub total: u32,
    pub alerts: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Telemetry {
    #[serde(default)]
    pub devices: HashMap<String, DeviceLive>,
    #[serde(default)]
    pub aggregate: Aggregate,
}

// ---- Calls ------------------------------------------------------------------

pub async fn me() -> Result<Session, String> {
    get_json("/api/auth/me").await
}

pub async fn login(creds: &serde_json::Value) -> Result<Session, String> {
    post_json("/api/auth/login", creds).await
}

pub async fn logout() -> Result<serde_json::Value, String> {
    post_json("/api/auth/logout", &serde_json::json!({})).await
}

pub async fn devices() -> Result<Vec<Device>, String> {
    get_json("/api/devices").await
}

pub async fn device(id: &str) -> Result<DeviceDetail, String> {
    get_json(&format!("/api/devices/{id}")).await
}

pub async fn live_snapshot() -> Result<Telemetry, String> {
    get_json("/api/live").await
}
