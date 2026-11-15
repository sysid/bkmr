// src/domain/history.rs
use crate::domain::per_type::PerType;
use chrono::{DateTime, Utc};

/// How many saved states history holds, for `bkmr info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryCounts {
    /// By the type of each saved state, which after a change of type differs between states of
    /// one bookmark
    pub per_type: PerType<usize>,
    /// Bookmarks that are deleted but have saved states
    pub deleted_bookmarks: usize,
    /// Saved states of those bookmarks
    pub deleted_states: usize,
}

/// What happened to the bookmark when its old state was saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryChange {
    Update,
    Delete,
}

impl HistoryChange {
    /// As stored by the database triggers that write history
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

/// The state of a bookmark just before it was changed or deleted. Embeddings are not kept: a
/// restored bookmark is embedded again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub history_id: i32,
    /// Bookmark IDs are never given out twice, so this always means the same bookmark
    pub bookmark_id: i32,
    pub change: HistoryChange,
    pub url: String,
    pub title: String,
    pub description: String,
    /// In the stored format, e.g. `,_mem_,fact,`
    pub tags: String,
    pub changed_at: DateTime<Utc>,
    pub embeddable: bool,
    pub file_path: Option<String>,
    pub file_mtime: Option<i32>,
    pub file_hash: Option<String>,
    pub opener: Option<String>,
}
