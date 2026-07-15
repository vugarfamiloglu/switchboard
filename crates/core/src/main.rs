//! Switchboard control plane — Axum server. In production it serves the built
//! Leptos WASM bundle (`crates/web/dist`) alongside the JSON API and WebSocket
//! live streams. Phase 0: health endpoint + static SPA hosting + config.

use std::net::SocketAddr;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

const DEFAULT_PORT: u16 = 7930;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=warn".into()),
        )
        .init();

    let cfg = Config::from_env();
    let app = router(&cfg);

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Switchboard control plane listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

struct Config {
    port: u16,
    dist_dir: String,
}

impl Config {
    fn from_env() -> Self {
        let port = std::env::var("SWITCHBOARD_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        let dist_dir =
            std::env::var("SWITCHBOARD_DIST").unwrap_or_else(|_| "crates/web/dist".into());
        Self { port, dist_dir }
    }
}

fn router(cfg: &Config) -> Router {
    // SPA hosting: serve static assets, falling back to index.html so client-side
    // routes resolve. API routes are matched before this fallback.
    let index = format!("{}/index.html", cfg.dist_dir);
    let spa = ServeDir::new(&cfg.dist_dir).fallback(ServeFile::new(index));

    Router::new()
        .route("/api/health", get(health))
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "data": { "service": "switchboard", "version": env!("CARGO_PKG_VERSION") }
    }))
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
