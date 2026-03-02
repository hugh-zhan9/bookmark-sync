pub mod models;
pub mod native_messaging;
pub mod cleaner;
pub mod metadata;

use models::{EventLog, SyncEvent, BookmarkPayload};
use rusqlite::{Connection, Result, params};

pub fn replay_events(conn: &mut Connection, events: Vec<EventLog>) -> Result<()> {
    let tx = conn.transaction()?;

    for log in events {
        match log.event {
            SyncEvent::BookmarkAdded(payload) => insert_or_update_bookmark(&tx, &payload)?,
            SyncEvent::BookmarkDeleted { id } => mark_bookmark_deleted(&tx, &id)?,
            SyncEvent::BookmarkUpdated(payload) => insert_or_update_bookmark(&tx, &payload)?,
            
            // Skip folder/tag implementations for core structure clarity initially
            _ => println!("Event not handled yet: {:?}", log.event),
        }
    }

    tx.commit()?;
    Ok(())
}

fn insert_or_update_bookmark(conn: &Connection, payload: &BookmarkPayload) -> Result<()> {
    let canonical_url = payload.url.clone(); // In actual logic, implement query param stripping here
    
    conn.execute(
        "INSERT INTO bookmarks (id, url, canonical_url, title, description, favicon_url, host, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT(canonical_url) DO UPDATE SET 
            url = excluded.url,
            title = excluded.title,
            description = excluded.description,
            favicon_url = excluded.favicon_url,
            host = excluded.host,
            updated_at = excluded.updated_at,
            is_deleted = 0",
        params![
            payload.id,
            payload.url,
            canonical_url,
            payload.title,
            payload.description,
            payload.favicon_url,
            payload.host,
            payload.created_at,
        ],
    )?;
    Ok(())
}

fn mark_bookmark_deleted(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE bookmarks SET is_deleted = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}
