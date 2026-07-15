//! Device log handler — recent history for the live-tail view (new lines arrive
//! over the WebSocket stream as `log` frames).

use axum::extract::State;
use axum::response::Response;

use super::ok;
use crate::state::AppState;

pub async fn list(State(st): State<AppState>) -> Response {
    ok(st.db.list_logs(300))
}
