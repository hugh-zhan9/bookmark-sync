use rusqlite::Connection;
use rusqlite::params;
use serde_json::Value;
use chrono::Utc;
use uuid::Uuid;
use crate::events::cleaner;

pub fn apply_native_message(conn: &Connection, msg: &Value) -> Result<bool, String> {
    let msg_type = msg
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "native message missing type".to_string())?;

    match msg_type {
        "BookmarkAdded" | "PageCaptured" => {
            let payload = msg.get("payload").unwrap_or(&Value::Null);
            let id = payload.get("id").and_then(|v| v.as_str());
            let url = payload.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let title = payload.get("title").and_then(|v| v.as_str());
            if upsert_bookmark(conn, id, url, title)? {
                return Ok(true);
            }
            Ok(false)
        }
        "FullSync" => {
            let mut changed = false;
            let bookmarks = msg
                .get("payload")
                .and_then(|p| p.get("bookmarks"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for bookmark in bookmarks {
                let id = bookmark.get("id").and_then(|v| v.as_str());
                let url = bookmark.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let title = bookmark.get("title").and_then(|v| v.as_str());
                if upsert_bookmark(conn, id, url, title)? {
                    changed = true;
                }
            }
            Ok(changed)
        }
        "BookmarkDeleted" => {
            let payload = msg.get("payload").unwrap_or(&Value::Null);
            let mut affected = 0;
            if let Some(id) = payload.get("id").and_then(|v| v.as_str()) {
                if !id.is_empty() {
                    affected += conn
                        .execute("UPDATE bookmarks SET is_deleted = 1, updated_at = ?1 WHERE id = ?2", params![Utc::now().to_rfc3339(), id])
                        .map_err(|e| e.to_string())?;
                }
            }
            if affected == 0 {
                if let Some(url) = payload.get("url").and_then(|v| v.as_str()) {
                    if !url.is_empty() {
                        affected += conn
                            .execute("UPDATE bookmarks SET is_deleted = 1, updated_at = ?1 WHERE url = ?2", params![Utc::now().to_rfc3339(), url])
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            Ok(affected > 0)
        }
        "BookmarkUpdated" => {
            let payload = msg.get("payload").unwrap_or(&Value::Null);
            let id = payload.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() {
                return Ok(false);
            }

            let mut changed = false;
            if let Some(title) = payload.get("title").and_then(|v| v.as_str()) {
                let affected = conn
                    .execute(
                        "UPDATE bookmarks SET title = ?1, updated_at = ?2 WHERE id = ?3",
                        params![title, Utc::now().to_rfc3339(), id],
                    )
                    .map_err(|e| e.to_string())?;
                changed = changed || affected > 0;
            }

            if let Some(url) = payload.get("url").and_then(|v| v.as_str()) {
                if !url.is_empty() {
                    let cleaned_url = cleaner::clean_url(url);
                    let host = extract_host(&cleaned_url);
                    let affected = conn
                        .execute(
                            "UPDATE bookmarks SET url = ?1, canonical_url = ?2, host = ?3, updated_at = ?4 WHERE id = ?5",
                            params![cleaned_url, cleaned_url, host, Utc::now().to_rfc3339(), id],
                        )
                        .map_err(|e| e.to_string())?;
                    changed = changed || affected > 0;
                }
            }
            Ok(changed)
        }
        _ => Ok(false),
    }
}

fn upsert_bookmark(conn: &Connection, id: Option<&str>, raw_url: &str, title: Option<&str>) -> Result<bool, String> {
    if raw_url.is_empty() {
        return Ok(false);
    }

    let url = normalize_for_dedupe(&cleaner::clean_url(raw_url));
    if url.is_empty() {
        return Ok(false);
    }

    let bookmark_id = id.filter(|v| !v.is_empty()).unwrap_or_else(|| {
        // Keep id stable when source doesn't provide it.
        ""
    });
    let final_id = if bookmark_id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        bookmark_id.to_string()
    };
    let now = Utc::now().to_rfc3339();
    let host = extract_host(&url);

    conn.execute(
        "INSERT INTO bookmarks (id, url, canonical_url, title, host, created_at, updated_at, is_deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 0)
         ON CONFLICT(url) DO UPDATE SET
           canonical_url = excluded.canonical_url,
           title = excluded.title,
           host = excluded.host,
           updated_at = excluded.updated_at,
           is_deleted = 0",
        params![
            final_id,
            url,
            url,
            title.unwrap_or(raw_url),
            host,
            now
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(true)
}

fn extract_host(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) => parsed.host_str().unwrap_or("").to_string(),
        Err(_) => String::new(),
    }
}

fn normalize_for_dedupe(input: &str) -> String {
    match url::Url::parse(input) {
        Ok(parsed) => {
            let is_root = parsed.path() == "/" && parsed.query().is_none();
            let mut out = parsed.to_string();
            if is_root && out.ends_with('/') {
                out.pop();
            }
            out
        }
        Err(_) => input.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use serde_json::json;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE bookmarks (
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
            "#,
        )
        .expect("create schema");
        conn
    }

    #[test]
    fn full_sync_upserts_and_recovers_deleted_rows() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO bookmarks (id,url,canonical_url,title,host,is_deleted) VALUES (?1,?2,?3,?4,?5,1)",
            params!["old-id", "https://example.com", "https://example.com", "old", "example.com"],
        )
        .expect("seed row");

        let msg = json!({
            "type": "FullSync",
            "payload": {
                "bookmarks": [
                    { "id": "new-id", "url": "https://example.com", "title": "new title" },
                    { "id": "v2ex-id", "url": "https://v2ex.com", "title": "v2ex" }
                ]
            }
        });

        let changed = apply_native_message(&conn, &msg).expect("apply full sync");
        assert!(changed);

        let row: (String, String, i64) = conn
            .query_row(
                "SELECT id, title, is_deleted FROM bookmarks WHERE url = ?1",
                params!["https://example.com"],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("fetch updated row");
        assert_eq!(row.0, "old-id");
        assert_eq!(row.1, "new title");
        assert_eq!(row.2, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM bookmarks", [], |r| r.get(0))
            .expect("count rows");
        assert_eq!(count, 2);
    }

    #[test]
    fn page_captured_is_idempotent_by_url() {
        let conn = setup_conn();

        let msg = json!({
            "type": "PageCaptured",
            "payload": {
                "id": "tab-1",
                "url": "https://news.ycombinator.com",
                "title": "HN"
            }
        });

        assert!(apply_native_message(&conn, &msg).expect("first capture"));
        assert!(apply_native_message(&conn, &msg).expect("second capture"));

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bookmarks WHERE url = ?1",
                params!["https://news.ycombinator.com"],
                |r| r.get(0),
            )
            .expect("count dedup rows");
        assert_eq!(count, 1);
    }

    #[test]
    fn bookmark_deleted_marks_row_deleted() {
        let conn = setup_conn();

        let added = json!({
            "type": "BookmarkAdded",
            "payload": {
                "id": "b-1",
                "url": "https://rust-lang.org",
                "title": "Rust"
            }
        });
        assert!(apply_native_message(&conn, &added).expect("insert added"));

        let deleted = json!({
            "type": "BookmarkDeleted",
            "payload": {
                "id": "b-1"
            }
        });
        assert!(apply_native_message(&conn, &deleted).expect("mark deleted"));

        let deleted_flag: i64 = conn
            .query_row(
                "SELECT is_deleted FROM bookmarks WHERE id = ?1",
                params!["b-1"],
                |r| r.get(0),
            )
            .expect("read delete flag");
        assert_eq!(deleted_flag, 1);
    }
}
