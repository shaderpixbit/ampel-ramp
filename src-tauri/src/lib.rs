use chrono::{Local, TimeZone};
use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use tauri::command;

const VALID_STATUSES: &[&str] = &["free", "pending", "closed"];
const MAX_CHAT_MESSAGES: usize = 100;
const MAX_LOG_EVENTS: usize = 500;

const SECTION_A: &[u32] = &[42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30];
const SECTION_B: &[u32] = &[57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43];

#[derive(Serialize, Deserialize, Clone, Debug)]
struct RampEvent {
    timestamp: String,
    ramp_id: u32,
    from_status: String,
    to_status: String,
    user: String,
    kennzeichen: Option<String>,
    duration_min: Option<i64>,
    #[serde(default)]
    reserviert_fuer: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Ramp {
    id: u32,
    name: String,
    status: String,
    last_updated_by: String,
    last_updated_at: Option<String>,
    // Unix ms timestamp until which this ramp is locked for all clients
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

fn get_db_path() -> PathBuf {
    // Save state file right next to the executable on the network drive
    let mut path = std::env::current_exe()
        .or_else(|_| std::env::current_dir())
        .expect("Cannot determine executable or working directory path");
    path.pop(); // Remove executable name
    path.push("ramps_state.json");
    path
}

fn get_log_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .or_else(|_| std::env::current_dir())
        .expect("Cannot determine path");
    path.pop();
    path.push("ramp_events.json");
    path
}

fn get_chat_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .or_else(|_| std::env::current_dir())
        .expect("Cannot determine executable or working directory path");
    path.pop();
    path.push("chat_messages.json");
    path
}

/// Parses either RFC3339 ("2026-05-07T12:30:00.000Z") or local space format
/// ("2026-05-07 14:30:00") and returns Unix milliseconds.
fn parse_ts_millis(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Local.from_local_datetime(&ndt).single().map(|dt| dt.timestamp_millis());
    }
    None
}

fn append_event(event: RampEvent) {
    let path = get_log_path();
    let Ok(mut file) = open_state_file(&path) else { return };
    let mut contents = String::new();
    let _ = file.read_to_string(&mut contents);
    let mut events: Vec<RampEvent> = serde_json::from_str(&contents).unwrap_or_default();
    events.push(event);
    if events.len() > MAX_LOG_EVENTS {
        let drain = events.len() - MAX_LOG_EVENTS;
        events.drain(0..drain);
    }
    if let Ok(json) = serde_json::to_string_pretty(&events) {
        let _ = file.seek(SeekFrom::Start(0));
        let _ = file.write_all(json.as_bytes());
        let _ = file.set_len(json.len() as u64);
    }
    let _ = file.unlock();
}

fn make_default_ramp(id: u32, now: &str) -> Ramp {
    Ramp {
        id,
        name: format!("Ramp {}", id),
        status: "free".to_string(),
        last_updated_by: "System".to_string(),
        last_updated_at: Some(now.to_string()),
        locked_until: None,
        kennzeichen: None,
        notiz: None,
        reserviert_fuer: None,
    }
}

fn initialize_state() -> Vec<Ramp> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    SECTION_A
        .iter()
        .chain(SECTION_B.iter())
        .map(|&id| make_default_ramp(id, &now))
        .collect()
}

/// Ensures the ramp list is canonical: keeps existing ramps whose IDs are in
/// SECTION_A or SECTION_B, adds missing ones with default state, and sorts to
/// canonical order (A then B). Returns (ramps, dirty) where dirty=true means
/// at least one ramp was added or removed.
fn ensure_canonical(ramps: Vec<Ramp>) -> (Vec<Ramp>, bool) {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let all_ids: Vec<u32> = SECTION_A.iter().chain(SECTION_B.iter()).copied().collect();
    let mut dirty = false;

    // Remove ramps whose IDs are not in the canonical set
    let before_len = ramps.len();
    let mut canonical: Vec<Ramp> = ramps
        .into_iter()
        .filter(|r| all_ids.contains(&r.id))
        .collect();
    if canonical.len() != before_len {
        dirty = true;
    }

    // Add any missing ramps
    let existing_ids: Vec<u32> = canonical.iter().map(|r| r.id).collect();
    for &id in &all_ids {
        if !existing_ids.contains(&id) {
            canonical.push(make_default_ramp(id, &now));
            dirty = true;
        }
    }

    // Sort to canonical order
    canonical.sort_by_key(|r| {
        all_ids.iter().position(|&id| id == r.id).unwrap_or(usize::MAX)
    });

    (canonical, dirty)
}

/// Opens the state file and acquires an exclusive lock.
/// Logs a warning if locking fails (e.g. unsupported on some SMB mounts) but continues.
fn open_state_file(path: &PathBuf) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .map_err(|e| format!("Open error: {}", e))?;

    if let Err(e) = file.lock_exclusive() {
        eprintln!("Warning: could not acquire exclusive lock ({e}); proceeding without lock");
    }

    Ok(file)
}

/// Reads and parses the ramp state from an open file.
/// Returns (ramps, dirty) where dirty=true means the state was initialized/migrated
/// and must be written back to disk.
fn read_state(file: &mut File) -> Result<(Vec<Ramp>, bool), String> {
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Read error: {}", e))?;

    let parsed = if contents.trim().is_empty() {
        None
    } else {
        match serde_json::from_str::<Vec<Ramp>>(&contents) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("Warning: corrupted JSON ({e}), reinitializing state");
                None
            }
        }
    };

    let (ramps, dirty) = match parsed {
        None => (initialize_state(), true),
        Some(r) => ensure_canonical(r),
    };

    Ok((ramps, dirty))
}

/// Serializes ramps and writes them to the beginning of the file, truncating any leftover bytes.
fn write_state(file: &mut File, ramps: &[Ramp]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(ramps).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("Seek error: {}", e))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.set_len(json.len() as u64).map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
fn get_current_user() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "Unknown User".to_string())
}

#[command]
fn get_ramps() -> Result<Vec<Ramp>, String> {
    let path = get_db_path();
    let mut file = open_state_file(&path)?;
    let (mut ramps, mut dirty) = read_state(&mut file)?;

    // Clear any locks that have expired so all clients see the ramp become available
    let now_ms = Local::now().timestamp_millis();
    for ramp in ramps.iter_mut() {
        if ramp.locked_until.map(|ms| ms <= now_ms).unwrap_or(false) {
            ramp.locked_until = None;
            dirty = true;
        }
    }

    if dirty {
        write_state(&mut file, &ramps)?;
    }

    let _ = file.unlock();
    Ok(ramps)
}

#[command]
fn update_ramp(updated_ramp: Ramp) -> Result<Vec<Ramp>, String> {
    if !VALID_STATUSES.contains(&updated_ramp.status.as_str()) {
        return Err(format!(
            "Invalid status '{}': must be one of {:?}",
            updated_ramp.status, VALID_STATUSES
        ));
    }

    let path = get_db_path();
    let mut file = open_state_file(&path)?;
    let (mut ramps, _) = read_state(&mut file)?;

    if let Some(r) = ramps.iter_mut().find(|r| r.id == updated_ramp.id) {
        let status_changed = r.status != updated_ramp.status;
        if status_changed {
            let duration_min = r.last_updated_at.as_deref()
                .and_then(parse_ts_millis)
                .map(|old_ms| (Local::now().timestamp_millis() - old_ms) / 60_000);
            append_event(RampEvent {
                timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                ramp_id: updated_ramp.id,
                from_status: r.status.clone(),
                to_status: updated_ramp.status.clone(),
                user: updated_ramp.last_updated_by.clone(),
                kennzeichen: updated_ramp.kennzeichen.clone().or_else(|| r.kennzeichen.clone()),
                duration_min,
                reserviert_fuer: updated_ramp.reserviert_fuer.clone().or_else(|| r.reserviert_fuer.clone()),
            });
        }
        *r = updated_ramp;
        // Only lock on status changes — field edits (kennzeichen/notiz) need no lock
        if status_changed {
            r.locked_until = Some(Local::now().timestamp_millis() + 2000);
        }
    }

    write_state(&mut file, &ramps)?;
    let _ = file.unlock();

    Ok(ramps)
}

#[command]
fn get_daily_log() -> Result<Vec<RampEvent>, String> {
    let path = get_log_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let mut file = open_state_file(&path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Read error: {}", e))?;
    let _ = file.unlock();
    let today = Local::now().format("%Y-%m-%d").to_string();
    let events: Vec<RampEvent> = serde_json::from_str(&contents).unwrap_or_default();
    Ok(events.into_iter().filter(|e| e.timestamp.starts_with(&today)).collect())
}

#[command]
fn get_messages() -> Result<Vec<ChatMessage>, String> {
    let path = get_chat_path();
    let mut file = open_state_file(&path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Read error: {}", e))?;
    let _ = file.unlock();

    let messages: Vec<ChatMessage> = if contents.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&contents).unwrap_or_default()
    };

    let start = messages.len().saturating_sub(50);
    Ok(messages[start..].to_vec())
}

#[command]
fn send_message(user: String, text: String) -> Result<Vec<ChatMessage>, String> {
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err("Message cannot be empty".to_string());
    }

    let path = get_chat_path();
    let mut file = open_state_file(&path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Read error: {}", e))?;

    let mut messages: Vec<ChatMessage> = if contents.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&contents).unwrap_or_default()
    };

    let id = Local::now().timestamp_millis().to_string();
    let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    messages.push(ChatMessage {
        id,
        user,
        text: trimmed,
        timestamp,
    });

    if messages.len() > MAX_CHAT_MESSAGES {
        let drain_to = messages.len() - MAX_CHAT_MESSAGES;
        messages.drain(0..drain_to);
    }

    let json = serde_json::to_string_pretty(&messages).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("Seek error: {}", e))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.set_len(json.len() as u64).map_err(|e| e.to_string())?;
    let _ = file.unlock();

    let start = messages.len().saturating_sub(50);
    Ok(messages[start..].to_vec())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_current_user,
            get_ramps,
            update_ramp,
            get_messages,
            send_message,
            get_daily_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
