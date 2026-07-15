//! Runtime configuration, read once from the environment at boot.

const DEFAULT_PORT: u16 = 7930;

pub struct Config {
    pub port: u16,
    pub dist_dir: String,
    pub data_dir: String,
    /// Overrides the default sign-in passcode when set.
    pub passcode_override: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env_u16("SWITCHBOARD_PORT", DEFAULT_PORT),
            dist_dir: env_str("SWITCHBOARD_DIST", "crates/web/dist"),
            data_dir: env_str("SWITCHBOARD_DATA", "data"),
            passcode_override: std::env::var("SWITCHBOARD_PASSCODE").ok(),
        }
    }

    pub fn db_path(&self) -> String {
        format!("{}/switchboard.db", self.data_dir)
    }

    pub fn vault_key_path(&self) -> String {
        format!("{}/.vault-key", self.data_dir)
    }
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}
