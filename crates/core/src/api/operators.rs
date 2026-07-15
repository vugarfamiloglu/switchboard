//! Team (operator) handlers — list, invite, edit, remove. Only owners and admins
//! may manage the team; the write-guard already blocks read-only viewers.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::json;

use super::{err, ok};
use crate::auth::{hash_passcode, now, Claims};
use crate::state::AppState;

fn can_manage(claims: &Claims) -> bool {
    claims.role == "owner" || claims.role == "admin"
}

fn role_or_default(r: &str) -> &str {
    match r {
        "owner" | "admin" | "operator" | "viewer" => r,
        _ => "operator",
    }
}

pub async fn list(State(st): State<AppState>) -> Response {
    ok(st.db.list_operators())
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub role: String,
    pub password: String,
}

pub async fn create(State(st): State<AppState>, Extension(claims): Extension<Claims>, Json(b): Json<CreateBody>) -> Response {
    if !can_manage(&claims) {
        return err(StatusCode::FORBIDDEN, "only owners and admins can manage the team");
    }
    if b.name.trim().is_empty() || b.email.trim().is_empty() || b.password.len() < 6 {
        return err(StatusCode::BAD_REQUEST, "name, email, and a 6+ character password are required");
    }
    let hash = match hash_passcode(&b.password) {
        Ok(h) => h,
        Err(_) => return err(StatusCode::INTERNAL_SERVER_ERROR, "could not hash password"),
    };
    let id = format!("op_{}", ulid::Ulid::new().to_string().to_lowercase());
    match st.db.insert_operator(&id, b.name.trim(), &b.email.trim().to_lowercase(), role_or_default(&b.role), &hash, now()) {
        Ok(_) => ok(json!({ "id": id })),
        Err(_) => err(StatusCode::BAD_REQUEST, "could not create operator (email may already exist)"),
    }
}

#[derive(Deserialize)]
pub struct UpdateBody {
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub role: String,
    #[serde(default = "active")]
    pub status: String,
    #[serde(default)]
    pub password: String,
}

fn active() -> String {
    "active".into()
}

pub async fn update(State(st): State<AppState>, Extension(claims): Extension<Claims>, Path(id): Path<String>, Json(b): Json<UpdateBody>) -> Response {
    if !can_manage(&claims) {
        return err(StatusCode::FORBIDDEN, "only owners and admins can manage the team");
    }
    if b.password.len() >= 6 {
        if let Ok(h) = hash_passcode(&b.password) {
            let _ = st.db.update_operator_password(&id, &h, now());
        }
    }
    match st.db.update_operator(&id, b.name.trim(), &b.email.trim().to_lowercase(), role_or_default(&b.role), &b.status, now()) {
        Ok(0) => err(StatusCode::NOT_FOUND, "operator not found"),
        Ok(_) => ok(json!({ "updated": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete(State(st): State<AppState>, Extension(claims): Extension<Claims>, Path(id): Path<String>) -> Response {
    if !can_manage(&claims) {
        return err(StatusCode::FORBIDDEN, "only owners and admins can manage the team");
    }
    if st.db.operator_role(&id).as_deref() == Some("owner") {
        return err(StatusCode::BAD_REQUEST, "cannot remove an owner");
    }
    match st.db.delete_operator(&id) {
        Ok(0) => err(StatusCode::NOT_FOUND, "operator not found"),
        Ok(_) => ok(json!({ "deleted": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
