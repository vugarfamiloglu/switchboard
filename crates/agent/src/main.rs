//! Reference device agent — the device-side counterpart to Switchboard's ingest.
//! It publishes telemetry to the control plane over HTTP; in production the same
//! shape maps directly onto MQTT (publish to `switchboard/{device}/telemetry`,
//! which the embedded broker forwards into the ingest pipeline).
//!
//! Configure via env:
//!   SWITCHBOARD_SERVER (default http://localhost:7930)
//!   SWITCHBOARD_DEVICE (default dev_agent_demo)
//!   SWITCHBOARD_NAME   (default "Field Agent Device")

use std::thread;
use std::time::Duration;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn main() {
    let server = env_or("SWITCHBOARD_SERVER", "http://localhost:7930");
    let device = env_or("SWITCHBOARD_DEVICE", "dev_agent_demo");
    let name = env_or("SWITCHBOARD_NAME", "Field Agent Device");
    let url = format!("{server}/api/ingest/{device}");
    println!("switchboard-agent → {url} (as \"{name}\")");

    let mut tick: f64 = 0.0;
    loop {
        tick += 1.0;
        let temp = 21.0 + (tick * 0.4).sin() * 3.5;
        let rssi = -58.0 + (tick * 0.2).cos() * 6.0;
        let body = serde_json::json!({
            "name": name,
            "model": "FieldKit A1",
            "metrics": {
                "tempC": (temp * 10.0).round() / 10.0,
                "rssiDbm": (rssi * 10.0).round() / 10.0,
            }
        });
        match ureq::post(&url).send_json(body) {
            Ok(_) => println!("published telemetry · tempC={temp:.1}"),
            Err(e) => eprintln!("publish failed: {e}"),
        }
        thread::sleep(Duration::from_secs(2));
    }
}
