use chrono::Local;
use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use tauri::command;

const VALID_STATUSES: &[&str] = &["free", "pending", "closed"];
const MAX_CHAT_MESSAGES: usize = 10;

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

fn get_chat_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .or_else(|_| std::env::current_dir())
        .expect("Cannot determine executable or working directory path");
    path.pop();
    path.push("chat_messages.json");
    path
}

fn initialize_state() -> Vec<Ramp> {
    let mut ramps = Vec::new();
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    // Generate ramps from 42 down to 30
    for i in (30..=42).rev() {
        ramps.push(Ramp {
            id: i,
            name: format!("Ramp {}", i),
            status: "free".to_string(),
            last_updated_by: "System".to_string(),
            last_updated_at: Some(now.clone()),
            locked_until: None,
        });
    }
    ramps
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

    let (ramps, dirty) = if contents.trim().is_empty() {
        (initialize_state(), true)
    } else {
        match serde_json::from_str::<Vec<Ramp>>(&contents) {
            Ok(r) => (r, false),
            Err(e) => {
                eprintln!("Warning: corrupted JSON ({e}), reinitializing state");
                (initialize_state(), true)
            }
        }
    };

    // Auto-migrate to current ramp layout (42 down to 30, 13 ramps)
    if ramps.len() != 13 || ramps.first().map(|r| r.id) != Some(42) {
        eprintln!("Migrating ramp state to current layout");
        return Ok((initialize_state(), true));
    }

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

    let locked_until = Local::now().timestamp_millis() + 2000;
    if let Some(r) = ramps.iter_mut().find(|r| r.id == updated_ramp.id) {
        *r = updated_ramp;
        r.locked_until = Some(locked_until);
    }

    write_state(&mut file, &ramps)?;
    let _ = file.unlock();

    Ok(ramps)
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
            send_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
