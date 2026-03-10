use rusqlite::{Connection, params_from_iter};
use std::sync::{Arc, Mutex};

use crate::db::store::BookmarkStore;
use crate::events::metadata::SiteMetadata;
use crate::events::models::{BookmarkPayload, EventLog};

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    fn with_conn<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Connection) -> Result<T, String>,
    {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        f(&conn)
    }
}

const BOOKMARK_SELECT_SQL: &str = "
    SELECT b.id, b.url, b.title, b.description, b.favicon_url, b.host, b.created_at,
    (SELECT GROUP_CONCAT(t.name) FROM tags t JOIN bookmark_tags bt ON t.id = bt.tag_id WHERE bt.bookmark_id = b.id) as tag_list
    FROM bookmarks b
";

fn map_bookmark_row(row: &rusqlite::Row) -> rusqlite::Result<BookmarkPayload> {
    let tag_list: Option<String> = row.get(7)?;
    let tags = tag_list.map(|list| {
        list.split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    });
    Ok(BookmarkPayload {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        favicon_url: row.get(4)?,
        host: row.get(5)?,
        created_at: row.get(6)?,
        tags,
    })
}

impl BookmarkStore for SqliteStore {
    fn get_bookmarks(&self) -> Result<Vec<BookmarkPayload>, String> {
        let sql = format!("{} WHERE b.is_deleted = 0 ORDER BY b.created_at DESC", BOOKMARK_SELECT_SQL);
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let iter = stmt.query_map([], map_bookmark_row).map_err(|e| e.to_string())?;
            let mut res = Vec::new();
            for b in iter { if let Ok(x) = b { res.push(x); } }
            Ok(res)
        })
    }

    fn search_bookmarks(&self, query: &str) -> Result<Vec<BookmarkPayload>, String> {
        let tokens = crate::tokenize_search_query(query);
        if tokens.is_empty() {
            return self.get_bookmarks();
        }
        let term_clauses: Vec<String> = tokens
            .iter()
            .enumerate()
            .map(|(idx, _)| crate::search_clause_for_param(idx))
            .collect();
        let sql = format!(
            "{} WHERE b.is_deleted = 0 AND {} ORDER BY b.created_at DESC",
            BOOKMARK_SELECT_SQL,
            term_clauses.join(" AND ")
        );
        let patterns: Vec<String> = tokens.iter().map(|t| format!("%{}%", t)).collect();
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let iter = stmt
                .query_map(params_from_iter(patterns.iter()), map_bookmark_row)
                .map_err(|e| e.to_string())?;
            let mut res = Vec::new();
            for b in iter { if let Ok(x) = b { res.push(x); } }
            Ok(res)
        })
    }

    fn apply_event(&self, log: &EventLog) -> Result<(), String> {
        self.with_conn(|conn| crate::db::apply_event(conn, log))
    }

    fn apply_event_if_new(&self, log: &EventLog) -> Result<bool, String> {
        self.with_conn(|conn| crate::db::apply_event_if_new(conn, log))
    }

    fn resolve_bookmark_id_for_url(&self, url: &str, fallback_id: &str) -> String {
        self.with_conn(|conn| Ok(crate::resolve_bookmark_id_for_url(conn, url, fallback_id)))
            .unwrap_or_else(|_| fallback_id.to_string())
    }

    fn apply_metadata_by_canonical_url(&self, canonical_url: &str, meta: &SiteMetadata) -> Result<usize, String> {
        self.with_conn(|conn| crate::apply_metadata_by_canonical_url(conn, canonical_url, meta))
    }

    fn is_bookmark_logically_deleted_by_canonical_url(&self, canonical_url: &str) -> Result<bool, String> {
        self.with_conn(|conn| crate::is_bookmark_logically_deleted_by_canonical_url(conn, canonical_url))
    }

    fn is_folder_logically_deleted_by_id(&self, folder_id: &str) -> Result<bool, String> {
        self.with_conn(|conn| crate::is_folder_logically_deleted_by_id(conn, folder_id))
    }

    fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        self.with_conn(|conn| Ok(crate::get_setting(conn, key))).map_err(|e| e.to_string())
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.with_conn(|conn| crate::set_setting(conn, key, value))
    }

    fn mark_pending_push(&self, pending: bool) -> Result<(), String> {
        self.with_conn(|conn| crate::mark_pending_push(conn, pending))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("mem");
        crate::db::create_tables(&conn).expect("tables");
        conn
    }

    #[test]
    fn sqlite_store_should_get_bookmarks_empty() {
        let conn = setup_conn();
        let store = SqliteStore::new(Arc::new(Mutex::new(conn)));
        let res = store.get_bookmarks().expect("get");
        assert!(res.is_empty());
    }
}
