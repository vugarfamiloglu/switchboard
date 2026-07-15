//! Remote command handlers — enqueue a command to a device and list recent
//! command history. The simulator fulfils pending commands each tick.

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
    ok(st.db.list_commands(200))
}

#[derive(Deserialize)]
pub struct SendBody {
    pub name: String,
    #[serde(default)]
    pub args: String,
}

pub async fn send(State(st): State<AppState>, Path(id): Path<String>, Json(b): Json<SendBody>) -> Response {
    if !st.db.device_exists(&id) {
        return err(StatusCode::NOT_FOUND, "device not found");
    }
    if b.name.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "command name is required");
    }
    let cid = format!("cmd_{}", ulid::Ulid::new().to_string().to_lowercase());
    match st.db.insert_command(&cid, &id, b.name.trim(), &b.args, now()) {
        Ok(_) => ok(json!({ "id": cid, "status": "pending" })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
