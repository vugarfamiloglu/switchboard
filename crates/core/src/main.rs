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
mod live;
mod models;
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
    ensure_demo_estate(&db)?;
    tracing::info!(
        "database ready at {} · {} operators · {} devices",
        cfg.db_path(),
        db.operator_count(),
        db.device_count()
    );

    let hub = ws::Hub::new();
    let live = live::Live::new();
    sim::Sim::new(hub.clone(), live.clone(), db.clone()).spawn();

    let port = cfg.port;
    let state = AppState {
        db,
        cfg: Arc::new(cfg),
        vault,
        secret: Arc::new(secret),
        hub,
        live,
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

/// Seed a demo estate (fleets + devices) on first boot so the console has data.
fn ensure_demo_estate(db: &Db) -> anyhow::Result<()> {
    if db.device_count() > 0 {
        return Ok(());
    }
    let now = auth::now();
    let fleets = [
        ("flt_hvac", "HVAC Controllers", "Building climate controllers"),
        ("flt_meters", "Smart Meters", "Grid and water meters"),
        ("flt_trackers", "Asset Trackers", "Fleet GPS trackers"),
    ];
    for (id, name, desc) in fleets {
        db.create_fleet(id, name, desc, now)?;
    }
    let devices = [
        ("Rooftop AHU-01", "AeroTherm X3", "2.4.1", "flt_hvac", "baku,rooftop"),
        ("Rooftop AHU-02", "AeroTherm X3", "2.4.1", "flt_hvac", "baku,rooftop"),
        ("Chiller Plant-A", "AeroTherm C9", "3.1.0", "flt_hvac", "baku,plant"),
        ("Chiller Plant-B", "AeroTherm C9", "3.0.7", "flt_hvac", "ganja,plant"),
        ("Boiler Room-1", "AeroTherm B2", "1.9.4", "flt_hvac", "sumqayit"),
        ("Grid Meter-4471", "VoltEdge M1", "5.2.0", "flt_meters", "baku,grid"),
        ("Grid Meter-4472", "VoltEdge M1", "5.2.0", "flt_meters", "baku,grid"),
        ("Water Meter-118", "AquaPulse W2", "2.0.3", "flt_meters", "ganja,water"),
        ("Water Meter-119", "AquaPulse W2", "2.0.3", "flt_meters", "ganja,water"),
        ("Substation Meter-7", "VoltEdge M3", "5.4.1", "flt_meters", "sumqayit,grid"),
        ("Tracker Truck-12", "PathTag T4", "4.0.2", "flt_trackers", "fleet,truck"),
        ("Tracker Truck-19", "PathTag T4", "4.0.2", "flt_trackers", "fleet,truck"),
        ("Tracker Van-03", "PathTag T4", "4.0.1", "flt_trackers", "fleet,van"),
        ("Tracker Reefer-08", "PathTag T7", "4.3.0", "flt_trackers", "fleet,reefer"),
    ];
    for (name, model, fw, fleet, tags) in devices {
        let id = format!("dev_{}", ulid::Ulid::new().to_string().to_lowercase());
        db.create_device(&id, name, model, fw, Some(fleet), &api::devices::claim_code(), tags, now)?;
    }
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
