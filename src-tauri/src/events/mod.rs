pub mod models;
pub mod cleaner;
pub mod metadata;
pub mod segment;
pub mod device_registry;
pub mod cleanup;

use rusqlite::Connection;
use models::EventLog;
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
    // 迁移旧版单文件（向后兼容）
    segment::migrate_legacy_if_exists(app_dir)?;
    // 追加到当前 segment（超限自动封口）
    segment::append_to_current_segment(app_dir, logs)
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

