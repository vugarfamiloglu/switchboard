//! WebSocket live-stream hub. The simulator (and, from Phase 1, the MQTT ingest
//! pipeline) broadcast telemetry frames; every connected console receives them.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use tokio::sync::broadcast;

use crate::state::AppState;

#[derive(Clone)]
pub struct Hub {
    tx: broadcast::Sender<String>,
}

impl Hub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    /// Fan a frame out to every subscriber. Ok to call with no listeners.
    pub fn broadcast(&self, msg: String) {
        let _ = self.tx.send(msg);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| client_loop(socket, state.hub.clone()))
}

async fn client_loop(mut socket: WebSocket, hub: Hub) {
    let mut rx = hub.subscribe();
    loop {
        tokio::select! {
            frame = rx.recv() => match frame {
                Ok(txt) => {
                    if socket.send(Message::Text(txt.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(_)) => {} // ignore client chatter for now
                Some(Err(_)) => break,
            },
        }
    }
}
