//! Reference device agent — the device-side counterpart to Switchboard's ingest.
//! It connects to the embedded MQTT broker and publishes telemetry to
//! `switchboard/{device}/telemetry`, which the broker forwards into the control
//! plane's ingest pipeline (registry + live state + logs).
//!
//! Configure via env:
//!   SWITCHBOARD_MQTT_HOST (default localhost)
//!   SWITCHBOARD_MQTT_PORT (default 1883)
//!   SWITCHBOARD_DEVICE    (default dev_agent_demo)
//!   SWITCHBOARD_NAME      (default "Field Agent Device")

use std::thread;
use std::time::Duration;

use rumqttc::{Client, MqttOptions, QoS};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn main() {
    let host = env_or("SWITCHBOARD_MQTT_HOST", "localhost");
    let port: u16 = env_or("SWITCHBOARD_MQTT_PORT", "1883").parse().unwrap_or(1883);
    let device = env_or("SWITCHBOARD_DEVICE", "dev_agent_demo");
    let name = env_or("SWITCHBOARD_NAME", "Field Agent Device");
    let topic = format!("switchboard/{device}/telemetry");

    let mut opts = MqttOptions::new(format!("agent-{device}"), &host, port);
    opts.set_keep_alive(Duration::from_secs(5));
    let (client, mut connection) = Client::new(opts, 10);
    println!("switchboard-agent → mqtt://{host}:{port} publishing {topic}");

    thread::spawn(move || {
        let mut tick = 0.0_f64;
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
            let payload = serde_json::to_vec(&body).unwrap_or_default();
            if let Err(e) = client.publish(&topic, QoS::AtLeastOnce, false, payload) {
                eprintln!("publish error: {e}");
            } else {
                println!("published telemetry · tempC={temp:.1}");
            }
            thread::sleep(Duration::from_secs(2));
        }
    });

    // Drive the MQTT event loop so queued publishes are actually sent.
    for notification in connection.iter() {
        if let Err(e) = notification {
            eprintln!("mqtt: {e}");
            thread::sleep(Duration::from_secs(2));
        }
    }
}
