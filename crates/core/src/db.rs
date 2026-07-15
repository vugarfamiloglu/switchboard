//! SQLite store — WAL mode, a single serialized connection behind a mutex.
//! Structural data (operators, devices, config, rules…) lives here; live
//! telemetry stays in memory (see the sim engine).
//!
//! DISCIPLINE: the connection is a single non-reentrant mutex, exactly like
//! Cipherlane's single-connection pool. Every helper locks, runs, and releases.
//! NEVER call another `Db` method (or re-enter `with`) while inside a `with`
//! closure — that self-deadlocks the mutex. Collect rows first, then act.

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS operators (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    email         TEXT NOT NULL UNIQUE,
    role          TEXT NOT NULL DEFAULT 'operator',
    password_hash TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'active',
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
";

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        if let Some(dir) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;",
        )?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Borrow the locked connection for one short unit of work. Keep it
    /// synchronous and self-contained — see the module discipline note.
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> rusqlite::Result<T> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        f(&conn)
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.with(|c| c.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0)))
            .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO settings(key,value) VALUES(?1,?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                (key, value),
            )
            .map(|_| ())
        })
    }

    pub fn operator_count(&self) -> i64 {
        self.with(|c| c.query_row("SELECT COUNT(*) FROM operators", [], |r| r.get(0)))
            .unwrap_or(0)
    }

    pub fn operator_by_email(&self, email: &str) -> Option<OperatorAuth> {
        self.with(|c| {
            c.query_row(
                "SELECT id,name,role,password_hash,status FROM operators WHERE email=?1",
                [email],
                |r| {
                    Ok(OperatorAuth {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        role: r.get(2)?,
                        password_hash: r.get(3)?,
                        status: r.get(4)?,
                    })
                },
            )
        })
        .ok()
    }

    pub fn insert_operator(
        &self,
        id: &str,
        name: &str,
        email: &str,
        role: &str,
        password_hash: &str,
        now: i64,
    ) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO operators(id,name,email,role,password_hash,status,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,'active',?6,?6)",
                (id, name, email, role, password_hash, now),
            )
            .map(|_| ())
        })
    }
}

/// The columns needed to authenticate an operator.
pub struct OperatorAuth {
    pub id: String,
    pub name: String,
    pub role: String,
    pub password_hash: String,
    pub status: String,
}
