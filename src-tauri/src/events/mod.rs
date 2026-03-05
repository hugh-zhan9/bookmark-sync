pub mod models;
pub mod cleaner;
pub mod metadata;

use rusqlite::Connection;
use models::EventLog;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub fn append_events_to_local_log(conn: &Connection, logs: &[EventLog]) -> Result<(), String> {
    if logs.is_empty() {
        return Ok(());
    }
    let db_path = conn
        .path()
        .ok_or_else(|| "database path unavailable".to_string())?;
    let app_dir = Path::new(db_path)
        .parent()
        .ok_or_else(|| "app dir unavailable".to_string())?;
    let log_path = app_dir.join("events.ndjson");
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| e.to_string())?;
    for log in logs {
        let line = serde_json::to_string(log).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn replay_events(conn: &mut Connection, logs: Vec<EventLog>) -> Result<(), String> {
    let mut applied = Vec::new();
    for log in logs {
        if crate::db::apply_event_if_new(conn, &log)? {
            applied.push(log);
        }
    }
    append_events_to_local_log(conn, &applied)?;
    Ok(())
}
