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

use crate::models::{
    Alert, Command, ConfigProfile, Device, DeviceDetail, Firmware, Fleet, LogEntry, Operator,
    OtaCampaign,
};

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
CREATE TABLE IF NOT EXISTS alerts (
    id         TEXT PRIMARY KEY,
    device_id  TEXT,
    severity   TEXT NOT NULL DEFAULT 'warning',
    title      TEXT NOT NULL,
    detail     TEXT NOT NULL DEFAULT '',
    state      TEXT NOT NULL DEFAULT 'open',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_alerts_state ON alerts(state);
CREATE TABLE IF NOT EXISTS logs (
    id        TEXT PRIMARY KEY,
    device_id TEXT,
    ts        INTEGER NOT NULL,
    level     TEXT NOT NULL DEFAULT 'info',
    msg       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_logs_ts ON logs(ts);
CREATE TABLE IF NOT EXISTS commands (
    id         TEXT PRIMARY KEY,
    device_id  TEXT NOT NULL,
    name       TEXT NOT NULL,
    args       TEXT NOT NULL DEFAULT '',
    status     TEXT NOT NULL DEFAULT 'pending',
    result     TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_commands_device ON commands(device_id);
CREATE TABLE IF NOT EXISTS config_profiles (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    body       TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS firmware (
    id         TEXT PRIMARY KEY,
    model      TEXT NOT NULL,
    version    TEXT NOT NULL,
    size_kb    INTEGER NOT NULL DEFAULT 0,
    sha256     TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS ota_campaigns (
    id           TEXT PRIMARY KEY,
    firmware_id  TEXT NOT NULL,
    fleet_id     TEXT,
    canary_pct   INTEGER NOT NULL DEFAULT 100,
    status       TEXT NOT NULL DEFAULT 'running',
    total        INTEGER NOT NULL DEFAULT 0,
    updated      INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
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

    /// A clean SQLite snapshot (VACUUM INTO) returned as bytes, for backup.
    pub fn snapshot(&self, data_dir: &str) -> Result<Vec<u8>, String> {
        let tmp = format!("{}/backup-{}.db", data_dir, ulid::Ulid::new());
        let escaped = tmp.replace('\'', "''");
        self.with(|c| c.execute_batch(&format!("VACUUM INTO '{escaped}'")))
            .map_err(|e| e.to_string())?;
        let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
        Ok(bytes)
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

    pub fn list_operators(&self) -> Vec<Operator> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id,name,email,role,status,created_at FROM operators
                 ORDER BY CASE role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 WHEN 'operator' THEN 2 ELSE 3 END, name",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(Operator {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    email: r.get(2)?,
                    role: r.get(3)?,
                    status: r.get(4)?,
                    created_at: r.get(5)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<Operator>>>()
        })
        .unwrap_or_default()
    }

    pub fn update_operator(&self, id: &str, name: &str, email: &str, role: &str, status: &str, now: i64) -> rusqlite::Result<usize> {
        self.with(|c| {
            c.execute(
                "UPDATE operators SET name=?2,email=?3,role=?4,status=?5,updated_at=?6 WHERE id=?1",
                rusqlite::params![id, name, email, role, status, now],
            )
        })
    }

    pub fn update_operator_password(&self, id: &str, hash: &str, now: i64) -> rusqlite::Result<usize> {
        self.with(|c| c.execute("UPDATE operators SET password_hash=?2,updated_at=?3 WHERE id=?1", rusqlite::params![id, hash, now]))
    }

    pub fn delete_operator(&self, id: &str) -> rusqlite::Result<usize> {
        self.with(|c| c.execute("DELETE FROM operators WHERE id=?1", [id]))
    }

    pub fn operator_role(&self, id: &str) -> Option<String> {
        self.with(|c| c.query_row("SELECT role FROM operators WHERE id=?1", [id], |r| r.get(0))).ok()
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

    // ---- Alerts -------------------------------------------------------------

    pub fn list_alerts(&self) -> Vec<Alert> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT a.id,a.device_id,d.name,a.severity,a.title,a.detail,a.state,a.created_at
                 FROM alerts a LEFT JOIN devices d ON d.id = a.device_id
                 ORDER BY CASE a.state WHEN 'open' THEN 0 WHEN 'acked' THEN 1 ELSE 2 END, a.created_at DESC
                 LIMIT 300",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(Alert {
                    id: r.get(0)?,
                    device_id: r.get(1)?,
                    device_name: r.get(2)?,
                    severity: r.get(3)?,
                    title: r.get(4)?,
                    detail: r.get(5)?,
                    state: r.get(6)?,
                    created_at: r.get(7)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<Alert>>>()
        })
        .unwrap_or_default()
    }

    pub fn open_alert_exists(&self, device: &str, title: &str) -> bool {
        self.with(|c| {
            c.query_row(
                "SELECT 1 FROM alerts WHERE device_id=?1 AND title=?2 AND state IN ('open','acked') LIMIT 1",
                (device, title),
                |_| Ok(()),
            )
        })
        .is_ok()
    }

    pub fn insert_alert(&self, id: &str, device: &str, severity: &str, title: &str, detail: &str, now: i64) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO alerts(id,device_id,severity,title,detail,state,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,'open',?6,?6)",
                rusqlite::params![id, device, severity, title, detail, now],
            )
            .map(|_| ())
        })
    }

    /// Open/acked alerts for a device as (id, title) — used to resolve cleared ones.
    pub fn open_alerts_for(&self, device: &str) -> Vec<(String, String)> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT id,title FROM alerts WHERE device_id=?1 AND state IN ('open','acked')")?;
            let rows = stmt.query_map([device], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            rows.collect::<rusqlite::Result<Vec<(String, String)>>>()
        })
        .unwrap_or_default()
    }

    pub fn set_alert_state(&self, id: &str, state: &str, now: i64) -> rusqlite::Result<usize> {
        self.with(|c| c.execute("UPDATE alerts SET state=?2, updated_at=?3 WHERE id=?1", rusqlite::params![id, state, now]))
    }

    pub fn open_alert_count(&self) -> i64 {
        self.with(|c| c.query_row("SELECT COUNT(*) FROM alerts WHERE state IN ('open','acked')", [], |r| r.get(0)))
            .unwrap_or(0)
    }

    // ---- Logs ---------------------------------------------------------------

    pub fn insert_log(&self, id: &str, device: &str, ts: i64, level: &str, msg: &str) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO logs(id,device_id,ts,level,msg) VALUES(?1,?2,?3,?4,?5)",
                rusqlite::params![id, device, ts, level, msg],
            )
            .map(|_| ())
        })
    }

    pub fn list_logs(&self, limit: i64) -> Vec<LogEntry> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT l.id,l.device_id,d.name,l.ts,l.level,l.msg
                 FROM logs l LEFT JOIN devices d ON d.id = l.device_id
                 ORDER BY l.ts DESC, l.id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit], |r| {
                Ok(LogEntry {
                    id: r.get(0)?,
                    device_id: r.get(1)?,
                    device_name: r.get(2)?,
                    ts: r.get(3)?,
                    level: r.get(4)?,
                    msg: r.get(5)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<LogEntry>>>()
        })
        .unwrap_or_default()
    }

    /// Keep only the most recent `keep` log rows.
    pub fn prune_logs(&self, keep: i64) {
        let _ = self.with(|c| {
            c.execute(
                "DELETE FROM logs WHERE id NOT IN (SELECT id FROM logs ORDER BY ts DESC, id DESC LIMIT ?1)",
                [keep],
            )
        });
    }

    // ---- Commands -----------------------------------------------------------

    pub fn insert_command(&self, id: &str, device: &str, name: &str, args: &str, now: i64) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO commands(id,device_id,name,args,status,result,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,'pending','',?5,?5)",
                rusqlite::params![id, device, name, args, now],
            )
            .map(|_| ())
        })
    }

    pub fn list_commands(&self, limit: i64) -> Vec<Command> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT c.id,c.device_id,d.name,c.name,c.args,c.status,c.result,c.created_at
                 FROM commands c LEFT JOIN devices d ON d.id = c.device_id
                 ORDER BY c.created_at DESC, c.id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit], row_to_command)?;
            rows.collect::<rusqlite::Result<Vec<Command>>>()
        })
        .unwrap_or_default()
    }

    /// Pending commands as (id, device_id, name) for the sim to fulfil.
    pub fn list_pending_commands(&self) -> Vec<(String, String, String)> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT id,device_id,name FROM commands WHERE status='pending'")?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<(String, String, String)>>>()
        })
        .unwrap_or_default()
    }

    pub fn complete_command(&self, id: &str, status: &str, result: &str, now: i64) -> rusqlite::Result<usize> {
        self.with(|c| {
            c.execute(
                "UPDATE commands SET status=?2, result=?3, updated_at=?4 WHERE id=?1",
                rusqlite::params![id, status, result, now],
            )
        })
    }

    pub fn device_exists(&self, id: &str) -> bool {
        self.with(|c| c.query_row("SELECT 1 FROM devices WHERE id=?1", [id], |_| Ok(())))
            .is_ok()
    }

    // ---- Config profiles ----------------------------------------------------

    pub fn list_config_profiles(&self) -> Vec<ConfigProfile> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT id,name,body,created_at FROM config_profiles ORDER BY name")?;
            let rows = stmt.query_map([], |r| {
                let values: String = r.get(2)?;
                Ok(ConfigProfile {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    values: serde_json::from_str(&values).unwrap_or_else(|_| serde_json::json!({})),
                    created_at: r.get(3)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<ConfigProfile>>>()
        })
        .unwrap_or_default()
    }

    pub fn create_config_profile(&self, id: &str, name: &str, values: &str, now: i64) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute("INSERT INTO config_profiles(id,name,body,created_at) VALUES(?1,?2,?3,?4)", rusqlite::params![id, name, values, now]).map(|_| ())
        })
    }

    pub fn delete_config_profile(&self, id: &str) -> rusqlite::Result<usize> {
        self.with(|c| c.execute("DELETE FROM config_profiles WHERE id=?1", [id]))
    }

    pub fn config_profile_values(&self, id: &str) -> Option<String> {
        self.with(|c| c.query_row("SELECT body FROM config_profiles WHERE id=?1", [id], |r| r.get(0))).ok()
    }

    // ---- Firmware + OTA -----------------------------------------------------

    pub fn list_firmware(&self) -> Vec<Firmware> {
        self.with(|c| {
            let mut stmt = c.prepare("SELECT id,model,version,size_kb,sha256,created_at FROM firmware ORDER BY created_at DESC")?;
            let rows = stmt.query_map([], |r| {
                Ok(Firmware { id: r.get(0)?, model: r.get(1)?, version: r.get(2)?, size_kb: r.get(3)?, sha256: r.get(4)?, created_at: r.get(5)? })
            })?;
            rows.collect::<rusqlite::Result<Vec<Firmware>>>()
        })
        .unwrap_or_default()
    }

    pub fn create_firmware(&self, id: &str, model: &str, version: &str, size_kb: i64, sha256: &str, now: i64) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute("INSERT INTO firmware(id,model,version,size_kb,sha256,created_at) VALUES(?1,?2,?3,?4,?5,?6)", rusqlite::params![id, model, version, size_kb, sha256, now]).map(|_| ())
        })
    }

    pub fn delete_firmware(&self, id: &str) -> rusqlite::Result<usize> {
        self.with(|c| c.execute("DELETE FROM firmware WHERE id=?1", [id]))
    }

    pub fn list_campaigns(&self) -> Vec<OtaCampaign> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT o.id,o.firmware_id, f.model||' '||f.version, o.fleet_id, fl.name,
                        o.canary_pct,o.status,o.total,o.updated,o.created_at
                 FROM ota_campaigns o
                 LEFT JOIN firmware f ON f.id = o.firmware_id
                 LEFT JOIN fleets fl ON fl.id = o.fleet_id
                 ORDER BY o.created_at DESC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(OtaCampaign {
                    id: r.get(0)?,
                    firmware_id: r.get(1)?,
                    firmware_label: r.get(2)?,
                    fleet_id: r.get(3)?,
                    fleet_name: r.get(4)?,
                    canary_pct: r.get(5)?,
                    status: r.get(6)?,
                    total: r.get(7)?,
                    updated: r.get(8)?,
                    created_at: r.get(9)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<OtaCampaign>>>()
        })
        .unwrap_or_default()
    }

    pub fn create_campaign(&self, id: &str, firmware: &str, fleet: Option<&str>, canary: i64, total: i64, now: i64) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO ota_campaigns(id,firmware_id,fleet_id,canary_pct,status,total,updated,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,'running',?5,0,?6,?6)",
                rusqlite::params![id, firmware, fleet, canary, total, now],
            )
            .map(|_| ())
        })
    }

    /// Advance running OTA campaigns one device at a time, completing when done.
    pub fn advance_campaigns(&self, now: i64) {
        let _ = self.with(|c| c.execute("UPDATE ota_campaigns SET updated=MIN(total, updated+1), updated_at=?1 WHERE status='running' AND updated<total", [now]));
        let _ = self.with(|c| c.execute("UPDATE ota_campaigns SET status='completed', updated_at=?1 WHERE status='running' AND updated>=total AND total>0", [now]));
    }
}

fn row_to_command(r: &rusqlite::Row) -> rusqlite::Result<Command> {
    Ok(Command {
        id: r.get(0)?,
        device_id: r.get(1)?,
        device_name: r.get(2)?,
        name: r.get(3)?,
        args: r.get(4)?,
        status: r.get(5)?,
        result: r.get(6)?,
        created_at: r.get(7)?,
    })
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
