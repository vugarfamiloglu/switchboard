//! Fleet (device group) handlers — list, create, delete.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use super::{err, ok};
use crate::auth::now;
use crate::state::AppState;

pub async fn list(State(st): State<AppState>) -> Response {
    ok(st.db.list_fleets())
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

pub async fn create(State(st): State<AppState>, Json(b): Json<CreateBody>) -> Response {
    if b.name.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "name is required");
    }
    let id = format!("flt_{}", ulid::Ulid::new().to_string().to_lowercase());
    match st
        .db
        .create_fleet(&id, b.name.trim(), &b.description, now())
    {
        Ok(_) => ok(json!({ "id": id })),
        Err(e) => err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn delete(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    match st.db.delete_fleet(&id) {
        Ok(0) => err(StatusCode::NOT_FOUND, "fleet not found"),
        Ok(_) => ok(json!({ "deleted": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
