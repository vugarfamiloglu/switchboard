//! Switchboard control plane — Axum server. In production it serves the built
//! Leptos WASM bundle (`crates/web/dist`) alongside the JSON API and WebSocket
//! live streams.

// Scaffold-time: some vault/db/auth helpers are wired incrementally across
// phases. TODO(phase-1): drop this once every module is exercised.
#![allow(dead_code)]

mod api;
mod auth;
mod config;
mod db;
mod sim;
mod state;
mod vault;
mod ws;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::db::Db;
use crate::state::AppState;
use crate::vault::Vault;

const DEFAULT_PASSCODE: &str = "switchboard";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=warn".into()),
        )
        .init();

    let cfg = Config::from_env();
    let db = Db::open(&cfg.db_path())?;
    let vault = Vault::load_or_create(&cfg.vault_key_path())?;

    let secret = ensure_setting(&db, "session_secret", || Ok(auth::new_secret()))?;
    ensure_setting(&db, "passcode_hash", || {
        auth::hash_passcode(cfg.passcode_override.as_deref().unwrap_or(DEFAULT_PASSCODE))
    })?;
    ensure_operators(&db)?;
    tracing::info!("database ready at {} · {} operators seeded", cfg.db_path(), db.operator_count());

    let hub = ws::Hub::new();
    sim::Sim::new(hub.clone()).spawn();

    let port = cfg.port;
    let state = AppState {
        db,
        cfg: Arc::new(cfg),
        vault,
        secret: Arc::new(secret),
        hub,
    };
    let app = router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Switchboard control plane listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn router(state: AppState) -> Router {
    let dist = state.cfg.dist_dir.clone();
    let spa = ServeDir::new(&dist).fallback(ServeFile::new(format!("{dist}/index.html")));
    api::routes(state)
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http())
}

/// Return an existing setting or generate, store, and return it.
fn ensure_setting(
    db: &Db,
    key: &str,
    gen: impl FnOnce() -> anyhow::Result<String>,
) -> anyhow::Result<String> {
    if let Some(v) = db.get_setting(key) {
        return Ok(v);
    }
    let v = gen()?;
    db.set_setting(key, &v)?;
    Ok(v)
}

/// Seed one operator per role on first boot; all share the default passcode.
fn ensure_operators(db: &Db) -> anyhow::Result<()> {
    if db.operator_count() > 0 {
        return Ok(());
    }
    let hash = auth::hash_passcode(DEFAULT_PASSCODE)?;
    let now = auth::now();
    let seed = [
        ("op_owner", "Aygun Mammadova", "owner@switchboard.local", "owner"),
        ("op_admin", "Rashad Guliyev", "admin@switchboard.local", "admin"),
        ("op_operator", "Kamran Aliyev", "operator@switchboard.local", "operator"),
        ("op_viewer", "Nigar Sadigova", "viewer@switchboard.local", "viewer"),
    ];
    for (id, name, email, role) in seed {
        db.insert_operator(id, name, email, role, &hash, now)?;
    }
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
