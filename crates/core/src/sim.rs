//! Phase-0 simulator skeleton: a 2s ticker advancing a synthetic aggregate
//! (mean-reverting message rate + jittering online count) and broadcasting a
//! telemetry frame over the hub — enough to prove the live path end to end.
//! Phase 1 replaces this with per-device state fed by the MQTT ingest pipeline.

use std::time::Duration;

use rand::Rng;
use serde_json::json;

use crate::auth::now;
use crate::ws::Hub;

const TICK: Duration = Duration::from_secs(2);
const RATE_MEAN: f64 = 2400.0;

pub struct Sim {
    hub: Hub,
}

impl Sim {
    pub fn new(hub: Hub) -> Self {
        Self { hub }
    }

    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mut ticker = tokio::time::interval(TICK);
        let mut rate = RATE_MEAN;
        let mut online: i64 = 128;
        loop {
            ticker.tick().await;
            {
                let mut rng = rand::thread_rng();
                // Mean-reverting random walk so the numbers look alive.
                rate += (RATE_MEAN - rate) * 0.1 + rng.gen_range(-120.0..120.0);
                rate = rate.clamp(800.0, 4200.0);
                online = (online + rng.gen_range(-2..=2)).clamp(120, 134);
            }
            let frame = json!({
                "type": "telemetry",
                "ts": now(),
                "data": {
                    "msgRate": (rate * 10.0).round() / 10.0,
                    "devicesOnline": online,
                    "openAlerts": 3,
                }
            });
            self.hub.broadcast(frame.to_string());
        }
    }
}
