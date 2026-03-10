pub mod config;
pub mod db;
pub mod events;
pub mod sync;

use std::sync::Mutex;
use std::thread;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::fs;
use tauri::{Emitter, Manager, State};
use events::models::{BookmarkPayload, SyncEvent, EventLog};
use events::replay_events;
use events::cleaner;
use events::metadata;
use events::segment;
use events::device_registry;
use events::cleanup;
use rusqlite::{params, params_from_iter};
use db::browser_scanner;
use db::router::DbRouter;

struct AppState {
    router: Mutex<DbRouter>,
    sync_lock: Mutex<()>,
    app_data_dir: PathBuf,
    config: Mutex<config::AppConfig>,
    config_dir: PathBuf,
}

fn debug_log_path_from_conn(conn: &rusqlite::Connection) -> Option<String> {
    conn.path()
        .map(|db_path| Path::new(db_path).with_file_name("debug.log").to_string_lossy().to_string())
}

fn append_debug_log(conn: &rusqlite::Connection, message: &str) {
    let line = format!("[{}] {}", chrono::Utc::now().to_rfc3339(), message);
    eprintln!("{line}");
    if let Some(log_path) = debug_log_path_from_conn(conn) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn append_debug_log_str(message: &str) {
    let line = format!("[{}] {}", chrono::Utc::now().to_rfc3339(), message);
    eprintln!("{line}");
}

/// 获取或创建本设备唯一标识（存储在 app_settings 中）
fn get_or_create_device_id(conn: &rusqlite::Connection) -> String {
    if let Some(id) = get_setting(conn, "device_id") {
        return id;
    }
    let new_id = uuid::Uuid::new_v4().to_string();
    let _ = set_setting(conn, "device_id", &new_id);
    new_id
}

fn get_setting(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1 LIMIT 1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

fn set_setting(conn: &rusqlite::Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn mark_pending_push(conn: &rusqlite::Connection, pending: bool) -> Result<(), String> {
    set_setting(conn, "event_sync_pending_push", if pending { "1" } else { "0" })
}

fn sqlite_only_sync_guard(kind: config::DataSourceKind) -> Result<(), String> {
    match kind {
        config::DataSourceKind::Sqlite => Ok(()),
        config::DataSourceKind::Postgres => Err("Git 同步仅支持 SQLite 数据源".into()),
    }
}

fn validate_pg_config(cfg: &config::AppConfig) -> Result<(), String> {
    if cfg.postgres.host.trim().is_empty() {
        return Err("postgres host 不能为空".into());
    }
    if cfg.postgres.db.trim().is_empty() {
        return Err("postgres db 不能为空".into());
    }
    if cfg.postgres.user.trim().is_empty() {
        return Err("postgres user 不能为空".into());
    }
    Ok(())
}

#[tauri::command]
fn get_app_config(state: State<'_, AppState>) -> Result<config::AppConfig, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    Ok(cfg)
}

#[tauri::command]
fn set_app_config(state: State<'_, AppState>, next: config::AppConfig) -> Result<(), String> {
    apply_app_config(&state, next)
}

fn apply_app_config(state: &AppState, next: config::AppConfig) -> Result<(), String> {
    if next.data_source == config::DataSourceKind::Postgres {
        validate_pg_config(&next)?;
        db::postgres::test_connection(&next.postgres)?;
    }
    let mut router = state.router.lock().map_err(|e| e.to_string())?;
    router.reinit(&next)?;
    config::save(&state.config_dir, &next)?;
    *state.config.lock().map_err(|e| e.to_string())? = next;
    Ok(())
}

#[cfg(test)]
mod data_source_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sqlite_only_sync_should_block_pg() {
        let res = sqlite_only_sync_guard(config::DataSourceKind::Postgres);
        assert!(res.is_err());
    }

    #[test]
    fn switch_should_reject_invalid_pg_config() {
        let mut cfg = config::AppConfig::default();
        cfg.data_source = config::DataSourceKind::Postgres;
        cfg.postgres.host = "".into();
        let err = validate_pg_config(&cfg).unwrap_err();
        assert!(err.contains("postgres host"));
    }

    #[test]
    fn set_app_config_should_keep_sqlite_on_pg_connection_failure() {
        let app_dir = tempdir().expect("app dir");
        let config_dir = tempdir().expect("config dir");
        let cfg = config::AppConfig::default();
        let router = DbRouter::init(&cfg, app_dir.path().to_path_buf()).expect("init router");
        let state = AppState {
            router: Mutex::new(router),
            sync_lock: Mutex::new(()),
            app_data_dir: app_dir.path().to_path_buf(),
            config: Mutex::new(cfg.clone()),
            config_dir: config_dir.path().to_path_buf(),
        };

        let mut next = cfg.clone();
        next.data_source = config::DataSourceKind::Postgres;
        next.postgres.host = "127.0.0.1".into();
        next.postgres.port = 1;

        let err = apply_app_config(&state, next).unwrap_err();
        assert!(!err.is_empty());
        let current = state.config.lock().expect("config lock").clone();
        assert_eq!(current.data_source, config::DataSourceKind::Sqlite);
        let router_kind = state.router.lock().expect("router lock").kind();
        assert_eq!(router_kind, config::DataSourceKind::Sqlite);
    }
}

fn app_dir_from_conn(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    let db_path = conn
        .path()
        .ok_or_else(|| "无法定位数据库路径".to_string())?;
    let app_dir = Path::new(db_path)
        .parent()
        .ok_or_else(|| "无法定位应用目录".to_string())?;
    Ok(app_dir.to_path_buf())
}

fn tokenize_search_query(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in query.split_whitespace() {
        let t = token.trim().to_lowercase();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

fn search_clause_for_param(param_index: usize) -> String {
    let p = param_index + 1;
    format!(
        "(b.title LIKE ?{p} OR b.host LIKE ?{p} OR EXISTS (SELECT 1 FROM tags t JOIN bookmark_tags bt ON t.id = bt.tag_id WHERE bt.bookmark_id = b.id AND t.name LIKE ?{p}))"
    )
}

fn resolve_bookmark_id_for_url(
    conn: &rusqlite::Connection,
    cleaned_url: &str,
    fallback_id: &str,
) -> String {
    conn.query_row(
        "SELECT id FROM bookmarks WHERE canonical_url = ?1 LIMIT 1",
        params![cleaned_url],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_else(|_| fallback_id.to_string())
}

fn is_bookmark_logically_deleted_by_canonical_url(
    conn: &rusqlite::Connection,
    canonical_url: &str,
) -> Result<bool, String> {
    let status: Option<i64> = conn
        .query_row(
            "SELECT is_deleted FROM bookmarks WHERE canonical_url = ?1 LIMIT 1",
            params![canonical_url],
            |r| r.get(0),
        )
        .ok();
    Ok(status.unwrap_or(0) == 1)
}

fn is_folder_logically_deleted_by_id(
    conn: &rusqlite::Connection,
    folder_id: &str,
) -> Result<bool, String> {
    let status: Option<i64> = conn
        .query_row(
            "SELECT is_deleted FROM folders WHERE id = ?1 LIMIT 1",
            params![folder_id],
            |r| r.get(0),
        )
        .ok();
    Ok(status.unwrap_or(0) == 1)
}

fn apply_metadata_by_canonical_url(
    conn: &rusqlite::Connection,
    canonical_url: &str,
    meta: &metadata::SiteMetadata,
) -> Result<usize, String> {
    conn.execute(
        "UPDATE bookmarks SET title = ?1, favicon_url = ?2, updated_at = CURRENT_TIMESTAMP WHERE canonical_url = ?3",
        params![meta.title, meta.favicon_url, canonical_url],
    )
    .map_err(|e| e.to_string())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct FolderNode { id: String, parent_id: Option<String>, name: String }

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct TagNode { id: String, name: String }

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct BookmarkExistsResult {
    exists: bool,
    title: Option<String>,
}

fn map_bookmark_row(row: &rusqlite::Row) -> rusqlite::Result<BookmarkPayload> {
    let tag_str: Option<String> = row.get(7)?;
    let tags = tag_str.map(|s| s.split(',').map(|t| t.to_string()).collect());
    
    Ok(BookmarkPayload {
        id: row.get(0)?, url: row.get(1)?, title: row.get(2)?, description: row.get(3)?,
        favicon_url: row.get(4)?, host: row.get(5)?, created_at: row.get(6)?, tags,
    })
}

#[tauri::command]
fn check_bookmark_exists(state: State<'_, AppState>, url: String) -> Result<BookmarkExistsResult, String> {
    let cleaned = cleaner::clean_url(&url);
    if cleaned.is_empty() {
        return Ok(BookmarkExistsResult {
            exists: false,
            title: None,
        });
    }
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let row: Option<(Option<String>,)> = conn
        .query_row(
            "SELECT title FROM bookmarks WHERE canonical_url = ?1 AND is_deleted = 0 LIMIT 1",
            params![cleaned],
            |r| Ok((r.get(0)?,)),
        )
        .ok();
    match row {
        Some((title,)) => Ok(BookmarkExistsResult { exists: true, title }),
        None => Ok(BookmarkExistsResult {
            exists: false,
            title: None,
        }),
    }
}

const BOOKMARK_SELECT_SQL: &str = "
    SELECT b.id, b.url, b.title, b.description, b.favicon_url, b.host, b.created_at,
    (SELECT GROUP_CONCAT(t.name) FROM tags t JOIN bookmark_tags bt ON t.id = bt.tag_id WHERE bt.bookmark_id = b.id) as tag_list
    FROM bookmarks b
";

#[tauri::command]
fn get_bookmarks(state: State<'_, AppState>) -> Result<Vec<BookmarkPayload>, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let sql = format!("{} WHERE b.is_deleted = 0 ORDER BY b.created_at DESC", BOOKMARK_SELECT_SQL);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let iter = stmt.query_map([], map_bookmark_row).map_err(|e| e.to_string())?;
    let mut res = Vec::new();
    for b in iter { if let Ok(x) = b { res.push(x); } }
    Ok(res)
}

#[tauri::command]
fn search_bookmarks(state: State<'_, AppState>, query: String) -> Result<Vec<BookmarkPayload>, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;

    let tokens = tokenize_search_query(&query);
    if tokens.is_empty() {
        let sql = format!("{} WHERE b.is_deleted = 0 ORDER BY b.created_at DESC", BOOKMARK_SELECT_SQL);
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let iter = stmt.query_map([], map_bookmark_row).map_err(|e| e.to_string())?;
        let mut res = Vec::new();
        for b in iter { if let Ok(x) = b { res.push(x); } }
        return Ok(res);
    }

    let term_clauses: Vec<String> = tokens
        .iter()
        .enumerate()
        .map(|(idx, _)| search_clause_for_param(idx))
        .collect();
    let sql = format!(
        "{} WHERE b.is_deleted = 0 AND {} ORDER BY b.created_at DESC",
        BOOKMARK_SELECT_SQL,
        term_clauses.join(" AND ")
    );

    let patterns: Vec<String> = tokens.iter().map(|t| format!("%{}%", t)).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let iter = stmt
        .query_map(params_from_iter(patterns.iter()), map_bookmark_row)
        .map_err(|e| e.to_string())?;
    let mut res = Vec::new();
    for b in iter { if let Ok(x) = b { res.push(x); } }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::{
        tokenize_search_query,
        search_clause_for_param,
        resolve_bookmark_id_for_url,
        apply_metadata_by_canonical_url,
        is_bookmark_logically_deleted_by_canonical_url,
        is_folder_logically_deleted_by_id,
    };
    use rusqlite::{Connection, params};
    use crate::events::metadata::SiteMetadata;

    #[test]
    fn tokenize_search_query_should_split_by_whitespace_and_dedup() {
        let tokens = tokenize_search_query("  rust   tag:work   rust  ");
        assert_eq!(tokens, vec!["rust".to_string(), "tag:work".to_string()]);
    }

    #[test]
    fn search_clause_should_not_match_raw_url_path() {
        let clause = search_clause_for_param(1);
        assert!(!clause.contains("b.url LIKE"));
        assert!(clause.contains("b.title LIKE"));
        assert!(clause.contains("b.host LIKE"));
        assert!(clause.contains("t.name LIKE"));
    }

    #[test]
    fn resolve_bookmark_id_should_use_existing_row_when_canonical_url_exists() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(
            "
            CREATE TABLE bookmarks (
              id TEXT PRIMARY KEY,
              url TEXT NOT NULL,
              canonical_url TEXT UNIQUE NOT NULL
            );
            ",
        )
        .expect("create table");
        conn.execute(
            "INSERT INTO bookmarks (id, url, canonical_url) VALUES (?1, ?2, ?3)",
            params!["existing-id", "https://juejin.cn/post/1", "https://juejin.cn/post/1"],
        )
        .expect("seed bookmark");

        let resolved = resolve_bookmark_id_for_url(&conn, "https://juejin.cn/post/1", "new-import-id");
        assert_eq!(resolved, "existing-id");
    }

    #[test]
    fn apply_metadata_should_update_row_by_canonical_url() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(
            "
            CREATE TABLE bookmarks (
              id TEXT PRIMARY KEY,
              url TEXT NOT NULL,
              canonical_url TEXT UNIQUE NOT NULL,
              title TEXT,
              favicon_url TEXT,
              updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )
        .expect("create table");
        conn.execute(
            "INSERT INTO bookmarks (id, url, canonical_url, title) VALUES (?1, ?2, ?3, ?4)",
            params!["existing-id", "https://juejin.cn/post/1?utm_source=x", "https://juejin.cn/post/1", "Loading..."],
        )
        .expect("seed bookmark");
        let meta = SiteMetadata {
            title: Some("Juejin Title".to_string()),
            favicon_url: Some("https://juejin.cn/favicon.ico".to_string()),
        };

        let updated = apply_metadata_by_canonical_url(&conn, "https://juejin.cn/post/1", &meta).expect("apply metadata");
        assert_eq!(updated, 1);
        let title: String = conn
            .query_row(
                "SELECT title FROM bookmarks WHERE canonical_url = ?1",
                params!["https://juejin.cn/post/1"],
                |r| r.get(0),
            )
            .expect("query title");
        assert_eq!(title, "Juejin Title");
    }

    #[test]
    fn import_should_skip_logically_deleted_bookmark() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(
            "
            CREATE TABLE bookmarks (
              id TEXT PRIMARY KEY,
              canonical_url TEXT UNIQUE NOT NULL,
              is_deleted BOOLEAN DEFAULT 0
            );
            ",
        )
        .expect("create table");
        conn.execute(
            "INSERT INTO bookmarks (id, canonical_url, is_deleted) VALUES (?1, ?2, 1)",
            params!["b1", "https://example.com/a"],
        )
        .expect("seed bookmark");

        let skipped = is_bookmark_logically_deleted_by_canonical_url(&conn, "https://example.com/a")
            .expect("query bookmark deleted");
        assert!(skipped);
    }

    #[test]
    fn import_should_skip_logically_deleted_folder() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(
            "
            CREATE TABLE folders (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              is_deleted BOOLEAN DEFAULT 0
            );
            ",
        )
        .expect("create table");
        conn.execute(
            "INSERT INTO folders (id, name, is_deleted) VALUES (?1, ?2, 1)",
            params!["chrome-123", "工作"],
        )
        .expect("seed folder");

        let skipped = is_folder_logically_deleted_by_id(&conn, "chrome-123")
            .expect("query folder deleted");
        assert!(skipped);
    }
}

#[tauri::command]
fn get_folders(state: State<'_, AppState>) -> Result<Vec<FolderNode>, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, parent_id, name FROM folders WHERE is_deleted = 0 ORDER BY name ASC")
        .map_err(|e| e.to_string())?;
    let iter = stmt.query_map([], |row| Ok(FolderNode { id: row.get(0)?, parent_id: row.get(1)?, name: row.get(2)? })).map_err(|e| e.to_string())?;
    let mut res = Vec::new();
    for f in iter { if let Ok(x) = f { res.push(x); } }
    let sample_ids = res.iter().take(5).map(|f| f.id.clone()).collect::<Vec<_>>().join(",");
    append_debug_log(&conn, &format!("get_folders count={} sample_ids={}", res.len(), sample_ids));
    Ok(res)
}

#[tauri::command]
fn get_tags(state: State<'_, AppState>) -> Result<Vec<TagNode>, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, name FROM tags ORDER BY name ASC").map_err(|e| e.to_string())?;
    let iter = stmt.query_map([], |row| Ok(TagNode { id: row.get(0)?, name: row.get(1)? })).map_err(|e| e.to_string())?;
    let mut res = Vec::new();
    for t in iter { if let Ok(x) = t { res.push(x); } }
    Ok(res)
}

#[tauri::command]
fn get_delete_sync_setting(state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let raw = get_setting(&conn, "sync_delete_to_browser").unwrap_or_else(|| "0".to_string());
    Ok(raw == "1" || raw.eq_ignore_ascii_case("true"))
}

#[tauri::command]
fn set_delete_sync_setting(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    set_setting(&conn, "sync_delete_to_browser", if enabled { "1" } else { "0" })
}

#[tauri::command]
fn get_bookmarks_by_folder(state: State<'_, AppState>, folder_id: String) -> Result<Vec<BookmarkPayload>, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let sql = format!("{} JOIN folder_bookmarks fb ON b.id = fb.bookmark_id WHERE fb.folder_id = ?1 AND b.is_deleted = 0 ORDER BY b.created_at DESC", BOOKMARK_SELECT_SQL);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let iter = stmt.query_map([&folder_id], map_bookmark_row).map_err(|e| e.to_string())?;
    let mut res = Vec::new();
    for b in iter { if let Ok(x) = b { res.push(x); } }
    Ok(res)
}

#[tauri::command]
fn get_bookmarks_by_tag(state: State<'_, AppState>, tag_id: String) -> Result<Vec<BookmarkPayload>, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let sql = format!("{} JOIN bookmark_tags bt ON b.id = bt.bookmark_id WHERE bt.tag_id = ?1 AND b.is_deleted = 0 ORDER BY b.created_at DESC", BOOKMARK_SELECT_SQL);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let iter = stmt.query_map([&tag_id], map_bookmark_row).map_err(|e| e.to_string())?;
    let mut res = Vec::new();
    for b in iter { if let Ok(x) = b { res.push(x); } }
    Ok(res)
}

#[tauri::command]
fn add_tag_to_bookmark(state: State<'_, AppState>, bookmark_id: String, tag_name: String) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let tag_id = uuid::Uuid::new_v4().to_string();
    
    // 1. Add tag event
    let tag_event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::TagAdded { id: tag_id.clone(), name: tag_name } };
    replay_events(&mut conn, vec![tag_event]).map_err(|e| e.to_string())?;
    
    // 2. Fetch actual tag_id (if already existed)
    let actual_tag_id: String = conn.query_row("SELECT id FROM tags WHERE name = (SELECT name FROM tags WHERE id = ?1)", params![tag_id], |r| r.get(0))
        .unwrap_or(tag_id);

    // 3. Link event
    let link_event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::BookmarkTagged { bookmark_id, tag_id: actual_tag_id } };
    replay_events(&mut conn, vec![link_event])
}

#[tauri::command]
fn remove_tag_from_bookmark(state: State<'_, AppState>, bookmark_id: String, tag_name: String) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let tag_id: String = match conn.query_row("SELECT id FROM tags WHERE name = ?1 LIMIT 1", params![tag_name], |r| r.get(0)) {
        Ok(id) => id,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };

    let event = EventLog {
        event_id: uuid::Uuid::new_v4().to_string(),
        device_id: "local".into(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        event: SyncEvent::BookmarkUntagged { bookmark_id, tag_id },
    };
    replay_events(&mut conn, vec![event])
}

#[tauri::command]
fn delete_folder(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let before_folder_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders WHERE id = ?1 AND is_deleted = 0", params![id.clone()], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let before_link_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folder_bookmarks WHERE folder_id = ?1", params![id.clone()], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let before_child_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders WHERE parent_id = ?1", params![id.clone()], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    append_debug_log(
        &conn,
        &format!(
            "delete_folder start id={} before_folders={} before_links={} before_children={}",
            id, before_folder_count, before_link_count, before_child_count
        ),
    );

    let event = EventLog {
        event_id: uuid::Uuid::new_v4().to_string(),
        device_id: "local".into(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        event: SyncEvent::FolderDeleted { id: id.clone() },
    };
    if let Err(e) = replay_events(&mut conn, vec![event]) {
        append_debug_log(&conn, &format!("delete_folder replay_events failed id={} err={}", id, e));
        return Err(e);
    }
    append_debug_log(&conn, &format!("delete_folder replay_events ok id={}", id));

    let after_folder_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders WHERE id = ?1 AND is_deleted = 0", params![id.clone()], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    append_debug_log(
        &conn,
        &format!(
            "delete_folder end id={} after_visible_folders={}",
            id, after_folder_count
        ),
    );
    if after_folder_count > 0 {
        return Err(format!("删除失败：文件夹 {} 仍存在", id));
    }
    Ok(())
}

#[tauri::command]
fn rename_folder(state: State<'_, AppState>, id: String, name: String) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::FolderRenamed { id, name } };
    replay_events(&mut conn, vec![event])
}

#[tauri::command]
fn add_bookmark_to_folder(state: State<'_, AppState>, bookmark_id: String, folder_id: String) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::BookmarkAddedToFolder { bookmark_id, folder_id } };
    replay_events(&mut conn, vec![event])
}

#[tauri::command]
fn remove_bookmark_from_folder(state: State<'_, AppState>, bookmark_id: String, folder_id: String) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::BookmarkRemovedFromFolder { bookmark_id, folder_id } };
    replay_events(&mut conn, vec![event])
}

#[tauri::command]
fn get_bookmark_folders(state: State<'_, AppState>, bookmark_id: String) -> Result<Vec<String>, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT folder_id FROM folder_bookmarks WHERE bookmark_id = ?1").map_err(|e| e.to_string())?;
    let iter = stmt.query_map([&bookmark_id], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
    let mut res = Vec::new();
    for id in iter { if let Ok(x) = id { res.push(x); } }
    Ok(res)
}

#[tauri::command]
fn get_debug_log_path(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    debug_log_path_from_conn(&conn).ok_or_else(|| "无法获取日志路径".to_string())
}

#[tauri::command]
fn write_debug_log(state: State<'_, AppState>, message: String) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    append_debug_log(&conn, &format!("frontend {}", message));
    Ok(())
}

#[tauri::command]
fn add_bookmark(state: State<'_, AppState>, app_handle: tauri::AppHandle, mut payload: BookmarkPayload) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    payload.url = cleaner::clean_url(&payload.url);
    let canonical_url_to_update = payload.url.clone();
    let url_to_fetch = payload.url.clone();
    let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::BookmarkAdded(payload) };
    replay_events(&mut conn, vec![event]).map_err(|e| e.to_string())?;
    
    let path = conn.path().map(|p| p.to_string());
    let app_handle = app_handle.clone();
    if let Some(p) = path {
        thread::spawn(move || {
            if let Ok(meta) = metadata::fetch_metadata(&url_to_fetch) {
                if let Ok(c) = rusqlite::Connection::open(p) {
                    let updated = apply_metadata_by_canonical_url(&c, &canonical_url_to_update, &meta).unwrap_or(0);
                    if updated > 0 {
                        let _ = app_handle.emit("bookmarks-updated", ());
                    }
                }
            }
        });
    }
    Ok(())
}

#[tauri::command]
fn delete_bookmark(
    state: State<'_, AppState>,
    id: String,
    sync_browser_delete: Option<bool>,
) -> Result<(), String> {
    let bookmark_id = id.clone();
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::BookmarkDeleted { id } };
    replay_events(&mut conn, vec![event])?;

    if sync_browser_delete.unwrap_or(false) {
        let deleted = browser_scanner::delete_bookmark_in_browser(&bookmark_id)?;
        append_debug_log(&conn, &format!("delete_bookmark browser_delete={} bookmark_id={}", deleted, bookmark_id));
    }
    Ok(())
}

#[tauri::command]
fn update_bookmark(state: State<'_, AppState>, payload: BookmarkPayload) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::BookmarkUpdated(payload) };
    replay_events(&mut conn, vec![event])
}

#[tauri::command]
fn create_folder(state: State<'_, AppState>, name: String, parent_id: Option<String>) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: "local".into(), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::FolderAdded { id: uuid::Uuid::new_v4().to_string(), parent_id, name } };
    replay_events(&mut conn, vec![event])
}

#[tauri::command]
async fn import_browser_bookmarks(state: State<'_, AppState>) -> Result<usize, String> {
    let nodes = browser_scanner::scan_all_nodes();
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let mut conn = conn.lock().map_err(|e| e.to_string())?;
    let mut count = 0;
    
    // Begin a transaction to vastly improve insert performance.
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    let mut events_to_replay = Vec::new();
    let mut folder_bookmarks = Vec::new();
    
    for n in nodes {
        let stable_id = format!("{}-{}", n.browser.to_lowercase(), n.original_id);
        let stable_parent_id = n.parent_original_id.map(|pid| format!("{}-{}", n.browser.to_lowercase(), pid));
        if n.is_folder {
            if is_folder_logically_deleted_by_id(&tx, &stable_id)? {
                continue;
            }
            let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: format!("{}_import", n.browser), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::FolderAdded { id: stable_id, parent_id: stable_parent_id, name: n.title } };
            // Since replay_events appends the event log, it might be better handled later, but we apply it to tx
            if crate::db::apply_event_if_new(&tx, &event).unwrap_or(false) {
                events_to_replay.push(event);
            }
        } else {
            let url = cleaner::clean_url(&n.url.unwrap_or_default());
            if url.is_empty() { continue; }
            if is_bookmark_logically_deleted_by_canonical_url(&tx, &url)? {
                continue;
            }
            let payload = BookmarkPayload { id: stable_id.clone(), url: url.clone(), title: Some(n.title), description: None, favicon_url: None, host: url::Url::parse(&url).ok().and_then(|u| u.host_str().map(|h| h.to_string())), created_at: chrono::Utc::now().to_rfc3339(), tags: None };
            let event = EventLog { event_id: uuid::Uuid::new_v4().to_string(), device_id: format!("{}_import", n.browser), timestamp: chrono::Utc::now().timestamp_millis(), event: SyncEvent::BookmarkAdded(payload) };
            if crate::db::apply_event_if_new(&tx, &event).unwrap_or(false) {
                events_to_replay.push(event);
                if let Some(fid) = stable_parent_id {
                    let actual_bookmark_id = resolve_bookmark_id_for_url(&tx, &url, &stable_id);
                    folder_bookmarks.push((fid, actual_bookmark_id));
                }
                count += 1;
            }
        }
    }
    
    for (fid, actual_bookmark_id) in folder_bookmarks {
        let _ = tx.execute(
            "INSERT INTO folder_bookmarks (folder_id, bookmark_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
            params![fid, actual_bookmark_id],
        );
    }
    
    tx.commit().map_err(|e| e.to_string())?;

    // Append to ndjson file afterwards to avoid file I/O within SQLite TX
    if !events_to_replay.is_empty() {
        crate::events::append_events_to_local_log(&conn, &events_to_replay)?;
    }
    
    Ok(count)
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct BrowserAutoSyncSettings {
    startup_enabled: bool,
    interval_enabled: bool,
    interval_minutes: u32,
}

#[tauri::command]
fn get_browser_auto_sync_settings(state: State<'_, AppState>) -> Result<BrowserAutoSyncSettings, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let startup_enabled = get_setting(&conn, "browser_auto_sync_startup")
        .map(|v| v == "1")
        .unwrap_or(true);
    let interval_enabled = get_setting(&conn, "browser_auto_sync_interval_enabled")
        .map(|v| v == "1")
        .unwrap_or(true);
    let interval_minutes = get_setting(&conn, "browser_auto_sync_interval_minutes")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(5)
        .max(1);
    Ok(BrowserAutoSyncSettings {
        startup_enabled,
        interval_enabled,
        interval_minutes,
    })
}

#[tauri::command]
fn set_browser_auto_sync_settings(
    state: State<'_, AppState>,
    startup_enabled: bool,
    interval_enabled: bool,
    interval_minutes: u32,
) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let minutes = interval_minutes.max(1);
    set_setting(&conn, "browser_auto_sync_startup", if startup_enabled { "1" } else { "0" })?;
    set_setting(&conn, "browser_auto_sync_interval_enabled", if interval_enabled { "1" } else { "0" })?;
    set_setting(&conn, "browser_auto_sync_interval_minutes", &minutes.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_git_sync_repo_dir(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    Ok(get_setting(&conn, "git_sync_repo_dir").unwrap_or_default())
}

#[tauri::command]
fn set_git_sync_repo_dir(state: State<'_, AppState>, repo_dir: String) -> Result<String, String> {
    if !sync::is_git_repo_dir(&repo_dir) {
        return Err("目录不是 git 仓库".to_string());
    }
    let branch = sync::current_branch(&repo_dir)?;
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    set_setting(&conn, "git_sync_repo_dir", &repo_dir)?;
    Ok(branch)
}

#[tauri::command]
fn sync_github_incremental(state: State<'_, AppState>) -> Result<(), String> {
    let _sync_guard = state.sync_lock.lock().map_err(|e| e.to_string())?;
    let router = state.router.lock().map_err(|e| e.to_string())?;
    sqlite_only_sync_guard(router.kind())?;
    let (repo_dir, app_dir, device_id) = {
        let conn = state
            .router
            .lock()
            .map_err(|e| e.to_string())?
            .sqlite_conn()?;
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let repo_dir = get_setting(&conn, "git_sync_repo_dir")
            .ok_or_else(|| "请先设置 Git 仓库目录".to_string())?;
        let app_dir = app_dir_from_conn(&conn)?;
        let device_id = get_or_create_device_id(&conn);
        (repo_dir, app_dir, device_id)
    };
    if !sync::is_git_repo_dir(&repo_dir) {
        return Err("Git 仓库目录无效".to_string());
    }

    sync::git_pull_current_branch(&repo_dir)?;
    {
        let conn = state
            .router
            .lock()
            .map_err(|e| e.to_string())?
            .sqlite_conn()?;
        let mut conn = conn.lock().map_err(|e| e.to_string())?;
        sync_events_from_repo(&mut conn, &repo_dir)?;
    }

    match sync_events_to_repo(&repo_dir, &app_dir, &device_id) {
        Ok(_) => {
            let conn = state
                .router
                .lock()
                .map_err(|e| e.to_string())?
                .sqlite_conn()?;
            let conn = conn.lock().map_err(|e| e.to_string())?;
            mark_pending_push(&conn, false)?;
            Ok(())
        }
        Err(e) => {
            let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
            let _ = mark_pending_push(&conn, true);
            Err(e)
        }
    }
}

fn sync_events_from_repo(conn: &mut rusqlite::Connection, repo_dir: &str) -> Result<(), String> {
    let events_dir = sync::ensure_events_dir(repo_dir)?;
    // 向后兼容：迁移旧版单文件
    segment::migrate_legacy_if_exists(&events_dir)?;
    // 从所有 segment 中读取事件（按 timestamp 升序）
    let logs = segment::read_all_events(&events_dir)?;
    if logs.is_empty() {
        return Ok(());
    }
    replay_events(conn, logs)?;
    Ok(())
}

fn sync_events_to_repo(
    repo_dir: &str,
    app_dir: &Path,
    device_id: &str,
) -> Result<(), String> {
    let events_dir = sync::ensure_events_dir(repo_dir)?;
    let devices_dir = events_dir.join("devices");
    fs::create_dir_all(&devices_dir).map_err(|e| e.to_string())?;

    // 向后兼容：迁移本地旧版单文件
    segment::migrate_legacy_if_exists(app_dir)?;

    // 向后兼容：repo 侧存在旧版 events.ndjson → 用 git rm 删除，改为 segment 模式
    let repo_legacy = events_dir.join(segment::LEGACY_SEGMENT_NAME);
    if repo_legacy.exists() {
        // 将旧文件内容迁移为 events-000001.ndjson（先本地操作，不用 git）
        let legacy_target = events_dir.join("events-000001.ndjson");
        if !legacy_target.exists() {
            fs::rename(&repo_legacy, &legacy_target).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&repo_legacy).map_err(|e| e.to_string())?;
        }
        // 从 git 索引中删除旧文件
        let _ = std::process::Command::new("git")
            .args(["-C", repo_dir, "rm", "--cached", "--force",
                   &format!("events/{}", segment::LEGACY_SEGMENT_NAME)])
            .status();
    }

    // 将本地所有 sealed segments 同步到 repo（只复制 repo 缺少的）
    for seg in segment::list_sealed_segments(app_dir)? {
        let name = seg.file_name().unwrap();
        let repo_seg = events_dir.join(name);
        if !repo_seg.exists() {
            fs::copy(&seg, &repo_seg).map_err(|e| e.to_string())?;
        }
    }

    // 将本地 current segment 的内容复制到 repo current segment
    let local_current = app_dir.join(segment::CURRENT_SEGMENT_NAME);
    if local_current.exists() {
        let local_events = segment::read_events_from_file(&local_current)?;
        if !local_events.is_empty() {
            segment::append_to_current_segment(&events_dir, &local_events)?;
        }
    }

    // 计算本地最大同步时间戳并更新设备注册表
    let all_local = segment::read_all_events(app_dir)?;
    let max_local_ts = all_local.iter().map(|e| e.timestamp).max().unwrap_or(0);
    device_registry::update_device(&devices_dir, device_id, max_local_ts)?;

    // 尝试清理所有设备已同步的旧 segment
    let cleaned = cleanup::try_cleanup_old_segments(&events_dir, &devices_dir)
        .unwrap_or(0);
    if cleaned > 0 {
        append_debug_log_str(&format!("sync_events_to_repo: cleaned {} old segments", cleaned));
    }

    // git add events/ 并提交推送（包括新 segment + 已 rm 的旧文件）
    sync::git_add_commit_push_current_branch(
        repo_dir,
        "events",
        &format!("sync events {}", chrono::Utc::now().to_rfc3339()),
    )?;
    Ok(())
}


#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct EventAutoSyncSettings {
    startup_pull_enabled: bool,
    interval_enabled: bool,
    interval_minutes: u32,
    close_push_enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct UiAppearanceSettings {
    theme_mode: String,
    background_enabled: bool,
    background_image_data_url: Option<String>,
    background_overlay_opacity: u32,
}

#[tauri::command]
fn get_event_auto_sync_settings(state: State<'_, AppState>) -> Result<EventAutoSyncSettings, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let startup_pull_enabled = get_setting(&conn, "event_sync_startup_pull")
        .map(|v| v == "1")
        .unwrap_or(true);
    let interval_enabled = get_setting(&conn, "event_sync_interval_enabled")
        .map(|v| v == "1")
        .unwrap_or(true);
    let interval_minutes = get_setting(&conn, "event_sync_interval_minutes")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(5)
        .max(1);
    let close_push_enabled = get_setting(&conn, "event_sync_close_push")
        .map(|v| v == "1")
        .unwrap_or(true);
    Ok(EventAutoSyncSettings {
        startup_pull_enabled,
        interval_enabled,
        interval_minutes,
        close_push_enabled,
    })
}

#[tauri::command]
fn set_event_auto_sync_settings(
    state: State<'_, AppState>,
    startup_pull_enabled: bool,
    interval_enabled: bool,
    interval_minutes: u32,
    close_push_enabled: bool,
) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let minutes = interval_minutes.max(1);
    set_setting(&conn, "event_sync_startup_pull", if startup_pull_enabled { "1" } else { "0" })?;
    set_setting(&conn, "event_sync_interval_enabled", if interval_enabled { "1" } else { "0" })?;
    set_setting(&conn, "event_sync_interval_minutes", &minutes.to_string())?;
    set_setting(&conn, "event_sync_close_push", if close_push_enabled { "1" } else { "0" })?;
    Ok(())
}

#[tauri::command]
fn sync_event_pull_only(state: State<'_, AppState>) -> Result<(), String> {
    let _sync_guard = state.sync_lock.lock().map_err(|e| e.to_string())?;
    let router = state.router.lock().map_err(|e| e.to_string())?;
    sqlite_only_sync_guard(router.kind())?;
    let (repo_dir, pending_push, app_dir, device_id) = {
        let conn = state
            .router
            .lock()
            .map_err(|e| e.to_string())?
            .sqlite_conn()?;
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let repo_dir = get_setting(&conn, "git_sync_repo_dir")
            .ok_or_else(|| "请先设置 Git 仓库目录".to_string())?;
        let pending_push = get_setting(&conn, "event_sync_pending_push")
            .map(|v| v == "1")
            .unwrap_or(false);
        let app_dir = app_dir_from_conn(&conn)?;
        let device_id = get_or_create_device_id(&conn);
        (repo_dir, pending_push, app_dir, device_id)
    };
    if !sync::is_git_repo_dir(&repo_dir) {
        return Err("Git 仓库目录无效".to_string());
    }

    sync::git_pull_current_branch(&repo_dir)?;
    {
        let conn = state
            .router
            .lock()
            .map_err(|e| e.to_string())?
            .sqlite_conn()?;
        let mut conn = conn.lock().map_err(|e| e.to_string())?;
        sync_events_from_repo(&mut conn, &repo_dir)?;
    }

    if pending_push {
        match sync_events_to_repo(&repo_dir, &app_dir, &device_id) {
            Ok(_) => {
                let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
                mark_pending_push(&conn, false)?;
            }
            Err(e) => {
                let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
                let _ = mark_pending_push(&conn, true);
                return Err(e);
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn sync_event_push_only(state: State<'_, AppState>) -> Result<(), String> {
    let _sync_guard = state.sync_lock.lock().map_err(|e| e.to_string())?;
    let router = state.router.lock().map_err(|e| e.to_string())?;
    sqlite_only_sync_guard(router.kind())?;
    let (repo_dir, app_dir, device_id) = {
        let conn = state
            .router
            .lock()
            .map_err(|e| e.to_string())?
            .sqlite_conn()?;
        let conn = conn.lock().map_err(|e| e.to_string())?;
        let repo_dir = get_setting(&conn, "git_sync_repo_dir")
            .ok_or_else(|| "请先设置 Git 仓库目录".to_string())?;
        let app_dir = app_dir_from_conn(&conn)?;
        let device_id = get_or_create_device_id(&conn);
        (repo_dir, app_dir, device_id)
    };
    if !sync::is_git_repo_dir(&repo_dir) {
        return Err("Git 仓库目录无效".to_string());
    }
    match sync_events_to_repo(&repo_dir, &app_dir, &device_id) {
        Ok(_) => {
            let conn = state
                .router
                .lock()
                .map_err(|e| e.to_string())?
                .sqlite_conn()?;
            let conn = conn.lock().map_err(|e| e.to_string())?;
            mark_pending_push(&conn, false)?;
            Ok(())
        }
        Err(e) => {
            let conn = state
                .router
                .lock()
                .map_err(|e| e.to_string())?
                .sqlite_conn()?;
            let conn = conn.lock().map_err(|e| e.to_string())?;
            let _ = mark_pending_push(&conn, true);
            Err(e)
        }
    }
}

#[tauri::command]
fn get_ui_appearance_settings(state: State<'_, AppState>) -> Result<UiAppearanceSettings, String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let theme_mode = get_setting(&conn, "ui_theme_mode").unwrap_or_else(|| "system".to_string());
    let background_enabled = get_setting(&conn, "ui_background_enabled")
        .map(|v| v == "1")
        .unwrap_or(false);
    let background_image_data_url = get_setting(&conn, "ui_background_image_data_url");
    let background_overlay_opacity = get_setting(&conn, "ui_background_overlay_opacity")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(45)
        .min(90);
    Ok(UiAppearanceSettings {
        theme_mode,
        background_enabled,
        background_image_data_url,
        background_overlay_opacity,
    })
}

#[tauri::command]
fn set_ui_appearance_settings(
    state: State<'_, AppState>,
    theme_mode: String,
    background_enabled: bool,
    background_image_data_url: Option<String>,
    background_overlay_opacity: u32,
) -> Result<(), String> {
    let conn = state
        .router
        .lock()
        .map_err(|e| e.to_string())?
        .sqlite_conn()?;
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let normalized_theme = match theme_mode.as_str() {
        "light" | "dark" | "system" => theme_mode,
        _ => "system".to_string(),
    };
    let safe_opacity = background_overlay_opacity.min(90);
    set_setting(&conn, "ui_theme_mode", &normalized_theme)?;
    set_setting(
        &conn,
        "ui_background_enabled",
        if background_enabled { "1" } else { "0" },
    )?;
    match background_image_data_url {
        Some(v) => set_setting(&conn, "ui_background_image_data_url", &v)?,
        None => {
            let _ = conn.execute(
                "DELETE FROM app_settings WHERE key = 'ui_background_image_data_url'",
                [],
            );
        }
    }
    set_setting(
        &conn,
        "ui_background_overlay_opacity",
        &safe_opacity.to_string(),
    )?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            let config_dir = app.path().app_config_dir().expect("Failed to get app config dir");
            let cfg = config::load_or_init(&config_dir).expect("Failed to load config");
            let router = DbRouter::init(&cfg, app_data_dir.clone()).expect("Failed to initialize database");
            app.manage(AppState {
                router: Mutex::new(router),
                sync_lock: Mutex::new(()),
                app_data_dir,
                config: Mutex::new(cfg),
                config_dir,
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 阻止默认的立即关闭行为
                api.prevent_close();
                // 隐藏窗口给用户立即关闭的反馈
                let _ = window.hide();
                
                let app_handle = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    let sync_lock_result = state.sync_lock.lock();
                    if let Ok(sync_guard) = sync_lock_result {
                        let context = if let Ok(router) = state.router.lock() {
                            if let Ok(conn_arc) = router.sqlite_conn() {
                                if let Ok(conn) = conn_arc.lock() {
                                    let close_push_enabled = get_setting(&conn, "event_sync_close_push")
                                        .map(|v| v == "1")
                                        .unwrap_or(true);
                                    if close_push_enabled {
                                        get_setting(&conn, "git_sync_repo_dir").map(|repo_dir| {
                                            let app_dir = app_dir_from_conn(&conn);
                                            let device_id = get_or_create_device_id(&conn);
                                            (repo_dir, app_dir, device_id)
                                        })
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if let Some((repo_dir, Ok(app_dir), device_id)) = context {
                            if sync::is_git_repo_dir(&repo_dir) {
                                let push_result = sync_events_to_repo(&repo_dir, &app_dir, &device_id);
                                if let Ok(router) = state.router.lock() {
                                    if let Ok(conn_arc) = router.sqlite_conn() {
                                        if let Ok(conn) = conn_arc.lock() {
                                            match push_result {
                                                Ok(_) => {
                                                    let _ = mark_pending_push(&conn, false);
                                                    append_debug_log(&conn, "close push success");
                                                }
                                                Err(err) => {
                                                    let _ = mark_pending_push(&conn, true);
                                                    append_debug_log(&conn, &format!("close push failed: {err}"));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        drop(sync_guard);
                    }
                    // 同步完成后真正退出应用
                    app_handle.exit(0);
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_config, set_app_config,
            get_bookmarks, add_bookmark, search_bookmarks,
            check_bookmark_exists,
            import_browser_bookmarks, get_folders, get_bookmarks_by_folder,
            update_bookmark, delete_bookmark, create_folder, delete_folder,
            rename_folder, add_bookmark_to_folder, remove_bookmark_from_folder, get_bookmark_folders,
            get_delete_sync_setting, set_delete_sync_setting,
            get_tags, get_bookmarks_by_tag, add_tag_to_bookmark, remove_tag_from_bookmark,
            get_debug_log_path, write_debug_log,
            get_browser_auto_sync_settings, set_browser_auto_sync_settings,
            get_git_sync_repo_dir, set_git_sync_repo_dir, sync_github_incremental,
            get_event_auto_sync_settings, set_event_auto_sync_settings,
            sync_event_pull_only, sync_event_push_only,
            get_ui_appearance_settings, set_ui_appearance_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
