use chrono::Local;
use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use tauri::command;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Ramp {
    id: u32,
    name: String,
    status: String,
    last_updated_by: String,
    last_updated_at: Option<String>,
}

fn get_db_path() -> PathBuf {
    // Save state file right next to the executable on the network drive
    let mut path = std::env::current_exe().unwrap_or_else(|_| std::env::current_dir().unwrap());
    path.pop(); // Remove executable name
    path.push("ramps_state.json");
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
        });
    }
    ramps
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

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .map_err(|e| format!("Open error: {}", e))?;

    // Lock exclusively (ignore error if unsupported on this specific SMB/Network drive)
    // We cannot use lock_shared() here because writing the initial empty file on Windows throws an error!
    let _ = file.lock_exclusive();

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Read error: {}", e))?;

    let mut ramps = if contents.trim().is_empty() {
        let initial = initialize_state();
        let json = serde_json::to_string_pretty(&initial).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Seek error: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
        file.set_len(json.len() as u64).map_err(|e| e.to_string())?;
        initial
    } else {
        // Fallback to initial if json is corrupted by sudden power loss
        serde_json::from_str(&contents).unwrap_or_else(|_| initialize_state())
    };

    // Auto-migrate the local database to the new mapping (42 down to 30)
    if ramps.len() != 13 || ramps.first().map(|r| r.id) != Some(42) {
        ramps = initialize_state();
        let json = serde_json::to_string_pretty(&ramps).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Seek error: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
        file.set_len(json.len() as u64).map_err(|e| e.to_string())?;
    }

    let _ = file.unlock();
    Ok(ramps)
}

#[command]
fn update_ramp(updated_ramp: Ramp) -> Result<Vec<Ramp>, String> {
    let path = get_db_path();

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .map_err(|e| format!("Open error: {}", e))?;

    // Lock exclusively for writing to prevent race conditions on network drives
    let _ = file.lock_exclusive();

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Read error: {}", e))?;

    let mut ramps: Vec<Ramp> = if contents.trim().is_empty() {
        initialize_state()
    } else {
        serde_json::from_str(&contents).unwrap_or_else(|_| initialize_state())
    };

    // Auto-migrate here as well just in case
    if ramps.len() != 13 || ramps.first().map(|r| r.id) != Some(42) {
        ramps = initialize_state();
    }

    // Apply update to the targeted ramp
    if let Some(r) = ramps.iter_mut().find(|r| r.id == updated_ramp.id) {
        *r = updated_ramp;
    }

    let json = serde_json::to_string_pretty(&ramps).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("Seek error: {}", e))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    file.set_len(json.len() as u64).map_err(|e| e.to_string())?;

    let _ = file.unlock();

    Ok(ramps)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_current_user,
            get_ramps,
            update_ramp
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
