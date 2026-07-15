//! In-memory live state — the latest telemetry snapshot per device plus a fleet
//! aggregate. The simulator (and, later, the MQTT ingest pipeline) publish here
//! every tick; the console reads it via `GET /api/live` and the WS stream.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::Serialize;

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLive {
    pub online: bool,
    pub last_seen: i64,
    pub metrics: HashMap<String, f64>,
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Aggregate {
    pub msg_rate: f64,
    pub online: u32,
    pub total: u32,
    pub alerts: u32,
}

#[derive(Clone, Default)]
pub struct Live {
    devices: Arc<RwLock<HashMap<String, DeviceLive>>>,
    aggregate: Arc<RwLock<Aggregate>>,
}

impl Live {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, devices: HashMap<String, DeviceLive>, aggregate: Aggregate) {
        *self.devices.write().unwrap() = devices;
        *self.aggregate.write().unwrap() = aggregate;
    }

    pub fn devices(&self) -> HashMap<String, DeviceLive> {
        self.devices.read().unwrap().clone()
    }

    pub fn aggregate(&self) -> Aggregate {
        self.aggregate.read().unwrap().clone()
    }
}
