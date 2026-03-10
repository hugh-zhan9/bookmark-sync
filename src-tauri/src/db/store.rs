use crate::events::metadata::SiteMetadata;
use crate::events::models::{BookmarkPayload, EventLog};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_store_trait_should_exist() {
        fn _assert(_s: &dyn BookmarkStore) {}
    }
}
