# Data Source Toggle Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在本地 SQLite 与本地 PostgreSQL 之间切换数据源，SQLite 模式保留 Git 同步，PostgreSQL 模式禁用同步，切换不迁移旧数据源。

**Architecture:** 引入配置文件与数据源路由层，按配置初始化 SQLite 或 PostgreSQL 连接；所有 Tauri 命令通过路由层调用对应数据库实现，Git 同步仅在 SQLite 模式可用。

**Tech Stack:** Rust (Tauri v2), rusqlite, postgres + r2d2_postgres, serde, Vitest

---

### Task 1: 配置文件读写（AppConfig）

**Files:**
- Create: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/config.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/config.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_or_init_should_create_default_config() {
        let dir = tempdir().expect("tmp dir");
        let cfg = load_or_init(dir.path()).expect("load_or_init");
        assert_eq!(cfg.data_source, DataSourceKind::Sqlite);
        let on_disk = load_or_init(dir.path()).expect("load_or_init again");
        assert_eq!(on_disk.data_source, DataSourceKind::Sqlite);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml load_or_init_should_create_default_config -v`
Expected: FAIL with missing symbols in `config.rs`

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/config.rs
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataSourceKind {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub db: String,
    pub user: String,
    pub password: String,
    pub sslmode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub data_source: DataSourceKind,
    pub postgres: PostgresConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            data_source: DataSourceKind::Sqlite,
            postgres: PostgresConfig {
                host: "127.0.0.1".into(),
                port: 5432,
                db: "bookmark_sync".into(),
                user: "bookmark".into(),
                password: "".into(),
                sslmode: "prefer".into(),
            },
        }
    }
}

pub fn config_path(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

pub fn load_or_init(dir: &Path) -> Result<AppConfig, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = config_path(dir);
    if !path.exists() {
        let cfg = AppConfig::default();
        save(dir, &cfg)?;
        return Ok(cfg);
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let cfg: AppConfig = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(cfg)
}

pub fn save(dir: &Path, cfg: &AppConfig) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = config_path(dir);
    let raw = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())?;
    Ok(())
}

// src-tauri/src/lib.rs
pub mod config;
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml load_or_init_should_create_default_config -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/config.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Feature" "新增应用配置文件读写与默认配置" "配置文件路径与权限问题可能导致读取失败" "S2" "src-tauri/src/config.rs"
git commit -m "feat: add app config loader"
```

---

### Task 2: 数据源路由与 State 重构（SQLite 先行）

**Files:**
- Create: `src-tauri/src/db/router.rs`
- Modify: `src-tauri/src/db/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db/router.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/db/router.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, DataSourceKind};
    use tempfile::tempdir;

    #[test]
    fn router_should_init_sqlite_by_default() {
        let dir = tempdir().expect("tmp dir");
        let cfg = AppConfig::default();
        let router = DbRouter::init(&cfg, dir.path().to_path_buf()).expect("init");
        assert_eq!(router.kind(), DataSourceKind::Sqlite);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml router_should_init_sqlite_by_default -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/db/router.rs
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use crate::config::{AppConfig, DataSourceKind};
use crate::db;

pub struct DbRouter {
    kind: DataSourceKind,
    sqlite: Option<Arc<Mutex<Connection>>>,
    app_data_dir: PathBuf,
}

impl DbRouter {
    pub fn init(cfg: &AppConfig, app_data_dir: PathBuf) -> Result<Self, String> {
        let mut router = Self {
            kind: cfg.data_source,
            sqlite: None,
            app_data_dir,
        };
        router.reinit(cfg)?;
        Ok(router)
    }

    pub fn kind(&self) -> DataSourceKind {
        self.kind
    }

    pub fn app_data_dir(&self) -> &PathBuf {
        &self.app_data_dir
    }

    pub fn reinit(&mut self, cfg: &AppConfig) -> Result<(), String> {
        self.kind = cfg.data_source;
        match cfg.data_source {
            DataSourceKind::Sqlite => {
                let conn = db::init_db(self.app_data_dir.clone()).map_err(|e| e.to_string())?;
                self.sqlite = Some(Arc::new(Mutex::new(conn)));
            }
            DataSourceKind::Postgres => {
                self.sqlite = None;
                return Err("postgres not initialized".into());
            }
        }
        Ok(())
    }

    pub fn sqlite_conn(&self) -> Result<Arc<Mutex<Connection>>, String> {
        self.sqlite
            .as_ref()
            .ok_or_else(|| "sqlite unavailable".to_string())
            .map(Arc::clone)
    }
}
```

```rust
// src-tauri/src/lib.rs
use crate::config::{self, AppConfig};
use crate::db::router::DbRouter;

struct AppState {
    router: Mutex<DbRouter>,
    sync_lock: Mutex<()>,
    config: Mutex<AppConfig>,
    config_dir: PathBuf,
    app_data_dir: PathBuf,
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
                config: Mutex::new(cfg),
                config_dir,
                app_data_dir,
            });
            Ok(())
        })
        // 其余 run() 保持不变
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml router_should_init_sqlite_by_default -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/db/router.rs src-tauri/src/db/mod.rs src-tauri/src/lib.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Refactor" "引入数据源路由骨架与 State 结构" "后续接入 PG 后切换流程可能引入状态不一致" "S2" "src-tauri/src/db/router.rs,src-tauri/src/db/mod.rs,src-tauri/src/lib.rs"
git commit -m "refactor: add db router skeleton"
```

---

### Task 3: PostgreSQL 连接与建表

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/db/postgres.rs`
- Modify: `src-tauri/src/db/router.rs`
- Test: `src-tauri/src/db/postgres.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/db/postgres.rs
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
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml build_dsn_should_format_connection_string -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```toml
# src-tauri/Cargo.toml
postgres = "0.19"
r2d2 = "0.8"
r2d2_postgres = "0.18"
```

```rust
// src-tauri/src/db/postgres.rs
use postgres::{Client, NoTls};
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use crate::config::PostgresConfig;

pub fn build_dsn(host: &str, port: u16, db: &str, user: &str, password: &str, sslmode: &str) -> String {
    format!(
        "host={} port={} dbname={} user={} password={} sslmode={}",
        host, port, db, user, password, sslmode
    )
}

pub fn init_db(cfg: &PostgresConfig) -> Result<Pool<PostgresConnectionManager<NoTls>>, String> {
    let dsn = build_dsn(&cfg.host, cfg.port, &cfg.db, &cfg.user, &cfg.password, &cfg.sslmode);
    let manager = PostgresConnectionManager::new(dsn.parse().map_err(|e| e.to_string())?, NoTls);
    let pool = Pool::new(manager).map_err(|e| e.to_string())?;
    let mut client = pool.get().map_err(|e| e.to_string())?;
    create_tables(&mut client)?;
    Ok(pool)
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
```

```rust
// src-tauri/src/db/router.rs
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use postgres::NoTls;
use crate::db::postgres;

pub struct DbRouter {
    kind: DataSourceKind,
    sqlite: Option<Arc<Mutex<Connection>>>,
    pg: Option<Pool<PostgresConnectionManager<NoTls>>>,
    app_data_dir: PathBuf,
}

pub fn reinit(&mut self, cfg: &AppConfig) -> Result<(), String> {
    self.kind = cfg.data_source;
    match cfg.data_source {
        DataSourceKind::Sqlite => {
            let conn = db::init_db(self.app_data_dir.clone()).map_err(|e| e.to_string())?;
            self.sqlite = Some(Arc::new(Mutex::new(conn)));
            self.pg = None;
        }
        DataSourceKind::Postgres => {
            let pool = postgres::init_db(&cfg.postgres)?;
            self.pg = Some(pool);
            self.sqlite = None;
        }
    }
    Ok(())
}

pub fn pg_pool(&self) -> Result<&Pool<PostgresConnectionManager<NoTls>>, String> {
    self.pg.as_ref().ok_or_else(|| "postgres unavailable".to_string())
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml build_dsn_should_format_connection_string -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/db/postgres.rs src-tauri/src/db/router.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Feature" "新增 PostgreSQL 连接与建表" "连接字符串与权限配置错误会导致初始化失败" "S2" "src-tauri/Cargo.toml,src-tauri/src/db/postgres.rs,src-tauri/src/db/router.rs"
git commit -m "feat: add postgres init"
```

---

### Task 4: 抽象数据库操作接口（SQLite/PG 双实现）

**Files:**
- Create: `src-tauri/src/db/store.rs`
- Modify: `src-tauri/src/db/sqlite.rs`
- Modify: `src-tauri/src/db/postgres.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db/store.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/db/store.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_store_trait_should_exist() {
        fn _assert(_s: &dyn BookmarkStore) {}
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml bookmark_store_trait_should_exist -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/db/store.rs
use crate::config::DataSourceKind;
use crate::events::models::EventLog;
use crate::events::metadata::SiteMetadata;
use crate::events::models::BookmarkPayload;

pub trait BookmarkStore {
    fn get_bookmarks(&self) -> Result<Vec<BookmarkPayload>, String>;
    fn search_bookmarks(&self, query: &str) -> Result<Vec<BookmarkPayload>, String>;
    fn apply_event(&self, log: &EventLog) -> Result<(), String>;
    fn apply_event_if_new(&self, log: &EventLog) -> Result<bool, String>;
    fn resolve_bookmark_id_for_url(&self, url: &str, fallback_id: &str) -> String;
    fn apply_metadata_by_canonical_url(&self, canonical_url: &str, meta: &SiteMetadata) -> Result<usize, String>;
    fn is_bookmark_logically_deleted_by_canonical_url(&self, canonical_url: &str) -> Result<bool, String>;
    fn is_folder_logically_deleted_by_id(&self, folder_id: &str) -> Result<bool, String>;
    fn get_setting(&self, key: &str) -> Result<Option<String>, String>;
    fn set_setting(&self, key: &str, value: &str) -> Result<(), String>;
    fn mark_pending_push(&self, pending: bool) -> Result<(), String>;
}

pub use crate::config::DataSourceKind;
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml data_source_kind_should_be_copy -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/db/store.rs src-tauri/src/db/sqlite.rs src-tauri/src/db/postgres.rs src-tauri/src/lib.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Refactor" "抽象数据库操作接口以支持多数据源" "接口不完整会导致运行期缺少实现" "S2" "src-tauri/src/db/store.rs,src-tauri/src/db/sqlite.rs,src-tauri/src/db/postgres.rs,src-tauri/src/lib.rs"
git commit -m "refactor: add db store trait"
```

---

### Task 5: SQLite 实现迁移到 store 接口

**Files:**
- Modify: `src-tauri/src/db/sqlite.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db/sqlite.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/db/sqlite.rs
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    #[test]
    fn sqlite_store_should_get_bookmarks_empty() {
        let conn = Connection::open_in_memory().expect("mem");
        create_tables(&conn).expect("tables");
        let store = SqliteStore::new(Arc::new(Mutex::new(conn)));
        let res = store.get_bookmarks().expect("get");
        assert!(res.is_empty());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sqlite_store_should_get_bookmarks_empty -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/db/sqlite.rs
use rusqlite::{Connection, params, params_from_iter};
use std::sync::{Arc, Mutex};
use crate::events::models::{EventLog, BookmarkPayload, SyncEvent};
use crate::events::metadata::SiteMetadata;
use crate::db::store::BookmarkStore;
use crate::events::models::SyncEvent;

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
    Ok(BookmarkPayload {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        favicon_url: row.get(4)?,
        host: row.get(5)?,
        created_at: row.get(6)?,
        tags: tag_list
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
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
```

```rust
// src-tauri/src/lib.rs
pub(crate) fn tokenize_search_query(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in query.split_whitespace() {
        let t = token.trim().to_lowercase();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

pub(crate) fn search_clause_for_param(param_index: usize) -> String {
    let p = param_index + 1;
    format!(
        "(b.title LIKE ?{p} OR b.host LIKE ?{p} OR EXISTS (SELECT 1 FROM tags t JOIN bookmark_tags bt ON t.id = bt.tag_id WHERE bt.bookmark_id = b.id AND t.name LIKE ?{p}))"
    )
}

pub(crate) fn resolve_bookmark_id_for_url(
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

pub(crate) fn is_bookmark_logically_deleted_by_canonical_url(
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

pub(crate) fn is_folder_logically_deleted_by_id(
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

pub(crate) fn apply_metadata_by_canonical_url(
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
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sqlite_store_should_get_bookmarks_empty -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/db/sqlite.rs src-tauri/src/lib.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Refactor" "SQLite 存储迁移到统一接口" "接口迁移遗漏会导致运行期报错" "S2" "src-tauri/src/db/sqlite.rs,src-tauri/src/lib.rs"
git commit -m "refactor: move sqlite store"
```

---

### Task 6: PostgreSQL Store 实现与路由切换

**Files:**
- Modify: `src-tauri/src/db/postgres.rs`
- Modify: `src-tauri/src/db/router.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db/postgres.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/db/postgres.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg_search_sql_should_use_string_agg() {
        let sql = bookmark_select_sql();
        assert!(sql.contains("STRING_AGG"));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml pg_search_sql_should_use_string_agg -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/db/postgres.rs
use postgres::{Client, Row};
use postgres::types::ToSql;
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use postgres::NoTls;
use crate::db::store::BookmarkStore;
use crate::events::models::{EventLog, BookmarkPayload};
use crate::events::metadata::SiteMetadata;

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
    BookmarkPayload {
        id: row.get(0),
        url: row.get(1),
        title: row.get(2),
        description: row.get(3),
        favicon_url: row.get(4),
        host: row.get(5),
        created_at: row.get(6),
        tags: tag_list
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
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
                    \"INSERT INTO bookmarks (id, url, canonical_url, title, description, favicon_url, host, created_at)\n                     VALUES ($1, $2, $2, $3, $4, $5, $6, $7)\n                     ON CONFLICT (canonical_url) DO UPDATE SET title = EXCLUDED.title, is_deleted = FALSE, updated_at = CURRENT_TIMESTAMP\",
                    &[&b.id, &b.url, &b.title, &b.description, &b.favicon_url, &b.host, &b.created_at],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::FolderAdded { id, parent_id, name } => {
                client.execute(
                    \"INSERT INTO folders (id, parent_id, name) VALUES ($1, $2, $3)\n                     ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, parent_id = EXCLUDED.parent_id\",
                    &[id, parent_id, name],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkDeleted { id } => {
                client.execute(
                    \"UPDATE bookmarks SET is_deleted = TRUE, updated_at = CURRENT_TIMESTAMP WHERE id = $1\",
                    &[id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkUpdated(b) => {
                client.execute(
                    \"UPDATE bookmarks SET title = $1, url = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $3\",
                    &[&b.title, &b.url, &b.id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::TagAdded { id, name } => {
                client.execute(
                    \"INSERT INTO tags (id, name) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING\",
                    &[id, name],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkTagged { bookmark_id, tag_id } => {
                client.execute(
                    \"INSERT INTO bookmark_tags (bookmark_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING\",
                    &[bookmark_id, tag_id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkUntagged { bookmark_id, tag_id } => {
                client.execute(
                    \"DELETE FROM bookmark_tags WHERE bookmark_id = $1 AND tag_id = $2\",
                    &[bookmark_id, tag_id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::FolderRenamed { id, name } => {
                client.execute(
                    \"UPDATE folders SET name = $2 WHERE id = $1\",
                    &[id, name],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::FolderDeleted { id } => {
                client.execute(
                    \"UPDATE folders SET is_deleted = TRUE WHERE id = $1\",
                    &[id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkAddedToFolder { bookmark_id, folder_id } => {
                client.execute(
                    \"INSERT INTO folder_bookmarks (folder_id, bookmark_id) VALUES ($1, $2) ON CONFLICT DO NOTHING\",
                    &[folder_id, bookmark_id],
                ).map_err(|e| e.to_string())?;
            }
            SyncEvent::BookmarkRemovedFromFolder { bookmark_id, folder_id } => {
                client.execute(
                    \"DELETE FROM folder_bookmarks WHERE folder_id = $1 AND bookmark_id = $2\",
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
            \"INSERT INTO applied_event_ids (event_id) VALUES ($1) ON CONFLICT (event_id) DO NOTHING\",
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
            \"SELECT id FROM bookmarks WHERE canonical_url = $1 LIMIT 1\",
            &[&cleaned_url],
        ).map_err(|e| e.to_string())?;
        Ok(row.map(|r| r.get::<_, String>(0)).unwrap_or_else(|| fallback_id.to_string()))
    }).unwrap_or_else(|_| fallback_id.to_string())
}

fn apply_metadata_by_canonical_url_pg(store: &PostgresStore, canonical_url: &str, meta: &SiteMetadata) -> Result<usize, String> {
    store.with_client(|client| {
        let updated = client.execute(
            \"UPDATE bookmarks SET title = $1, favicon_url = $2, updated_at = CURRENT_TIMESTAMP WHERE canonical_url = $3\",
            &[&meta.title, &meta.favicon_url, &canonical_url],
        ).map_err(|e| e.to_string())?;
        Ok(updated as usize)
    })
}

fn is_bookmark_logically_deleted_by_canonical_url_pg(store: &PostgresStore, canonical_url: &str) -> Result<bool, String> {
    store.with_client(|client| {
        let row = client.query_opt(
            \"SELECT is_deleted FROM bookmarks WHERE canonical_url = $1 LIMIT 1\",
            &[&canonical_url],
        ).map_err(|e| e.to_string())?;
        Ok(row.map(|r| r.get::<_, bool>(0)).unwrap_or(false))
    })
}

fn is_folder_logically_deleted_by_id_pg(store: &PostgresStore, folder_id: &str) -> Result<bool, String> {
    store.with_client(|client| {
        let row = client.query_opt(
            \"SELECT is_deleted FROM folders WHERE id = $1 LIMIT 1\",
            &[&folder_id],
        ).map_err(|e| e.to_string())?;
        Ok(row.map(|r| r.get::<_, bool>(0)).unwrap_or(false))
    })
}

fn get_setting_pg(store: &PostgresStore, key: &str) -> Result<Option<String>, String> {
    store.with_client(|client| {
        let row = client.query_opt(
            \"SELECT value FROM app_settings WHERE key = $1 LIMIT 1\",
            &[&key],
        ).map_err(|e| e.to_string())?;
        Ok(row.map(|r| r.get::<_, String>(0)))
    })
}

fn set_setting_pg(store: &PostgresStore, key: &str, value: &str) -> Result<(), String> {
    store.with_client(|client| {
        client.execute(
            \"INSERT INTO app_settings (key, value) VALUES ($1, $2)\n             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value\",
            &[&key, &value],
        ).map_err(|e| e.to_string())?;
        Ok(())
    })
}

fn mark_pending_push_pg(store: &PostgresStore, pending: bool) -> Result<(), String> {
    set_setting_pg(store, \"event_sync_pending_push\", if pending { \"1\" } else { \"0\" })
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml pg_search_sql_should_use_string_agg -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/db/postgres.rs src-tauri/src/db/router.rs src-tauri/src/lib.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Feature" "实现 Postgres Store 并接入路由" "SQL 语法差异可能导致运行期失败" "S2" "src-tauri/src/db/postgres.rs,src-tauri/src/db/router.rs,src-tauri/src/lib.rs"
git commit -m "feat: add postgres store"
```

---

### Task 7: Tauri 命令改造与 Git 同步开关

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/sync/mod.rs`
- Test: `src-tauri/src/lib.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/lib.rs
#[cfg(test)]
mod data_source_tests {
    use super::*;

    #[test]
    fn sqlite_only_sync_should_block_pg() {
        let res = sqlite_only_sync_guard(DataSourceKind::Postgres);
        assert!(res.is_err());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sqlite_only_sync_should_block_pg -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/lib.rs
fn sqlite_only_sync_guard(kind: DataSourceKind) -> Result<(), String> {
    match kind {
        DataSourceKind::Sqlite => Ok(()),
        DataSourceKind::Postgres => Err("Git 同步仅支持 SQLite 数据源".into()),
    }
}
```

在所有 sync 入口（如 `sync_event_push_only` / `sync_event_pull_only` / `sync_github_incremental`）中添加：

```rust
let router = state.router.lock().map_err(|e| e.to_string())?;
sqlite_only_sync_guard(router.kind())?;
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sqlite_only_sync_should_block_pg -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/sync/mod.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Feature" "Git 同步仅在 SQLite 模式可用" "错误分支可能导致同步被意外阻断" "S2" "src-tauri/src/lib.rs,src-tauri/src/sync/mod.rs"
git commit -m "feat: gate sync by data source"
```

---

### Task 8: 数据源切换命令与配置写回

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/config.rs`
- Test: `src-tauri/src/lib.rs`

**Step 1: Write the failing test**

```rust
// src-tauri/src/lib.rs
#[cfg(test)]
mod switch_tests {
    use super::*;
    use crate::config::{AppConfig, DataSourceKind};

    #[test]
    fn switch_should_reject_invalid_pg_config() {
        let mut cfg = AppConfig::default();
        cfg.data_source = DataSourceKind::Postgres;
        cfg.postgres.host = "".into();
        let err = validate_pg_config(&cfg).unwrap_err();
        assert!(err.contains("postgres host"));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml switch_should_reject_invalid_pg_config -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```rust
// src-tauri/src/lib.rs
fn validate_pg_config(cfg: &AppConfig) -> Result<(), String> {
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
fn get_app_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    Ok(cfg)
}

#[tauri::command]
fn set_app_config(state: State<'_, AppState>, next: AppConfig) -> Result<(), String> {
    if next.data_source == DataSourceKind::Postgres {
        validate_pg_config(&next)?;
    }
    let mut router = state.router.lock().map_err(|e| e.to_string())?;
    router.reinit(&next)?;
    config::save(&state.config_dir, &next)?;
    *state.config.lock().map_err(|e| e.to_string())? = next;
    Ok(())
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml switch_should_reject_invalid_pg_config -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/config.rs
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Feature" "新增数据源切换命令与配置校验" "切换失败会导致状态与配置不一致" "S2" "src-tauri/src/lib.rs,src-tauri/src/config.rs"
git commit -m "feat: add data source switch command"
```

---

### Task 9: 回归测试与文档

**Files:**
- Modify: `README.md`
- Test: `src/App.test.tsx`

**Step 1: Write the failing test**

```ts
// src/App.test.tsx
// 在 invoke mock 中加入新命令的默认处理
if (cmd === 'get_app_config') {
  return Promise.resolve({ data_source: 'sqlite', postgres: { host: '127.0.0.1', port: 5432, db: 'bookmark_sync', user: 'bookmark', password: '', sslmode: 'prefer' } });
}
```

**Step 2: Run test to verify it fails**

Run: `npm run test`
Expected: FAIL if mock missing new command

**Step 3: Write minimal implementation**

```md
# README.md
增加“配置文件路径与数据源开关说明”段落，提示明文密码风险与切换不迁移。
```

**Step 4: Run test to verify it passes**

Run: `npm run test`
Expected: PASS

**Step 5: Commit**

```bash
git add README.md src/App.test.tsx
python3 "/Users/zhangyukun/.codex/skills/flight-recorder/scripts/log_change.py" "Docs" "更新数据源配置与测试说明" "文档遗漏可能导致用户误操作" "S3" "README.md,src/App.test.tsx"
git commit -m "docs: add data source config notes"
```

---

## Final Verification

Run:
- `npm run test`
- `cargo test --manifest-path src-tauri/Cargo.toml`

Expected:
- All tests pass.
