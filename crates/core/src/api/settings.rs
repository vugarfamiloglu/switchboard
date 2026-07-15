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
        return err(
            StatusCode::BAD_REQUEST,
            "the new passcode must be at least 6 characters",
        );
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
            h.insert(
                header::CONTENT_TYPE,
                "application/octet-stream".parse().unwrap(),
            );
            h.insert(
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"switchboard-backup.db\""
                    .parse()
                    .unwrap(),
            );
            resp
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

pub async fn export_devices(State(st): State<AppState>) -> Response {
    let mut csv = String::from("id,name,model,fleet,status,fwVersion,lastSeen\n");
    for d in st.db.list_devices() {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            d.id,
            csv_field(&d.name),
            csv_field(&d.model),
            csv_field(&d.fleet_name.unwrap_or_default()),
            d.status,
            d.fw_version,
            d.last_seen
        ));
    }
    let mut resp = csv.into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        "text/csv; charset=utf-8".parse().unwrap(),
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"switchboard-devices.csv\""
            .parse()
            .unwrap(),
    );
    resp
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn get_webhook(State(st): State<AppState>) -> Response {
    // The webhook URL is sealed at rest in the vault.
    let sealed = st.db.get_setting("webhook_url").unwrap_or_default();
    let url = if sealed.is_empty() {
        String::new()
    } else {
        st.vault.open(&sealed).unwrap_or_default()
    };
    ok(json!({ "url": url }))
}

#[derive(Deserialize)]
pub struct WebhookBody {
    pub url: String,
}

pub async fn set_webhook(State(st): State<AppState>, Json(b): Json<WebhookBody>) -> Response {
    let trimmed = b.url.trim();
    let stored = if trimmed.is_empty() {
        String::new()
    } else {
        st.vault.seal(trimmed).unwrap_or_default()
    };
    let _ = st.db.set_setting("webhook_url", &stored);
    ok(json!({ "url": trimmed }))
}
