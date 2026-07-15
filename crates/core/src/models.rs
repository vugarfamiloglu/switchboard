//! Serializable DTOs for the JSON API. Field names are camelCase on the wire.

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fleet {
    pub id: String,
    pub name: String,
    pub description: String,
    pub device_count: i64,
    pub created_at: i64,
}

#[derive(Serialize)]
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
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub ts: i64,
    pub level: String,
    pub msg: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDetail {
    #[serde(flatten)]
    pub device: Device,
    pub desired: Value,
    pub reported: Value,
    pub claim_code: Option<String>,
}
