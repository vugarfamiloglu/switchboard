//! Live telemetry over WebSocket. Opens `/api/ws`, parses each frame into the
//! shared `LiveCtx` signal, and reconnects on drop. Also seeds the signal from
//! the REST snapshot so the UI has data before the first frame arrives.

use futures::StreamExt;
use gloo_net::websocket::{futures::WebSocket, Message};
use leptos::prelude::*;
use serde::Deserialize;

use crate::api::{self, LogEntry, Telemetry};

const LOG_CAP: usize = 200;
const SERIES_CAP: usize = 60;

#[derive(Clone, Copy)]
pub struct LiveCtx {
    pub telemetry: RwSignal<Telemetry>,
    pub logs: RwSignal<Vec<LogEntry>>,
    /// Rolling history of the aggregate message rate for the throughput chart.
    pub series: RwSignal<Vec<f64>>,
    pub connected: RwSignal<bool>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Frame {
    Telemetry { data: Telemetry },
    Log { data: LogEntry },
}

/// Create the live context, seed it from REST, start the WS loop, and provide it.
pub fn provide_live() -> LiveCtx {
    let ctx = LiveCtx {
        telemetry: RwSignal::new(Telemetry::default()),
        logs: RwSignal::new(Vec::new()),
        series: RwSignal::new(Vec::new()),
        connected: RwSignal::new(false),
    };
    provide_context(ctx);

    // Seed from the REST snapshot.
    wasm_bindgen_futures::spawn_local(async move {
        if let Ok(t) = api::live_snapshot().await {
            ctx.telemetry.set(t);
        }
    });

    // Live stream with reconnect.
    let url = ws_url();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            if let Ok(ws) = WebSocket::open(&url) {
                ctx.connected.set(true);
                let (_write, mut read) = ws.split();
                while let Some(Ok(Message::Text(txt))) = read.next().await {
                    match serde_json::from_str::<Frame>(&txt) {
                        Ok(Frame::Telemetry { data }) => {
                            let rate = data.aggregate.msg_rate;
                            ctx.telemetry.set(data);
                            ctx.series.update(|s| {
                                s.push(rate);
                                if s.len() > SERIES_CAP {
                                    let excess = s.len() - SERIES_CAP;
                                    s.drain(0..excess);
                                }
                            });
                        }
                        Ok(Frame::Log { data }) => ctx.logs.update(|v| {
                            v.insert(0, data);
                            v.truncate(LOG_CAP);
                        }),
                        Err(_) => {}
                    }
                }
                ctx.connected.set(false);
            }
            gloo_timers::future::TimeoutFuture::new(2000).await;
        }
    });

    ctx
}

pub fn use_live() -> LiveCtx {
    use_context::<LiveCtx>().expect("LiveCtx provided at app root")
}

fn ws_url() -> String {
    let loc = web_sys::window().expect("window").location();
    let proto = if loc.protocol().unwrap_or_default() == "https:" {
        "wss"
    } else {
        "ws"
    };
    let host = loc.host().unwrap_or_default();
    format!("{proto}://{host}/api/ws")
}
