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
pub struct Operator {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub created_at: i64,
}

#[derive(Serialize, Clone)]
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
pub struct ConfigProfile {
    pub id: String,
    pub name: String,
    pub values: Value,
    pub created_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Firmware {
    pub id: String,
    pub model: String,
    pub version: String,
    pub size_kb: i64,
    pub sha256: String,
    pub created_at: i64,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
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
