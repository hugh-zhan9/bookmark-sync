use rusqlite::{Connection, Result};
use std::fs;
use std::path::PathBuf;

pub fn init_db(app_dir: PathBuf) -> Result<Connection> {
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).expect("Failed to create app data directory");
    }
    
    let db_path = app_dir.join("bookmarks.db");
    let conn = Connection::open(db_path)?;
    
    // Enable foreign key support
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    
    create_tables(&conn)?;
    
    Ok(conn)
}

fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- Event Source Cursor table keeps track of synced logs
        CREATE TABLE IF NOT EXISTS event_cursors (
            id INTEGER PRIMARY KEY,
            last_event_id TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Core Bookmarks Table
        CREATE TABLE IF NOT EXISTS bookmarks (
            id TEXT PRIMARY KEY,
            url TEXT UNIQUE NOT NULL,
            canonical_url TEXT UNIQUE NOT NULL,
            title TEXT,
            description TEXT,
            favicon_url TEXT,
            host TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            is_deleted BOOLEAN DEFAULT 0
        );
        
        -- Tags
        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        
        -- Many-to-many relationship: Bookmarks <-> Tags
        CREATE TABLE IF NOT EXISTS bookmark_tags (
            bookmark_id TEXT NOT NULL,
            tag_id TEXT NOT NULL,
            PRIMARY KEY (bookmark_id, tag_id),
            FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );

        -- Folders / Tree Structure
        CREATE TABLE IF NOT EXISTS folders (
            id TEXT PRIMARY KEY,
            parent_id TEXT,
            name TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE CASCADE
        );

        -- One-to-many relationship: Folders -> Bookmarks
        -- Note: A bookmark can belong to one folder here, adjust if M-to-N is needed
        CREATE TABLE IF NOT EXISTS folder_bookmarks (
            folder_id TEXT NOT NULL,
            bookmark_id TEXT NOT NULL,
            PRIMARY KEY (folder_id, bookmark_id),
            FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
            FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE
        );

        -- FTS5 Virtual Table for full-text search
        CREATE VIRTUAL TABLE IF NOT EXISTS bookmarks_fts USING fts5(
            id UNINDEXED,
            title,
            description,
            url,
            host,
            content='bookmarks',
            content_rowid='rowid'
        );

        -- Triggers to keep FTS5 table in sync
        CREATE TRIGGER IF NOT EXISTS bookmarks_ai AFTER INSERT ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(rowid, id, title, description, url, host)
            VALUES (new.rowid, new.id, new.title, new.description, new.url, new.host);
        END;

        CREATE TRIGGER IF NOT EXISTS bookmarks_ad AFTER DELETE ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(bookmarks_fts, rowid, id, title, description, url, host)
            VALUES ('delete', old.rowid, old.id, old.title, old.description, old.url, old.host);
        END;

        CREATE TRIGGER IF NOT EXISTS bookmarks_au AFTER UPDATE ON bookmarks BEGIN
            INSERT INTO bookmarks_fts(bookmarks_fts, rowid, id, title, description, url, host)
            VALUES ('delete', old.rowid, old.id, old.title, old.description, old.url, old.host);
            INSERT INTO bookmarks_fts(rowid, id, title, description, url, host)
            VALUES (new.rowid, new.id, new.title, new.description, new.url, new.host);
        END;
        "
    )?;

    Ok(())
}
