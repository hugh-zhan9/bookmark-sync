pub mod db;
pub mod events;
pub mod sync;


use std::sync::Mutex;
use std::thread;
use tauri::{Manager, State, Emitter};
use sync::{credentials, init_or_open_repo, commit_all};
use events::models::{BookmarkPayload, SyncEvent, EventLog};
use events::replay_events;
use events::cleaner;
use events::metadata;
use rusqlite::params;

struct DbState {
    conn: Mutex<rusqlite::Connection>,
}

#[tauri::command]
fn get_bookmarks(state: State<'_, DbState>) -> Result<Vec<BookmarkPayload>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    
    let mut stmt = conn.prepare("SELECT id, url, title, description, favicon_url, host, created_at FROM bookmarks WHERE is_deleted = 0 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
        
    let iter = stmt.query_map([], |row| {
        Ok(BookmarkPayload {
            id: row.get(0)?,
            url: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            favicon_url: row.get(4)?,
            host: row.get(5)?,
            created_at: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?;
    
    let mut bookmarks = Vec::new();
    for b in iter {
        if let Ok(bookmark) = b {
            bookmarks.push(bookmark);
        }
    }
    
    Ok(bookmarks)
}

#[tauri::command]
fn search_bookmarks(state: State<'_, DbState>, query: String) -> Result<Vec<BookmarkPayload>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    
    let mut stmt = conn.prepare(
        "SELECT b.id, b.url, b.title, b.description, b.favicon_url, b.host, b.created_at 
         FROM bookmarks b
         JOIN bookmarks_fts fts ON b.rowid = fts.rowid
         WHERE bookmarks_fts MATCH ?1 AND b.is_deleted = 0
         ORDER BY fts.rank"
    ).map_err(|e| e.to_string())?;
        
    let iter = stmt.query_map([&query], |row| {
        Ok(BookmarkPayload {
            id: row.get(0)?,
            url: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            favicon_url: row.get(4)?,
            host: row.get(5)?,
            created_at: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?;
    
    let mut bookmarks = Vec::new();
    for b in iter {
        if let Ok(bookmark) = b {
            bookmarks.push(bookmark);
        }
    }
    
    Ok(bookmarks)
}

#[tauri::command]
fn add_bookmark(state: State<'_, DbState>, mut payload: BookmarkPayload) -> Result<(), String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    
    // Clean tracking params
    payload.url = cleaner::clean_url(&payload.url);
    let new_host = payload.host.clone().unwrap_or_default();
    
    // In a real CQRS system, you only persist the event to the Event Log,
    // and a separate reactor processes the logs and writes to `bookmarks` table.
    // For simplicity of [M1], we directly run replay_events on an synthetic log here.
    // Prepare background fetch params before payload moves
    let url_to_fetch = payload.url.clone();
    let bm_id = payload.id.clone();
    
    let event_log = EventLog {
        event_id: uuid::Uuid::new_v4().to_string(),
        device_id: "local_device".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        event: SyncEvent::BookmarkAdded(payload),
    };

    replay_events(&mut conn, vec![event_log]).map_err(|e| e.to_string())?;
    
    // Spawn background task to fetch metadata
    let db_path = state.conn.lock().unwrap().path().map(|p| p.to_string());
    
    if let Some(path) = db_path {
        thread::spawn(move || {
            if let Ok(meta) = metadata::fetch_metadata(&url_to_fetch) {
                if let Ok(background_conn) = rusqlite::Connection::open(path) {
                    let title = meta.title.unwrap_or(format!("Mock Title for {}", new_host));
                    let favicon = meta.favicon_url.unwrap_or_default();
                    
                    let _ = background_conn.execute(
                        "UPDATE bookmarks SET title = ?1, favicon_url = ?2 WHERE id = ?3",
                        params![title, favicon, bm_id]
                    );
                    println!("Async meta synced for {}", bm_id);
                }
            }
        });
    }

    Ok(())
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn save_credentials(repo_url: String, token: String) -> Result<(), String> {
    credentials::save_credentials(&repo_url, &token)
}

#[tauri::command]
fn trigger_sync(app: tauri::AppHandle) -> Result<String, String> {
    // 1. Get credentials
    let (repo_url, token) = credentials::get_credentials()?;
    
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    
    // 2. Open or Clone repo
    let repo = init_or_open_repo(&app_data_dir, &repo_url, &token)?;

    // 3. (Mock) Write some new event logs to the repo
    let events_dir = app_data_dir.join("sync-repo").join("events");
    std::fs::create_dir_all(&events_dir).map_err(|e| e.to_string())?;
    
    let dbg_file = events_dir.join(format!("{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&dbg_file, "{\"mock\": \"event_data\"}").map_err(|e| e.to_string())?;

    // 4. Commit and Push (Push is omitted below for brevity in M3, uses git push equivalent)
    commit_all(&repo, "Sync incoming local bookmarks.")?;

    Ok("Synced successfully".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            let conn = db::init_db(app_data_dir).expect("Failed to initialize database");
            
            // Manage DbState globally 
            app.manage(DbState {
                conn: Mutex::new(conn),
            });
            
            // Spawn Native Messaging Observer Loop
            let app_handle = app.handle().clone();
            thread::spawn(move || {
                loop {
                    match events::native_messaging::read_message() {
                        Ok(Some(msg)) => {
                            // Optionally broadcast to JS frontend for realtime UI updates
                            let _ = app_handle.emit("native-message", &msg);
                            
                            // Here you'd parse `msg` into SyncEvent and persist it via DbState
                            println!("NativeMsg: {:?}", msg);
                        },
                        Ok(None) => break, // EOF means Chrome disconnected
                        Err(e) => {
                            eprintln!("NativeMsg Error: {}", e);
                            break;
                        }
                    }
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet, get_bookmarks, add_bookmark,
            search_bookmarks,
            save_credentials, trigger_sync
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
