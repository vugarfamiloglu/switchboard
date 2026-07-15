//! Per-device telemetry simulator. Loads the device estate from SQLite and gives
//! each device a model-appropriate metric profile, advancing it every 2s with a
//! mean-reverting random walk (plus the occasional offline flap and fault). It
//! publishes the live snapshot to shared state, broadcasts it over the WS hub,
//! and periodically persists last_seen + reported twin. Phase 2+ swaps this feed
//! for the MQTT ingest pipeline while keeping the same live/broadcast contract.

use std::collections::HashMap;
use std::time::Duration;

use rand::Rng;
use serde_json::json;

use crate::auth::now;
use crate::db::Db;
use crate::live::{Aggregate, DeviceLive, Live};
use crate::ws::Hub;

const TICK: Duration = Duration::from_secs(2);
const TOUCH_EVERY: u64 = 15; // persist last_seen/reported ~every 30s

enum Profile {
    Hvac,
    Meter,
    Tracker,
    Generic,
}

fn profile_for(model: &str) -> Profile {
    if model.starts_with("AeroTherm") {
        Profile::Hvac
    } else if model.starts_with("VoltEdge") || model.starts_with("AquaPulse") {
        Profile::Meter
    } else if model.starts_with("PathTag") {
        Profile::Tracker
    } else {
        Profile::Generic
    }
}

struct DevSim {
    id: String,
    metrics: HashMap<String, f64>,
    base: HashMap<String, f64>,
    online: bool,
}

impl DevSim {
    fn new(id: String, model: &str, rng: &mut impl Rng) -> Self {
        let (metrics, base) = seed_metrics(&profile_for(model), rng);
        Self { id, metrics, base, online: true }
    }

    fn tick(&mut self, rng: &mut impl Rng) {
        // Rare offline flap so a few devices show as down over time.
        if rng.gen_range(0.0..1.0) < 0.012 {
            self.online = !self.online;
        }
        if !self.online {
            return;
        }
        for (k, v) in self.metrics.iter_mut() {
            if k == "kwh" || k == "odometerKm" {
                *v += rng.gen_range(0.0..0.6); // monotonic counters
            } else {
                let base = self.base.get(k).copied().unwrap_or(*v);
                *v += (base - *v) * 0.1 + rng.gen_range(-1.0..1.0) * (base.abs() * 0.03 + 0.4);
            }
        }
    }

    fn faulted(&self) -> bool {
        if !self.online {
            return true;
        }
        if self.metrics.get("tempC").is_some_and(|t| *t > 30.0) {
            return true;
        }
        if self.metrics.get("batteryPct").is_some_and(|b| *b < 15.0) {
            return true;
        }
        false
    }

    /// The active fault conditions (severity, title, detail) driving alerts.
    fn fault_conditions(&self) -> Vec<(&'static str, &'static str, String)> {
        let mut v = Vec::new();
        if !self.online {
            v.push(("critical", "Device offline", "No telemetry received from the device.".to_string()));
            return v;
        }
        if let Some(t) = self.metrics.get("tempC") {
            if *t > 30.0 {
                v.push(("warning", "High temperature", format!("Temperature {:.1} C exceeds the 30 C threshold.", t)));
            }
        }
        if let Some(b) = self.metrics.get("batteryPct") {
            if *b < 15.0 {
                v.push(("warning", "Low battery", format!("Battery at {:.0}% (below 15%).", b)));
            }
        }
        v
    }

    fn live(&self, ts: i64) -> DeviceLive {
        DeviceLive {
            online: self.online,
            last_seen: ts,
            metrics: self
                .metrics
                .iter()
                .map(|(k, v)| (k.clone(), (v * 10.0).round() / 10.0))
                .collect(),
        }
    }
}

fn seed_metrics(p: &Profile, rng: &mut impl Rng) -> (HashMap<String, f64>, HashMap<String, f64>) {
    let mut m = HashMap::new();
    match p {
        Profile::Hvac => {
            m.insert("tempC".into(), 21.0 + rng.gen_range(-2.0..3.0));
            m.insert("setpointC".into(), 22.0);
            m.insert("fanPct".into(), 55.0 + rng.gen_range(-10.0..10.0));
            m.insert("rssiDbm".into(), -60.0 + rng.gen_range(-8.0..8.0));
        }
        Profile::Meter => {
            m.insert("kwh".into(), rng.gen_range(1000.0..90000.0));
            m.insert("voltageV".into(), 230.0 + rng.gen_range(-4.0..4.0));
            m.insert("rssiDbm".into(), -66.0 + rng.gen_range(-8.0..8.0));
        }
        Profile::Tracker => {
            m.insert("speedKmh".into(), rng.gen_range(0.0..70.0));
            m.insert("batteryPct".into(), rng.gen_range(20.0..100.0));
            m.insert("rssiDbm".into(), -72.0 + rng.gen_range(-10.0..10.0));
        }
        Profile::Generic => {
            m.insert("value".into(), 50.0 + rng.gen_range(-10.0..10.0));
        }
    }
    let base = m.clone();
    (m, base)
}

fn log_line(s: &DevSim, rng: &mut impl Rng) -> (&'static str, String) {
    if !s.online {
        return ("error", "Connection lost — no heartbeat within the keepalive window".into());
    }
    let roll = rng.gen_range(0.0..1.0);
    if roll < 0.07 {
        ("warning", "RSSI degraded — backing off publish rate".into())
    } else if roll < 0.10 {
        ("error", "Publish acknowledgement timed out, retrying".into())
    } else {
        const INFO: &[&str] = &[
            "Telemetry batch published",
            "Heartbeat acknowledged by broker",
            "Desired state reconciled",
            "Sensor sample recorded",
            "Time sync completed",
        ];
        ("info", INFO[rng.gen_range(0..INFO.len())].to_string())
    }
}

pub struct Sim {
    hub: Hub,
    live: Live,
    db: Db,
}

impl Sim {
    pub fn new(hub: Hub, live: Live, db: Db) -> Self {
        Self { hub, live, db }
    }

    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let devices = self.db.list_devices();
        let names: HashMap<String, String> =
            devices.iter().map(|d| (d.id.clone(), d.name.clone())).collect();
        let mut sims: Vec<DevSim> = {
            let mut rng = rand::thread_rng();
            devices
                .iter()
                .map(|d| DevSim::new(d.id.clone(), &d.model, &mut rng))
                .collect()
        };
        tracing::info!("simulator driving {} devices", sims.len());

        let mut ticker = tokio::time::interval(TICK);
        let mut tick_no: u64 = 0;
        loop {
            ticker.tick().await;
            tick_no += 1;
            let ts = now();

            let mut map: HashMap<String, DeviceLive> = HashMap::with_capacity(sims.len());
            let (mut online, mut alerts, mut msgs) = (0u32, 0u32, 0f64);
            {
                let mut rng = rand::thread_rng();
                for s in sims.iter_mut() {
                    s.tick(&mut rng);
                    if s.online {
                        online += 1;
                        msgs += rng.gen_range(1.0..4.0);
                    }
                    if s.faulted() {
                        alerts += 1;
                    }
                    map.insert(s.id.clone(), s.live(ts));
                }
            }
            let aggregate = Aggregate {
                msg_rate: (msgs / 2.0 * 10.0).round() / 10.0,
                online,
                total: sims.len() as u32,
                alerts,
            };

            self.live.publish(map.clone(), aggregate.clone());
            let frame = json!({ "type": "telemetry", "ts": ts, "data": { "devices": map, "aggregate": aggregate } });
            self.hub.broadcast(frame.to_string());

            // Emit a couple of device log lines and stream them live.
            {
                let mut rng = rand::thread_rng();
                for _ in 0..rng.gen_range(1..=2) {
                    if sims.is_empty() {
                        break;
                    }
                    let s = &sims[rng.gen_range(0..sims.len())];
                    let (level, msg) = log_line(s, &mut rng);
                    let id = format!("log_{}", ulid::Ulid::new().to_string().to_lowercase());
                    let name = names.get(&s.id).cloned().unwrap_or_default();
                    let _ = self.db.insert_log(&id, &s.id, ts, level, &msg);
                    let frame = json!({ "type": "log", "ts": ts, "data": {
                        "id": id, "deviceId": s.id, "deviceName": name, "ts": ts, "level": level, "msg": msg
                    } });
                    self.hub.broadcast(frame.to_string());
                }
            }

            if tick_no % TOUCH_EVERY == 0 {
                for s in sims.iter().filter(|s| s.online) {
                    let reported = serde_json::to_string(&s.metrics).unwrap_or_else(|_| "{}".into());
                    let _ = self.db.touch_device(&s.id, &reported, ts);
                }
            }
            if tick_no % TOUCH_EVERY == 7 {
                self.evaluate_alerts(&sims, ts);
            }
            if tick_no % 150 == 0 {
                self.db.prune_logs(1500);
            }
        }
    }

    /// Raise alerts for newly-faulted devices and resolve those whose condition
    /// has cleared. Deduplicated on (device, title) so an open alert isn't
    /// re-raised each cycle.
    fn evaluate_alerts(&self, sims: &[DevSim], now: i64) {
        for s in sims {
            let conds = s.fault_conditions();
            for (severity, title, detail) in &conds {
                if !self.db.open_alert_exists(&s.id, title) {
                    let id = format!("alr_{}", ulid::Ulid::new().to_string().to_lowercase());
                    let _ = self.db.insert_alert(&id, &s.id, severity, title, detail, now);
                }
            }
            let active: Vec<&str> = conds.iter().map(|c| c.1).collect();
            for (id, title) in self.db.open_alerts_for(&s.id) {
                if !active.contains(&title.as_str()) {
                    let _ = self.db.set_alert_state(&id, "resolved", now);
                }
            }
        }
    }
}
