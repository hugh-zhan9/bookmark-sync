use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum SyncEvent {
    BookmarkAdded(BookmarkPayload),
    BookmarkDeleted { id: String },
    BookmarkUpdated(BookmarkPayload),
    FolderAdded { id: String, parent_id: Option<String>, name: String },
    FolderDeleted { id: String },
    FolderRenamed { id: String, name: String },
    TagAdded { id: String, name: String },
    TagDeleted { id: String },
    BookmarkTagged { bookmark_id: String, tag_id: String },
    BookmarkUntagged { bookmark_id: String, tag_id: String },
    BookmarkAddedToFolder { bookmark_id: String, folder_id: String },
    BookmarkRemovedFromFolder { bookmark_id: String, folder_id: String },
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
    pub tags: Option<Vec<String>>, // 增加标签列表
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventLog {
    pub event_id: String,
    pub device_id: String,
    pub timestamp: i64,
    pub event: SyncEvent,
}
