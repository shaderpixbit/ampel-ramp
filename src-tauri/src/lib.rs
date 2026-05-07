use chrono::{Local, TimeZone};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{command, State};

// ── Constants ────────────────────────────────────────────────────────────────

const VALID_STATUSES: &[&str] = &["free", "pending", "closed"];
const MAX_CHAT_MESSAGES: usize = 100;

/// Section A: Tor 30–42 (13 ramps). Adjust here if numbering changes.
const SECTION_A: &[u32] = &[42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30];
/// Section B: Tor 43–57 (15 ramps). Adjust here if numbering changes.
const SECTION_B: &[u32] = &[57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43];

// ── Shared DB state ──────────────────────────────────────────────────────────

pub struct DbState(pub Mutex<Connection>);

// ── Data types ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Ramp {
    id: u32,
    name: String,
    status: String,
    last_updated_by: String,
    last_updated_at: Option<String>,
    #[serde(default)]
    locked_until: Option<i64>,
    #[serde(default)]
    kennzeichen: Option<String>,
    #[serde(default)]
    notiz: Option<String>,
    #[serde(default)]
    reserviert_fuer: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ChatMessage {
    id: String,
    user: String,
    text: String,
    timestamp: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct RampEvent {
    timestamp: String,
    ramp_id: u32,
    from_status: String,
    to_status: String,
    user: String,
    kennzeichen: Option<String>,
    #[serde(default)]
    reserviert_fuer: Option<String>,
    duration_min: Option<i64>,
}

// ── DB path & connection ─────────────────────────────────────────────────────

fn get_db_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .or_else(|_| std::env::current_dir())
        .expect("Cannot determine path");
    path.pop();
    path.push("ampel.db");
    path
}

/// Opens the SQLite database, applies PRAGMAs, and creates the schema.
/// WAL mode + busy_timeout make it safe on SMB network drives with multiple clients.
pub fn open_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(get_db_path())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=5000;
         PRAGMA foreign_keys=ON;",
    )?;
    create_schema(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ramps (
            id              INTEGER PRIMARY KEY,
            name            TEXT    NOT NULL,
            status          TEXT    NOT NULL DEFAULT 'free',
            last_updated_by TEXT    NOT NULL DEFAULT '',
            last_updated_at TEXT,
            locked_until    INTEGER,
            kennzeichen     TEXT,
            notiz           TEXT,
            reserviert_fuer TEXT
        );

        CREATE TABLE IF NOT EXISTS chat_messages (
            id        TEXT PRIMARY KEY,
            user      TEXT NOT NULL,
            text      TEXT NOT NULL,
            timestamp TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ramp_events (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp       TEXT    NOT NULL,
            ramp_id         INTEGER NOT NULL,
            from_status     TEXT    NOT NULL,
            to_status       TEXT    NOT NULL,
            user            TEXT    NOT NULL,
            kennzeichen     TEXT,
            reserviert_fuer TEXT,
            duration_min    INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_events_ts ON ramp_events(timestamp);",
    )?;
    ensure_canonical_ramps(conn)?;
    Ok(())
}

// ── Ramp helpers ─────────────────────────────────────────────────────────────

/// Inserts any canonical ramps that are not yet in the DB (idempotent).
fn ensure_canonical_ramps(conn: &Connection) -> Result<(), rusqlite::Error> {
    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    for &id in SECTION_A.iter().chain(SECTION_B.iter()) {
        conn.execute(
            "INSERT OR IGNORE INTO ramps (id, name, status, last_updated_by, last_updated_at)
             VALUES (?1, ?2, 'free', 'System', ?3)",
            params![id, format!("Ramp {}", id), now],
        )?;
    }
    Ok(())
}

fn load_all_ramps(conn: &Connection) -> Result<Vec<Ramp>, rusqlite::Error> {
    let all_ids: Vec<u32> = SECTION_A.iter().chain(SECTION_B.iter()).copied().collect();
    let mut stmt = conn.prepare(
        "SELECT id, name, status, last_updated_by, last_updated_at,
                locked_until, kennzeichen, notiz, reserviert_fuer
         FROM ramps ORDER BY id",
    )?;
    let mut ramps: Vec<Ramp> = stmt
        .query_map([], |row| {
            Ok(Ramp {
                id: row.get(0)?,
                name: row.get(1)?,
                status: row.get(2)?,
                last_updated_by: row.get(3)?,
                last_updated_at: row.get(4)?,
                locked_until: row.get(5)?,
                kennzeichen: row.get(6)?,
                notiz: row.get(7)?,
                reserviert_fuer: row.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Sort to canonical order (Section A then B)
    ramps.sort_by_key(|r| all_ids.iter().position(|&id| id == r.id).unwrap_or(usize::MAX));
    Ok(ramps)
}

fn parse_ts_millis(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Local
            .from_local_datetime(&ndt)
            .single()
            .map(|dt| dt.timestamp_millis());
    }
    None
}

// ── Migration from legacy JSON files ─────────────────────────────────────────

/// Called once on startup. Reads the old JSON files if they exist and imports
/// their data into SQLite, then renames them to *.bak so the migration never
/// runs again but the originals are preserved as a safety copy.
pub fn migrate_from_json(conn: &Connection) {
    import_ramps_json(conn);
    import_chat_json(conn);
    import_events_json(conn);
}

fn base_dir() -> PathBuf {
    let mut p = std::env::current_exe()
        .or_else(|_| std::env::current_dir())
        .unwrap();
    p.pop();
    p
}

fn import_ramps_json(conn: &Connection) {
    let src = base_dir().join("ramps_state.json");
    if !src.exists() { return; }
    let Ok(text) = std::fs::read_to_string(&src) else { return };
    let Ok(ramps) = serde_json::from_str::<Vec<Ramp>>(&text) else { return };
    for r in ramps {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO ramps
             (id, name, status, last_updated_by, last_updated_at,
              locked_until, kennzeichen, notiz, reserviert_fuer)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                r.id, r.name, r.status, r.last_updated_by, r.last_updated_at,
                r.locked_until, r.kennzeichen, r.notiz, r.reserviert_fuer
            ],
        );
    }
    let _ = std::fs::rename(&src, base_dir().join("ramps_state.json.bak"));
    eprintln!("Migrated ramps_state.json → SQLite");
}

fn import_chat_json(conn: &Connection) {
    let src = base_dir().join("chat_messages.json");
    if !src.exists() { return; }
    let Ok(text) = std::fs::read_to_string(&src) else { return };
    let Ok(msgs) = serde_json::from_str::<Vec<ChatMessage>>(&text) else { return };
    for m in msgs {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO chat_messages (id, user, text, timestamp)
             VALUES (?1,?2,?3,?4)",
            params![m.id, m.user, m.text, m.timestamp],
        );
    }
    let _ = std::fs::rename(&src, base_dir().join("chat_messages.json.bak"));
    eprintln!("Migrated chat_messages.json → SQLite");
}

fn import_events_json(conn: &Connection) {
    // Import all ramp_events_YYYY-MM.json files found next to the executable
    let dir = base_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("ramp_events_") && name.ends_with(".json") {
            let path = entry.path();
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(events) = serde_json::from_str::<Vec<RampEvent>>(&text) else { continue };
            for e in events {
                let _ = conn.execute(
                    "INSERT INTO ramp_events
                     (timestamp, ramp_id, from_status, to_status, user,
                      kennzeichen, reserviert_fuer, duration_min)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        e.timestamp, e.ramp_id, e.from_status, e.to_status,
                        e.user, e.kennzeichen, e.reserviert_fuer, e.duration_min
                    ],
                );
            }
            let bak = path.with_extension("json.bak");
            let _ = std::fs::rename(&path, &bak);
            eprintln!("Migrated {} → SQLite", name);
        }
    }
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[command]
fn get_current_user() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "Unknown User".to_string())
}

#[command]
fn get_ramps(state: State<DbState>) -> Result<Vec<Ramp>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let now_ms = Local::now().timestamp_millis();

    // Clear expired locks
    db.execute(
        "UPDATE ramps SET locked_until = NULL WHERE locked_until IS NOT NULL AND locked_until <= ?1",
        params![now_ms],
    )
    .map_err(|e| e.to_string())?;

    load_all_ramps(&db).map_err(|e| e.to_string())
}

#[command]
fn update_ramp(state: State<DbState>, updated_ramp: Ramp) -> Result<Vec<Ramp>, String> {
    if !VALID_STATUSES.contains(&updated_ramp.status.as_str()) {
        return Err(format!(
            "Invalid status '{}': must be one of {:?}",
            updated_ramp.status, VALID_STATUSES
        ));
    }

    let db = state.0.lock().map_err(|e| e.to_string())?;

    // Read old status to detect changes and calculate dwell duration
    let old: Option<(String, Option<String>)> = db
        .query_row(
            "SELECT status, last_updated_at FROM ramps WHERE id = ?1",
            params![updated_ramp.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    let status_changed = old.as_ref().map(|(s, _)| s != &updated_ramp.status).unwrap_or(false);

    if status_changed {
        let duration_min = old
            .as_ref()
            .and_then(|(_, ts)| ts.as_deref())
            .and_then(parse_ts_millis)
            .map(|old_ms| (Local::now().timestamp_millis() - old_ms) / 60_000);

        db.execute(
            "INSERT INTO ramp_events
             (timestamp, ramp_id, from_status, to_status, user,
              kennzeichen, reserviert_fuer, duration_min)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                updated_ramp.id,
                old.as_ref().map(|(s, _)| s.as_str()).unwrap_or("free"),
                updated_ramp.status,
                updated_ramp.last_updated_by,
                updated_ramp.kennzeichen,
                updated_ramp.reserviert_fuer,
                duration_min,
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    let locked_until: Option<i64> = if status_changed {
        Some(Local::now().timestamp_millis() + 2000)
    } else {
        updated_ramp.locked_until
    };

    db.execute(
        "UPDATE ramps SET
            status          = ?1,
            last_updated_by = ?2,
            last_updated_at = ?3,
            locked_until    = ?4,
            kennzeichen     = ?5,
            notiz           = ?6,
            reserviert_fuer = ?7
         WHERE id = ?8",
        params![
            updated_ramp.status,
            updated_ramp.last_updated_by,
            updated_ramp.last_updated_at,
            locked_until,
            updated_ramp.kennzeichen,
            updated_ramp.notiz,
            updated_ramp.reserviert_fuer,
            updated_ramp.id,
        ],
    )
    .map_err(|e| e.to_string())?;

    load_all_ramps(&db).map_err(|e| e.to_string())
}

#[command]
fn get_messages(state: State<DbState>) -> Result<Vec<ChatMessage>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(
            "SELECT id, user, text, timestamp FROM chat_messages
             ORDER BY timestamp DESC LIMIT 50",
        )
        .map_err(|e| e.to_string())?;
    let mut msgs: Vec<ChatMessage> = stmt
        .query_map([], |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                user: row.get(1)?,
                text: row.get(2)?,
                timestamp: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    msgs.reverse(); // oldest first for display
    Ok(msgs)
}

#[command]
fn send_message(state: State<DbState>, user: String, text: String) -> Result<Vec<ChatMessage>, String> {
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err("Message cannot be empty".to_string());
    }

    let db = state.0.lock().map_err(|e| e.to_string())?;
    let id = Local::now().timestamp_millis().to_string();
    let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    db.execute(
        "INSERT INTO chat_messages (id, user, text, timestamp) VALUES (?1,?2,?3,?4)",
        params![id, user, trimmed, timestamp],
    )
    .map_err(|e| e.to_string())?;

    // Trim to MAX_CHAT_MESSAGES
    db.execute(
        "DELETE FROM chat_messages WHERE id NOT IN (
             SELECT id FROM chat_messages ORDER BY timestamp DESC LIMIT ?1
         )",
        params![MAX_CHAT_MESSAGES],
    )
    .map_err(|e| e.to_string())?;

    // Return last 50, oldest first — replicate get_messages logic without re-locking
    let mut stmt = db
        .prepare("SELECT id, user, text, timestamp FROM chat_messages ORDER BY timestamp DESC LIMIT 50")
        .map_err(|e| e.to_string())?;
    let mut msgs: Vec<ChatMessage> = stmt
        .query_map([], |row| Ok(ChatMessage {
            id: row.get(0)?, user: row.get(1)?, text: row.get(2)?, timestamp: row.get(3)?,
        }))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    msgs.reverse();
    Ok(msgs)
}

#[command]
fn get_daily_log(state: State<DbState>) -> Result<Vec<RampEvent>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let today = Local::now().format("%Y-%m-%d").to_string();
    query_events(&db, &today, &format!("{}T23:59:59", today))
}

#[command]
fn get_events_for_period(
    state: State<DbState>,
    from_date: String,
    to_date: String,
) -> Result<Vec<RampEvent>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    query_events(&db, &from_date, &format!("{}T23:59:59", to_date))
}

fn query_events(conn: &Connection, from: &str, to: &str) -> Result<Vec<RampEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT timestamp, ramp_id, from_status, to_status, user,
                    kennzeichen, reserviert_fuer, duration_min
             FROM ramp_events
             WHERE timestamp >= ?1 AND timestamp <= ?2
             ORDER BY timestamp ASC",
        )
        .map_err(|e| e.to_string())?;
    let events = stmt
        .query_map(params![from, to], |row| {
            Ok(RampEvent {
                timestamp: row.get(0)?,
                ramp_id: row.get(1)?,
                from_status: row.get(2)?,
                to_status: row.get(3)?,
                user: row.get(4)?,
                kennzeichen: row.get(5)?,
                reserviert_fuer: row.get(6)?,
                duration_min: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(events)
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = open_db().expect("Failed to open database");
    migrate_from_json(&conn);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(DbState(Mutex::new(conn)))
        .invoke_handler(tauri::generate_handler![
            get_current_user,
            get_ramps,
            update_ramp,
            get_messages,
            send_message,
            get_daily_log,
            get_events_for_period
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
