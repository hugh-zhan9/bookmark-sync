use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum SyncEvent {
    BookmarkAdded(BookmarkPayload),
    BookmarkDeleted { id: String },
    BookmarkUpdated(BookmarkPayload),
    TagAdded { id: String, name: String },
    TagDeleted { id: String },
    BookmarkTagged { bookmark_id: String, tag_id: String },
    BookmarkUntagged { bookmark_id: String, tag_id: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BookmarkPayload {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub favicon_url: Option<String>,
    pub host: Option<String>,
    pub created_at: String, // ISO 8601
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventLog {
    /// A globally unique identifier for this event
    pub event_id: String,
    /// Client ID that generated the event
    pub device_id: String,
    /// The event timestamp in ms
    pub timestamp: i64,
    /// The actual operation
    pub event: SyncEvent,
}
