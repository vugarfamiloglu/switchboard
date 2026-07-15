//! Typed client for the Switchboard JSON API. Every response uses the
//! `{ ok, data } | { ok, error }` envelope, which these helpers unwrap.
//!
//! DTO fields mirror the backend wire format; not every field is rendered by the
//! UI, so dead-code is allowed at the module level.
#![allow(dead_code)]

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

pub async fn post_json<B: Serialize, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T, String> {
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

pub async fn put_json<B: Serialize, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    let resp = Request::put(path)
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

pub async fn del_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let resp = Request::delete(path)
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
pub struct Operator {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub created_at: i64,
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
pub struct Rule {
    pub id: String,
    pub name: String,
    pub metric: String,
    pub op: String,
    pub threshold: f64,
    pub severity: String,
    pub enabled: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub id: String,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub state: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigProfile {
    pub id: String,
    pub name: String,
    pub values: serde_json::Value,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Firmware {
    pub id: String,
    pub model: String,
    pub version: String,
    pub size_kb: i64,
    pub sha256: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtaCampaign {
    pub id: String,
    pub firmware_id: String,
    pub firmware_label: Option<String>,
    pub fleet_id: Option<String>,
    pub fleet_name: Option<String>,
    pub canary_pct: i64,
    pub status: String,
    pub total: i64,
    pub updated: i64,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    pub device_id: String,
    pub device_name: Option<String>,
    pub name: String,
    pub args: String,
    pub status: String,
    pub result: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub ts: i64,
    pub level: String,
    pub msg: String,
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

pub async fn alerts() -> Result<Vec<Alert>, String> {
    get_json("/api/alerts").await
}

pub async fn alert_action(id: &str, action: &str) -> Result<serde_json::Value, String> {
    post_json(
        &format!("/api/alerts/{id}/{action}"),
        &serde_json::json!({}),
    )
    .await
}

pub async fn rules() -> Result<Vec<Rule>, String> {
    get_json("/api/rules").await
}

pub async fn create_rule(
    name: &str,
    metric: &str,
    op: &str,
    threshold: f64,
    severity: &str,
) -> Result<serde_json::Value, String> {
    post_json("/api/rules", &serde_json::json!({ "name": name, "metric": metric, "op": op, "threshold": threshold, "severity": severity })).await
}

pub async fn toggle_rule(id: &str, enabled: bool) -> Result<serde_json::Value, String> {
    put_json(
        &format!("/api/rules/{id}"),
        &serde_json::json!({ "enabled": enabled }),
    )
    .await
}

pub async fn delete_rule(id: &str) -> Result<serde_json::Value, String> {
    del_json(&format!("/api/rules/{id}")).await
}

pub async fn rollback_campaign(id: &str) -> Result<serde_json::Value, String> {
    post_json(&format!("/api/ota/{id}/rollback"), &serde_json::json!({})).await
}

pub async fn logs() -> Result<Vec<LogEntry>, String> {
    get_json("/api/logs").await
}

pub async fn commands() -> Result<Vec<Command>, String> {
    get_json("/api/commands").await
}

pub async fn send_command(device: &str, name: &str) -> Result<serde_json::Value, String> {
    post_json(
        &format!("/api/devices/{device}/command"),
        &serde_json::json!({ "name": name }),
    )
    .await
}

pub async fn config_profiles() -> Result<Vec<ConfigProfile>, String> {
    get_json("/api/config-profiles").await
}

pub async fn create_profile(
    name: &str,
    values: serde_json::Value,
) -> Result<serde_json::Value, String> {
    post_json(
        "/api/config-profiles",
        &serde_json::json!({ "name": name, "values": values }),
    )
    .await
}

pub async fn apply_profile(id: &str, device: &str) -> Result<serde_json::Value, String> {
    post_json(
        &format!("/api/config-profiles/{id}/apply"),
        &serde_json::json!({ "device_id": device }),
    )
    .await
}

pub async fn firmware() -> Result<Vec<Firmware>, String> {
    get_json("/api/firmware").await
}

pub async fn create_firmware(model: &str, version: &str) -> Result<serde_json::Value, String> {
    post_json(
        "/api/firmware",
        &serde_json::json!({ "model": model, "version": version }),
    )
    .await
}

pub async fn campaigns() -> Result<Vec<OtaCampaign>, String> {
    get_json("/api/ota").await
}

pub async fn fleets() -> Result<Vec<Fleet>, String> {
    get_json("/api/fleets").await
}

pub async fn create_fleet(name: &str, description: &str) -> Result<serde_json::Value, String> {
    post_json(
        "/api/fleets",
        &serde_json::json!({ "name": name, "description": description }),
    )
    .await
}

pub async fn delete_fleet(id: &str) -> Result<serde_json::Value, String> {
    del_json(&format!("/api/fleets/{id}")).await
}

pub async fn operators() -> Result<Vec<Operator>, String> {
    get_json("/api/operators").await
}

pub async fn create_operator(
    name: &str,
    email: &str,
    role: &str,
    password: &str,
) -> Result<serde_json::Value, String> {
    post_json(
        "/api/operators",
        &serde_json::json!({ "name": name, "email": email, "role": role, "password": password }),
    )
    .await
}

pub async fn delete_operator(id: &str) -> Result<serde_json::Value, String> {
    del_json(&format!("/api/operators/{id}")).await
}

pub async fn change_passcode(current: &str, next: &str) -> Result<serde_json::Value, String> {
    post_json(
        "/api/auth/passcode",
        &serde_json::json!({ "current": current, "next": next }),
    )
    .await
}

pub async fn get_webhook() -> Result<serde_json::Value, String> {
    get_json("/api/webhook").await
}

pub async fn set_webhook(url: &str) -> Result<serde_json::Value, String> {
    put_json("/api/webhook", &serde_json::json!({ "url": url })).await
}

pub async fn create_campaign(
    firmware: &str,
    fleet: Option<String>,
    canary: i64,
) -> Result<serde_json::Value, String> {
    post_json(
        "/api/ota",
        &serde_json::json!({ "firmware_id": firmware, "fleet_id": fleet, "canary_pct": canary }),
    )
    .await
}
