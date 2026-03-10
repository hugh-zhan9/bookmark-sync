use postgres::{Client, Row};
use postgres::types::ToSql;
use r2d2_postgres::postgres::tls::NoTls;
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;

use crate::config::PostgresConfig;
use crate::db::store::BookmarkStore;
use crate::events::metadata::SiteMetadata;
use crate::events::models::{BookmarkPayload, EventLog, SyncEvent};

pub fn build_dsn(host: &str, port: u16, db: &str, user: &str, password: &str, sslmode: &str) -> String {
    format!(
        "host={} port={} dbname={} user={} password={} sslmode={}",
        host, port, db, user, password, sslmode
    )
}

pub fn init_db(cfg: &PostgresConfig) -> Result<Pool<PostgresConnectionManager<NoTls>>, String> {
    let dsn = build_dsn(&cfg.host, cfg.port, &cfg.db, &cfg.user, &cfg.password, &cfg.sslmode);
    let manager = PostgresConnectionManager::new(dsn.parse::<postgres::Config>().map_err(|e| e.to_string())?, NoTls);
    let pool = Pool::new(manager).map_err(|e| e.to_string())?;
    let mut client = pool.get().map_err(|e| e.to_string())?;
    create_tables(&mut client)?;
    Ok(pool)
}

pub fn bookmark_select_sql() -> String {
    "
    SELECT b.id, b.url, b.title, b.description, b.favicon_url, b.host, b.created_at,
    (SELECT STRING_AGG(t.name, ',') FROM tags t JOIN bookmark_tags bt ON t.id = bt.tag_id WHERE bt.bookmark_id = b.id) as tag_list
    FROM bookmarks b
    ".to_string()
}

fn search_clause_for_param_pg(param_index: usize) -> String {
    let p = param_index + 1;
    format!(
        "(b.title ILIKE ${p} OR b.host ILIKE ${p} OR EXISTS (SELECT 1 FROM tags t JOIN bookmark_tags bt ON t.id = bt.tag_id WHERE bt.bookmark_id = b.id AND t.name ILIKE ${p}))"
    )
}

fn map_bookmark_row(row: &Row) -> BookmarkPayload {
    let tag_list: Option<String> = row.get(7);
    let tags = tag_list.map(|list| {
        list.split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    });
    BookmarkPayload {
        id: row.get(0),
        url: row.get(1),
        title: row.get(2),
        description: row.get(3),
        favicon_url: row.get(4),
        host: row.get(5),
        created_at: row.get(6),
        tags,
    }
}

pub struct PostgresStore {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl PostgresStore {
    pub fn new(pool: Pool<PostgresConnectionManager<NoTls>>) -> Self {
        Self { pool }
    }

    fn with_client<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&mut Client) -> Result<T, String>,
    {
        let mut client = self.pool.get().map_err(|e| e.to_string())?;
        f(&mut client)
    }
}

impl BookmarkStore for PostgresStore {
    fn get_bookmarks(&self) -> Result<Vec<BookmarkPayload>, String> {
        let sql = format!("{} WHERE b.is_deleted = FALSE ORDER BY b.created_at DESC", bookmark_select_sql());
        self.with_client(|client| {
            let rows = client.query(&sql, &[]).map_err(|e| e.to_string())?;
            Ok(rows.iter().map(map_bookmark_row).collect())
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
            .map(|(idx, _)| search_clause_for_param_pg(idx))
            .collect();
        let sql = format!(
            "{} WHERE b.is_deleted = FALSE AND {} ORDER BY b.created_at DESC",
            bookmark_select_sql(),
            term_clauses.join(" AND ")
        );
        let patterns: Vec<String> = tokens.iter().map(|t| format!("%{}%", t)).collect();
        self.with_client(|client| {
            let mut params: Vec<&(dyn ToSql + Sync)> = Vec::new();
            for p in &patterns {
                params.push(p);
            }
            let rows = client.query(&sql, &params).map_err(|e| e.to_string())?;
            Ok(rows.iter().map(map_bookmark_row).collect())
        })
    }

    fn apply_event(&self, log: &EventLog) -> Result<(), String> {
        apply_event_pg(self, log)
    }

    fn apply_event_if_new(&self, log: &EventLog) -> Result<bool, String> {
        apply_event_if_new_pg(self, log)
    }

    fn resolve_bookmark_id_for_url(&self, url: &str, fallback_id: &str) -> String {
        resolve_bookmark_id_for_url_pg(self, url, fallback_id)
    }

    fn apply_metadata_by_canonical_url(&self, canonical_url: &str, meta: &SiteMetadata) -> Result<usize, String> {
        apply_metadata_by_canonical_url_pg(self, canonical_url, meta)
    }

    fn is_bookmark_logically_deleted_by_canonical_url(&self, canonical_url: &str) -> Result<bool, String> {
        is_bookmark_logically_deleted_by_canonical_url_pg(self, canonical_url)
    }

    fn is_folder_logically_deleted_by_id(&self, folder_id: &str) -> Result<bool, String> {
        is_folder_logically_deleted_by_id_pg(self, folder_id)
    }

    fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        get_setting_pg(self, key)
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        set_setting_pg(self, key, value)
    }

    fn mark_pending_push(&self, pending: bool) -> Result<(), String> {
        mark_pending_push_pg(self, pending)
    }
}

fn apply_event_pg(store: &PostgresStore, log: &EventLog) -> Result<(), String> {
    store.with_client(|client| {
        match &log.event {
            SyncEvent::BookmarkAdded(b) => {
                client.execute(
                    "INSERT INTO bookmarks (id, url, canonical_url, title, description, favicon_url, host, created_at)
                     VALUES ($1, $2, $2, $3, $4, $5, $6, $7)
                     ON CONFLICT (canonical_url) DO UPDATE SET title = EXCLUDED.title, is_deleted = FALSE, updated_at = CURRENT_TIMESTAMP",
                    &[&b.id, &b.url, &b.title, &b.description, &b.favicon_url, &b.host, &b.created_at],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::FolderAdded { id, parent_id, name } => {
                client.execute(
                    "INSERT INTO folders (id, parent_id, name) VALUES ($1, $2, $3)
                     ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, parent_id = EXCLUDED.parent_id",
                    &[id, parent_id, name],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkDeleted { id } => {
                client.execute(
                    "UPDATE bookmarks SET is_deleted = TRUE, updated_at = CURRENT_TIMESTAMP WHERE id = $1",
                    &[id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkUpdated(b) => {
                client.execute(
                    "UPDATE bookmarks SET title = $1, url = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $3",
                    &[&b.title, &b.url, &b.id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::TagAdded { id, name } => {
                client.execute(
                    "INSERT INTO tags (id, name) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING",
                    &[id, name],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkTagged { bookmark_id, tag_id } => {
                client.execute(
                    "INSERT INTO bookmark_tags (bookmark_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    &[bookmark_id, tag_id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkUntagged { bookmark_id, tag_id } => {
                client.execute(
                    "DELETE FROM bookmark_tags WHERE bookmark_id = $1 AND tag_id = $2",
                    &[bookmark_id, tag_id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::FolderRenamed { id, name } => {
                client.execute(
                    "UPDATE folders SET name = $2 WHERE id = $1",
                    &[id, name],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::FolderDeleted { id } => {
                client.execute(
                    "UPDATE folders SET is_deleted = TRUE WHERE id = $1",
                    &[id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkAddedToFolder { bookmark_id, folder_id } => {
                client.execute(
                    "INSERT INTO folder_bookmarks (folder_id, bookmark_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    &[folder_id, bookmark_id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkRemovedFromFolder { bookmark_id, folder_id } => {
                client.execute(
                    "DELETE FROM folder_bookmarks WHERE folder_id = $1 AND bookmark_id = $2",
                    &[folder_id, bookmark_id],
                ).map_err(|e| e.to_string())?;
            }
            _ => {}
        }
        Ok(())
    })
}

fn apply_event_if_new_pg(store: &PostgresStore, log: &EventLog) -> Result<bool, String> {
    store.with_client(|client| {
        let inserted = client.execute(
            "INSERT INTO applied_event_ids (event_id) VALUES ($1) ON CONFLICT (event_id) DO NOTHING",
            &[&log.event_id],
        ).map_err(|e| e.to_string())?;
        if inserted == 0 {
            return Ok(false);
        }
        apply_event_pg(store, log)?;
        Ok(true)
    })
}

fn resolve_bookmark_id_for_url_pg(store: &PostgresStore, cleaned_url: &str, fallback_id: &str) -> String {
    store.with_client(|client| {
        let row = client.query_opt(
            "SELECT id FROM bookmarks WHERE canonical_url = $1 LIMIT 1",
            &[&cleaned_url],
        ).map_err(|e| e.to_string())?;
        Ok(row.map(|r| r.get::<_, String>(0)).unwrap_or_else(|| fallback_id.to_string()))
    }).unwrap_or_else(|_| fallback_id.to_string())
}

fn apply_metadata_by_canonical_url_pg(store: &PostgresStore, canonical_url: &str, meta: &SiteMetadata) -> Result<usize, String> {
    store.with_client(|client| {
        let updated = client.execute(
            "UPDATE bookmarks SET title = $1, favicon_url = $2, updated_at = CURRENT_TIMESTAMP WHERE canonical_url = $3",
            &[&meta.title, &meta.favicon_url, &canonical_url],
        ).map_err(|e| e.to_string())?;
        Ok(updated as usize)
    })
}

fn is_bookmark_logically_deleted_by_canonical_url_pg(store: &PostgresStore, canonical_url: &str) -> Result<bool, String> {
    store.with_client(|client| {
        let row = client.query_opt(
            "SELECT is_deleted FROM bookmarks WHERE canonical_url = $1 LIMIT 1",
            &[&canonical_url],
        ).map_err(|e| e.to_string())?;
        Ok(row.map(|r| r.get::<_, bool>(0)).unwrap_or(false))
    })
}

fn is_folder_logically_deleted_by_id_pg(store: &PostgresStore, folder_id: &str) -> Result<bool, String> {
    store.with_client(|client| {
        let row = client.query_opt(
            "SELECT is_deleted FROM folders WHERE id = $1 LIMIT 1",
            &[&folder_id],
        ).map_err(|e| e.to_string())?;
        Ok(row.map(|r| r.get::<_, bool>(0)).unwrap_or(false))
    })
}

fn get_setting_pg(store: &PostgresStore, key: &str) -> Result<Option<String>, String> {
    store.with_client(|client| {
        let row = client.query_opt(
            "SELECT value FROM app_settings WHERE key = $1 LIMIT 1",
            &[&key],
        ).map_err(|e| e.to_string())?;
        Ok(row.map(|r| r.get::<_, String>(0)))
    })
}

fn set_setting_pg(store: &PostgresStore, key: &str, value: &str) -> Result<(), String> {
    store.with_client(|client| {
        client.execute(
            "INSERT INTO app_settings (key, value) VALUES ($1, $2)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
            &[&key, &value],
        ).map_err(|e| e.to_string())?;
        Ok(())
    })
}

fn mark_pending_push_pg(store: &PostgresStore, pending: bool) -> Result<(), String> {
    set_setting_pg(store, "event_sync_pending_push", if pending { "1" } else { "0" })
}

fn create_tables(client: &mut Client) -> Result<(), String> {
    client
        .batch_execute(
            "
            CREATE TABLE IF NOT EXISTS event_cursors (
                id INTEGER PRIMARY KEY,
                last_event_id TEXT NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                canonical_url TEXT UNIQUE NOT NULL,
                title TEXT,
                description TEXT,
                favicon_url TEXT,
                host TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                is_deleted BOOLEAN DEFAULT FALSE
            );

            CREATE TABLE IF NOT EXISTS tags (
                id TEXT PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS bookmark_tags (
                bookmark_id TEXT NOT NULL,
                tag_id TEXT NOT NULL,
                PRIMARY KEY (bookmark_id, tag_id)
            );

            CREATE TABLE IF NOT EXISTS folders (
                id TEXT PRIMARY KEY,
                parent_id TEXT,
                name TEXT NOT NULL,
                is_deleted BOOLEAN DEFAULT FALSE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS folder_bookmarks (
                folder_id TEXT NOT NULL,
                bookmark_id TEXT NOT NULL,
                PRIMARY KEY (folder_id, bookmark_id)
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS applied_event_ids (
                event_id TEXT PRIMARY KEY
            );
            ",
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dsn_should_format_connection_string() {
        let dsn = build_dsn("127.0.0.1", 5432, "bookmark_sync", "bookmark", "secret", "prefer");
        assert!(dsn.contains("host=127.0.0.1"));
        assert!(dsn.contains("port=5432"));
        assert!(dsn.contains("dbname=bookmark_sync"));
    }

    #[test]
    fn pg_search_sql_should_use_string_agg() {
        let sql = bookmark_select_sql();
        assert!(sql.contains("STRING_AGG"));
    }
}
