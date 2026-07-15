//! Embedded MQTT broker (rumqttd) on :1883. A local link subscribes to
//! `switchboard/+/telemetry` and routes every published message into the shared
//! ingest pipeline — so a real device (or the reference agent) publishing over
//! MQTT lands in the registry, live state, and log stream exactly like the HTTP
//! ingest path. Runs in its own threads; if the broker can't start, the rest of
//! the control plane continues (HTTP ingest still works).

use std::thread;

use rumqttd::{Broker, Config, Notification};

use crate::db::Db;
use crate::ingest;
use crate::ws::Hub;

pub fn start(db: Db, hub: Hub) {
    let config = match build_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("MQTT broker disabled (config error): {e}");
            return;
        }
    };
    let mut broker = Broker::new(config);
    let (mut link_tx, mut link_rx) = match broker.link("switchboard-ingest") {
        Ok(link) => link,
        Err(e) => {
            tracing::warn!("MQTT ingest link failed: {e}");
            return;
        }
    };

    thread::spawn(move || {
        if let Err(e) = broker.start() {
            tracing::error!("MQTT broker error: {e}");
        }
    });

    thread::spawn(move || {
        if link_tx.subscribe("switchboard/#").is_err() {
            tracing::warn!("MQTT ingest subscribe failed");
            return;
        }
        tracing::info!("MQTT broker listening on :1883 (ingest switchboard/+/telemetry)");
        loop {
            match link_rx.recv() {
                Ok(Some(Notification::Forward(fwd))) => {
                    let topic = String::from_utf8_lossy(&fwd.publish.topic).to_string();
                    let parts: Vec<&str> = topic.split('/').collect();
                    if parts.len() == 3 && parts[0] == "switchboard" && parts[2] == "telemetry" {
                        if let Ok(v) =
                            serde_json::from_slice::<serde_json::Value>(&fwd.publish.payload)
                        {
                            let (name, model, reported) = ingest::parse_payload(&v);
                            ingest::apply(&db, &hub, parts[1], &name, &model, &reported, "MQTT");
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
}

fn build_config() -> anyhow::Result<Config> {
    const TOML: &str = r#"
id = 0
[router]
id = 0
max_connections = 10010
max_outgoing_packet_count = 200
max_segment_size = 104857600
max_segment_count = 10
[v4.1]
name = "v4-1"
listen = "0.0.0.0:1883"
next_connection_delay_ms = 1
[v4.1.connections]
connection_timeout_ms = 60000
max_payload_size = 20480
max_inflight_count = 100
dynamic_filters = true
[console]
listen = "127.0.0.1:9430"
"#;
    let cfg = ::config::Config::builder()
        .add_source(::config::File::from_str(TOML, ::config::FileFormat::Toml))
        .build()?
        .try_deserialize()?;
    Ok(cfg)
}
