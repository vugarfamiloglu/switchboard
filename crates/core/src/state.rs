//! Shared application state handed to every handler.

use std::sync::Arc;

use crate::config::Config;
use crate::db::Db;
use crate::vault::Vault;
use crate::ws::Hub;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub cfg: Arc<Config>,
    pub vault: Vault,
    /// HMAC secret for signing session tokens.
    pub secret: Arc<String>,
    /// Live telemetry fan-out to connected consoles.
    pub hub: Hub,
}
