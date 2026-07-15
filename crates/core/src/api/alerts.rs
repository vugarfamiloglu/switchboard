//! Alert handlers — list, acknowledge, resolve. Alerts are raised and cleared by
//! the simulator's rule evaluator; operators triage them here.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde_json::json;

use super::{err, ok};
use crate::auth::now;
use crate::state::AppState;

pub async fn list(State(st): State<AppState>) -> Response {
    ok(st.db.list_alerts())
}

pub async fn ack(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    transition(&st, &id, "acked")
}

pub async fn resolve(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    transition(&st, &id, "resolved")
}

fn transition(st: &AppState, id: &str, state: &str) -> Response {
    match st.db.set_alert_state(id, state, now()) {
        Ok(0) => err(StatusCode::NOT_FOUND, "alert not found"),
        Ok(_) => ok(json!({ "state": state })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
