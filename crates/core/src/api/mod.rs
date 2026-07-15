//! HTTP API — response envelope, router, session cookies, and the RBAC
//! write-guard middleware.

pub mod auth;
pub mod devices;
pub mod fleets;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;

use crate::auth as session;
use crate::state::AppState;

/// Success envelope: `{ "ok": true, "data": ... }`.
pub fn ok<T: Serialize>(data: T) -> Response {
    (StatusCode::OK, Json(json!({ "ok": true, "data": data }))).into_response()
}

/// Error envelope: `{ "ok": false, "error": "..." }`.
pub fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "ok": false, "error": msg }))).into_response()
}

/// Extract and verify the session token from the request's cookie header.
pub fn claims_from_headers(secret: &str, headers: &HeaderMap) -> Option<session::Claims> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{}=", session::SESSION_COOKIE);
    let token = cookie
        .split(';')
        .find_map(|kv| kv.trim().strip_prefix(&prefix))?;
    session::parse_token(secret, token)
}

pub fn set_session_cookie(token: &str) -> String {
    format!(
        "{}={}; HttpOnly; Path=/; SameSite=Lax; Max-Age={}",
        session::SESSION_COOKIE,
        token,
        session::SESSION_TTL_SECS
    )
}

pub fn clear_session_cookie() -> String {
    format!("{}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0", session::SESSION_COOKIE)
}

/// Require a valid session; block mutating methods for read-only roles. Injects
/// `Claims` into request extensions for downstream handlers.
pub async fn require_auth(State(state): State<AppState>, mut req: Request<Body>, next: Next) -> Response {
    let claims = match claims_from_headers(&state.secret, req.headers()) {
        Some(c) => c,
        None => return err(StatusCode::UNAUTHORIZED, "authentication required"),
    };
    let is_write = !matches!(req.method().as_str(), "GET" | "HEAD" | "OPTIONS");
    if is_write && session::is_read_only(&claims.role) {
        return err(StatusCode::FORBIDDEN, "viewers have read-only access");
    }
    req.extensions_mut().insert(claims);
    next.run(req).await
}

async fn health() -> Response {
    ok(json!({ "service": "switchboard", "version": env!("CARGO_PKG_VERSION") }))
}

/// Current live snapshot for initial console load before the WS stream kicks in.
async fn live_snapshot(State(state): State<AppState>) -> Response {
    ok(json!({ "devices": state.live.devices(), "aggregate": state.live.aggregate() }))
}

pub fn routes(state: AppState) -> Router {
    // Public endpoints (no session required).
    let public = Router::new()
        .route("/health", get(health))
        .route("/ws", get(crate::ws::ws_handler))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me));

    // Protected endpoints carry the write-guard. Device/config/rule routes join
    // this group in later phases.
    let protected = Router::new()
        .route("/whoami", get(auth::whoami))
        .route("/live", get(live_snapshot))
        .route("/devices", get(devices::list).post(devices::create))
        .route("/devices/{id}", get(devices::get_one).put(devices::update).delete(devices::delete))
        .route("/devices/{id}/twin", post(devices::set_twin))
        .route("/fleets", get(fleets::list).post(fleets::create))
        .route("/fleets/{id}", delete(fleets::delete))
        .layer(axum::middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .nest("/api", public.merge(protected))
        .with_state(state)
}
