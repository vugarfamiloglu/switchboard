//! Settings handlers — passcode rotation, an encrypted-at-rest snapshot backup,
//! and the alert-notification webhook endpoint.

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use super::{err, ok};
use crate::auth::{hash_passcode, verify_passcode};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct PasscodeBody {
    pub current: String,
    pub next: String,
}

pub async fn change_passcode(State(st): State<AppState>, Json(b): Json<PasscodeBody>) -> Response {
    let current = st.db.get_setting("passcode_hash").unwrap_or_default();
    if !verify_passcode(&current, b.current.trim()) {
        return err(StatusCode::UNAUTHORIZED, "current passcode is incorrect");
    }
    if b.next.trim().len() < 6 {
        return err(StatusCode::BAD_REQUEST, "the new passcode must be at least 6 characters");
    }
    match hash_passcode(b.next.trim()) {
        Ok(hash) => {
            let _ = st.db.set_setting("passcode_hash", &hash);
            ok(json!({ "changed": true }))
        }
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "could not hash passcode"),
    }
}

pub async fn backup(State(st): State<AppState>) -> Response {
    match st.db.snapshot(&st.cfg.data_dir) {
        Ok(bytes) => {
            let mut resp = bytes.into_response();
            let h = resp.headers_mut();
            h.insert(header::CONTENT_TYPE, "application/octet-stream".parse().unwrap());
            h.insert(header::CONTENT_DISPOSITION, "attachment; filename=\"switchboard-backup.db\"".parse().unwrap());
            resp
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

pub async fn get_webhook(State(st): State<AppState>) -> Response {
    ok(json!({ "url": st.db.get_setting("webhook_url").unwrap_or_default() }))
}

#[derive(Deserialize)]
pub struct WebhookBody {
    pub url: String,
}

pub async fn set_webhook(State(st): State<AppState>, Json(b): Json<WebhookBody>) -> Response {
    let _ = st.db.set_setting("webhook_url", b.url.trim());
    ok(json!({ "url": b.url.trim() }))
}
