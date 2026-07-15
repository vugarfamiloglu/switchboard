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

use crate::models::{Device, DeviceDetail, Fleet};

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
CREATE TABLE IF NOT EXISTS fleets (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS devices (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    model        TEXT NOT NULL DEFAULT '',
    fw_version   TEXT NOT NULL DEFAULT '',
    fleet_id     TEXT REFERENCES fleets(id) ON DELETE SET NULL,
    status       TEXT NOT NULL DEFAULT 'provisioned',
    claim_code   TEXT,
    tags         TEXT NOT NULL DEFAULT '',
    desired      TEXT NOT NULL DEFAULT '{}',
    reported     TEXT NOT NULL DEFAULT '{}',
    twin_version INTEGER NOT NULL DEFAULT 0,
    last_seen    INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_devices_fleet ON devices(fleet_id);
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

    // ---- Fleets -------------------------------------------------------------

    pub fn list_fleets(&self) -> Vec<Fleet> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT f.id, f.name, f.description, f.created_at,
                        (SELECT COUNT(*) FROM devices d WHERE d.fleet_id = f.id)
                 FROM fleets f ORDER BY f.name",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(Fleet {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    created_at: r.get(3)?,
                    device_count: r.get(4)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<Fleet>>>()
        })
        .unwrap_or_default()
    }

    pub fn create_fleet(&self, id: &str, name: &str, description: &str, now: i64) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO fleets(id,name,description,created_at) VALUES(?1,?2,?3,?4)",
                (id, name, description, now),
            )
            .map(|_| ())
        })
    }

    pub fn delete_fleet(&self, id: &str) -> rusqlite::Result<usize> {
        self.with(|c| c.execute("DELETE FROM fleets WHERE id=?1", [id]))
    }

    // ---- Devices ------------------------------------------------------------

    pub fn list_devices(&self) -> Vec<Device> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT d.id,d.name,d.model,d.fw_version,d.fleet_id,f.name,d.status,d.tags,
                        d.twin_version,d.last_seen,d.created_at,d.updated_at
                 FROM devices d LEFT JOIN fleets f ON f.id = d.fleet_id
                 ORDER BY d.name",
            )?;
            let rows = stmt.query_map([], row_to_device)?;
            rows.collect::<rusqlite::Result<Vec<Device>>>()
        })
        .unwrap_or_default()
    }

    pub fn get_device(&self, id: &str) -> Option<DeviceDetail> {
        self.with(|c| {
            c.query_row(
                "SELECT d.id,d.name,d.model,d.fw_version,d.fleet_id,f.name,d.status,d.tags,
                        d.twin_version,d.last_seen,d.created_at,d.updated_at,
                        d.desired,d.reported,d.claim_code
                 FROM devices d LEFT JOIN fleets f ON f.id = d.fleet_id
                 WHERE d.id=?1",
                [id],
                |r| {
                    let device = row_to_device(r)?;
                    let desired: String = r.get(12)?;
                    let reported: String = r.get(13)?;
                    let claim_code: Option<String> = r.get(14)?;
                    Ok(DeviceDetail {
                        device,
                        desired: serde_json::from_str(&desired).unwrap_or_else(|_| serde_json::json!({})),
                        reported: serde_json::from_str(&reported).unwrap_or_else(|_| serde_json::json!({})),
                        claim_code,
                    })
                },
            )
        })
        .ok()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_device(
        &self,
        id: &str,
        name: &str,
        model: &str,
        fw: &str,
        fleet: Option<&str>,
        claim: &str,
        tags: &str,
        now: i64,
    ) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO devices(id,name,model,fw_version,fleet_id,status,claim_code,tags,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,'provisioned',?6,?7,?8,?8)",
                rusqlite::params![id, name, model, fw, fleet, claim, tags, now],
            )
            .map(|_| ())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_device(
        &self,
        id: &str,
        name: &str,
        model: &str,
        fleet: Option<&str>,
        status: &str,
        tags: &str,
        now: i64,
    ) -> rusqlite::Result<usize> {
        self.with(|c| {
            c.execute(
                "UPDATE devices SET name=?2,model=?3,fleet_id=?4,status=?5,tags=?6,updated_at=?7 WHERE id=?1",
                rusqlite::params![id, name, model, fleet, status, tags, now],
            )
        })
    }

    pub fn delete_device(&self, id: &str) -> rusqlite::Result<usize> {
        self.with(|c| c.execute("DELETE FROM devices WHERE id=?1", [id]))
    }

    pub fn set_twin_desired(&self, id: &str, desired_json: &str, now: i64) -> rusqlite::Result<usize> {
        self.with(|c| {
            c.execute(
                "UPDATE devices SET desired=?2, twin_version=twin_version+1, updated_at=?3 WHERE id=?1",
                rusqlite::params![id, desired_json, now],
            )
        })
    }

    /// Ingest touch: a device reported in — update its reported twin and
    /// last_seen, and mark it active (unless retired). Used by the ingest path.
    pub fn touch_device(&self, id: &str, reported_json: &str, now: i64) -> rusqlite::Result<usize> {
        self.with(|c| {
            c.execute(
                "UPDATE devices SET reported=?2, last_seen=?3,
                    status=CASE WHEN status='retired' THEN status ELSE 'active' END
                 WHERE id=?1",
                rusqlite::params![id, reported_json, now],
            )
        })
    }

    pub fn device_count(&self) -> i64 {
        self.with(|c| c.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)))
            .unwrap_or(0)
    }
}

fn row_to_device(r: &rusqlite::Row) -> rusqlite::Result<Device> {
    Ok(Device {
        id: r.get(0)?,
        name: r.get(1)?,
        model: r.get(2)?,
        fw_version: r.get(3)?,
        fleet_id: r.get(4)?,
        fleet_name: r.get(5)?,
        status: r.get(6)?,
        tags: r.get(7)?,
        twin_version: r.get(8)?,
        last_seen: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

/// The columns needed to authenticate an operator.
pub struct OperatorAuth {
    pub id: String,
    pub name: String,
    pub role: String,
    pub password_hash: String,
    pub status: String,
}
