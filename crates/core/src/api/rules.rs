//! Alert-rule handlers — the telemetry engine evaluates enabled rules against
//! device metrics every cycle. Operators manage them here (write-guarded).

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
    ok(st.db.list_rules())
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    pub metric: String,
    #[serde(default = "default_op")]
    pub op: String,
    pub threshold: f64,
    #[serde(default = "default_sev")]
    pub severity: String,
}

fn default_op() -> String {
    "gt".into()
}
fn default_sev() -> String {
    "warning".into()
}

pub async fn create(State(st): State<AppState>, Json(b): Json<CreateBody>) -> Response {
    if b.name.trim().is_empty() || b.metric.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "name and metric are required");
    }
    let op = if b.op == "lt" { "lt" } else { "gt" };
    let sev = if b.severity == "critical" {
        "critical"
    } else {
        "warning"
    };
    let id = format!("rule_{}", ulid::Ulid::new().to_string().to_lowercase());
    match st.db.create_rule(
        &id,
        b.name.trim(),
        b.metric.trim(),
        op,
        b.threshold,
        sev,
        now(),
    ) {
        Ok(_) => ok(json!({ "id": id })),
        Err(e) => err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct ToggleBody {
    pub enabled: bool,
}

pub async fn toggle(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<ToggleBody>,
) -> Response {
    match st.db.set_rule_enabled(&id, b.enabled) {
        Ok(0) => err(StatusCode::NOT_FOUND, "rule not found"),
        Ok(_) => ok(json!({ "enabled": b.enabled })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    match st.db.delete_rule(&id) {
        Ok(0) => err(StatusCode::NOT_FOUND, "rule not found"),
        Ok(_) => ok(json!({ "deleted": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
