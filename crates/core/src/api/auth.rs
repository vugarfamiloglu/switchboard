//! Auth handlers — dual-mode sign-in (owner passcode or operator email/password),
//! session probe, and sign-out.

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::json;

use super::{claims_from_headers, clear_session_cookie, err, ok, set_session_cookie};
use crate::auth as session;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct LoginBody {
    #[serde(default)]
    pub passcode: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub password: String,
}

pub async fn login(State(st): State<AppState>, Json(body): Json<LoginBody>) -> Response {
    let (oid, role, name) = if !body.email.trim().is_empty() {
        // Operator sign-in (email + password).
        let op = match st.db.operator_by_email(body.email.trim()) {
            Some(o) => o,
            None => return err(StatusCode::UNAUTHORIZED, "invalid email or password"),
        };
        if !session::verify_passcode(&op.password_hash, &body.password) {
            return err(StatusCode::UNAUTHORIZED, "invalid email or password");
        }
        if op.status != "active" {
            return err(StatusCode::FORBIDDEN, "this operator is suspended");
        }
        (op.id, op.role, op.name)
    } else {
        // Owner sign-in (console passcode).
        let hash = st.db.get_setting("passcode_hash").unwrap_or_default();
        if !session::verify_passcode(&hash, body.passcode.trim()) {
            return err(StatusCode::UNAUTHORIZED, "invalid passcode");
        }
        (
            "owner".to_string(),
            "owner".to_string(),
            "Owner".to_string(),
        )
    };

    let token = match session::issue_token(&st.secret, &oid, &role, &name) {
        Ok(t) => t,
        Err(_) => {
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not create session",
            )
        }
    };
    let mut resp = ok(json!({ "authenticated": true, "role": role, "name": name }));
    resp.headers_mut().insert(
        header::SET_COOKIE,
        set_session_cookie(&token).parse().unwrap(),
    );
    resp
}

pub async fn logout() -> Response {
    let mut resp = ok(json!({ "authenticated": false }));
    resp.headers_mut()
        .insert(header::SET_COOKIE, clear_session_cookie().parse().unwrap());
    resp
}

pub async fn me(State(st): State<AppState>, headers: HeaderMap) -> Response {
    match claims_from_headers(&st.secret, &headers) {
        Some(c) => ok(json!({ "authenticated": true, "role": c.role, "name": c.name })),
        None => ok(json!({ "authenticated": false })),
    }
}

pub async fn whoami(Extension(claims): Extension<session::Claims>) -> Response {
    ok(json!({ "oid": claims.oid, "role": claims.role, "name": claims.name }))
}
