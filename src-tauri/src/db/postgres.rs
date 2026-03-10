use postgres::Client;
use r2d2_postgres::postgres::tls::NoTls;
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
    let manager = PostgresConnectionManager::new(dsn.parse::<postgres::Config>().map_err(|e| e.to_string())?, NoTls);
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
