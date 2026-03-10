use rusqlite::{Connection, Result, params};
use std::fs;
use std::path::PathBuf;
use crate::events::models::{EventLog, SyncEvent};

pub mod browser_scanner;
pub mod postgres;
pub mod router;

pub fn init_db(app_dir: PathBuf) -> Result<Connection> {
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).expect("Failed to create app data directory");
    }
    
    let db_path = app_dir.join("bookmarks.db");
    let conn = Connection::open(db_path)?;
    
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    create_tables(&conn)?;
    
    Ok(conn)
}

pub fn apply_event(conn: &Connection, log: &EventLog) -> Result<(), String> {
    match &log.event {
        SyncEvent::BookmarkAdded(b) => {
            conn.execute(
                "INSERT INTO bookmarks (id, url, canonical_url, title, description, favicon_url, host, created_at)
                 VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(canonical_url) DO UPDATE SET
                 title = excluded.title, is_deleted = 0, updated_at = CURRENT_TIMESTAMP",
                params![b.id, b.url, b.title, b.description, b.favicon_url, b.host, b.created_at]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::FolderAdded { id, parent_id, name } => {
            conn.execute(
                "INSERT INTO folders (id, parent_id, name) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, parent_id = excluded.parent_id",
                params![id, parent_id, name]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::BookmarkDeleted { id } => {
            conn.execute(
                "UPDATE bookmarks SET is_deleted = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                params![id]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::BookmarkUpdated(b) => {
            conn.execute(
                "UPDATE bookmarks SET title = ?1, url = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3",
                params![b.title, b.url, b.id]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::TagAdded { id, name } => {
            conn.execute(
                "INSERT INTO tags (id, name) VALUES (?1, ?2) ON CONFLICT(name) DO NOTHING",
                params![id, name]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::BookmarkTagged { bookmark_id, tag_id } => {
            conn.execute(
                "INSERT INTO bookmark_tags (bookmark_id, tag_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                params![bookmark_id, tag_id]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::BookmarkUntagged { bookmark_id, tag_id } => {
            conn.execute(
                "DELETE FROM bookmark_tags WHERE bookmark_id = ?1 AND tag_id = ?2",
                params![bookmark_id, tag_id]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::FolderRenamed { id, name } => {
            conn.execute(
                "UPDATE folders SET name = ?2 WHERE id = ?1",
                params![id, name]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::FolderDeleted { id } => {
            conn.execute(
                "UPDATE folders SET is_deleted = 1 WHERE id = ?1",
                params![id]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::BookmarkAddedToFolder { bookmark_id, folder_id } => {
            conn.execute(
                "INSERT INTO folder_bookmarks (folder_id, bookmark_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                params![folder_id, bookmark_id]
            ).map_err(|e| e.to_string())?;
        },
        SyncEvent::BookmarkRemovedFromFolder { bookmark_id, folder_id } => {
            conn.execute(
                "DELETE FROM folder_bookmarks WHERE folder_id = ?1 AND bookmark_id = ?2",
                params![folder_id, bookmark_id]
            ).map_err(|e| e.to_string())?;
        },
        _ => {}
    }
    Ok(())
}

pub fn apply_event_if_new(conn: &Connection, log: &EventLog) -> Result<bool, String> {
    let inserted = conn
        .execute(
            "INSERT INTO applied_event_ids (event_id) VALUES (?1) ON CONFLICT(event_id) DO NOTHING",
            params![log.event_id],
        )
        .map_err(|e| e.to_string())?;
    if inserted == 0 {
        return Ok(false);
    }
    apply_event(conn, log)?;
    Ok(true)
}

fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS event_cursors (
            id INTEGER PRIMARY KEY,
            last_event_id TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS bookmarks (
            id TEXT PRIMARY KEY,
            url TEXT NOT NULL,
            canonical_url TEXT UNIQUE NOT NULL,
            title TEXT,
            description TEXT,
            favicon_url TEXT,
            host TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            is_deleted BOOLEAN DEFAULT 0
        );
        
        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        
        CREATE TABLE IF NOT EXISTS bookmark_tags (
            bookmark_id TEXT NOT NULL,
            tag_id TEXT NOT NULL,
            PRIMARY KEY (bookmark_id, tag_id),
            FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS folders (
            id TEXT PRIMARY KEY,
            parent_id TEXT,
            name TEXT NOT NULL,
            is_deleted BOOLEAN DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS folder_bookmarks (
            folder_id TEXT NOT NULL,
            bookmark_id TEXT NOT NULL,
            PRIMARY KEY (folder_id, bookmark_id),
            FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
            FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS applied_event_ids (
            event_id TEXT PRIMARY KEY
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS bookmarks_fts USING fts5(
            title,
            url,
            host,
            content='bookmarks',
            content_rowid='rowid'
        );

        -- FTS Triggers (Fixed)
        DROP TRIGGER IF EXISTS bookmarks_ai;
        CREATE TRIGGER bookmarks_ai AFTER INSERT ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(rowid, title, url, host)
            VALUES (new.rowid, new.title, new.url, new.host);
        END;

        DROP TRIGGER IF EXISTS bookmarks_ad;
        CREATE TRIGGER bookmarks_ad AFTER DELETE ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(bookmarks_fts, rowid, title, url, host)
            VALUES ('delete', old.rowid, old.title, old.url, old.host);
        END;

        DROP TRIGGER IF EXISTS bookmarks_au;
        CREATE TRIGGER bookmarks_au AFTER UPDATE ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(bookmarks_fts, rowid, title, url, host)
            VALUES ('delete', old.rowid, old.title, old.url, old.host);
            INSERT INTO bookmarks_fts(rowid, title, url, host)
            VALUES (new.rowid, new.title, new.url, new.host);
        END;
        "
    )?;
    ensure_folders_is_deleted_column(conn)?;
    Ok(())
}

fn ensure_folders_is_deleted_column(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(folders)")?;
    let mut rows = stmt.query([])?;
    let mut has_is_deleted = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "is_deleted" {
            has_is_deleted = true;
            break;
        }
    }
    if !has_is_deleted {
        conn.execute(
            "ALTER TABLE folders ADD COLUMN is_deleted BOOLEAN DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        create_tables(&conn).expect("create tables");
        conn
    }

    #[test]
    fn bookmark_untagged_should_remove_relation() {
        let mut conn = setup_conn();
        conn.execute(
            "INSERT INTO bookmarks (id, url, canonical_url, title, host, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["b1", "https://example.com", "https://example.com", "Example", "example.com", "2026-03-02T00:00:00Z"],
        )
        .expect("seed bookmark");

        let tag_event = EventLog {
            event_id: "e1".into(),
            device_id: "test".into(),
            timestamp: 1,
            event: SyncEvent::TagAdded {
                id: "t1".into(),
                name: "工作".into(),
            },
        };
        apply_event(&mut conn, &tag_event).expect("apply tag added");

        let tagged_event = EventLog {
            event_id: "e2".into(),
            device_id: "test".into(),
            timestamp: 2,
            event: SyncEvent::BookmarkTagged {
                bookmark_id: "b1".into(),
                tag_id: "t1".into(),
            },
        };
        apply_event(&mut conn, &tagged_event).expect("apply bookmark tagged");

        let untagged_event = EventLog {
            event_id: "e3".into(),
            device_id: "test".into(),
            timestamp: 3,
            event: SyncEvent::BookmarkUntagged {
                bookmark_id: "b1".into(),
                tag_id: "t1".into(),
            },
        };
        apply_event(&mut conn, &untagged_event).expect("apply bookmark untagged");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bookmark_tags WHERE bookmark_id = ?1 AND tag_id = ?2",
                params!["b1", "t1"],
                |r| r.get(0),
            )
            .expect("count relations");
        assert_eq!(count, 0);
    }

    #[test]
    fn folder_deleted_event_should_mark_folder_deleted() {
        let mut conn = setup_conn();
        conn.execute(
            "INSERT INTO folders (id, parent_id, name) VALUES (?1, ?2, ?3)",
            params!["f1", Option::<String>::None, "工作"],
        )
        .expect("seed folder");

        let deleted_event = EventLog {
            event_id: "e4".into(),
            device_id: "test".into(),
            timestamp: 4,
            event: SyncEvent::FolderDeleted { id: "f1".into() },
        };

        apply_event(&mut conn, &deleted_event).expect("apply folder deleted");

        let deleted: i64 = conn
            .query_row(
                "SELECT is_deleted FROM folders WHERE id = ?1",
                params!["f1"],
                |r| r.get(0),
            )
            .expect("query folder is_deleted");
        assert_eq!(deleted, 1);
    }
}
