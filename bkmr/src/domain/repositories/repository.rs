// src/domain/repositories/repository

use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainError;
use crate::domain::history::HistoryEntry;
use crate::domain::repositories::query::{BookmarkQuery, SortDirection};
use crate::domain::search::RankedResult;
use crate::domain::tag::Tag;
use std::collections::HashSet;
/*
   Repository Interface
   The BookmarkRepository interface follows the repository pattern to separate domain models from data access:

   Domain-Centric: Methods speak in domain terms, not persistence terms
   Abstraction: Hides data access details behind a clean interface
   Testability: Easy to create mock implementations for testing
   Flexibility: Allows switching persistence mechanisms without changing domain code
*/
/// Repository trait for bookmark persistence operations
pub trait BookmarkRepository: std::fmt::Debug + Send + Sync {
    /// Get a bookmark by its ID
    fn get_by_id(&self, id: i32) -> Result<Option<Bookmark>, DomainError>;

    /// Get a bookmark by its URL
    fn get_by_url(&self, url: &str) -> Result<Option<Bookmark>, DomainError>;

    /// Search for bookmarks using a query specification
    fn search(&self, query: &BookmarkQuery) -> Result<Vec<Bookmark>, DomainError>; // Helper method to get all bookmark IDs

    /// Get all bookmarks
    fn get_all(&self) -> Result<Vec<Bookmark>, DomainError>;

    /// Add a new bookmark
    fn add(&self, bookmark: &mut Bookmark) -> Result<(), DomainError>;

    /// Update an existing bookmark (stamps updated_at automatically)
    fn update(&self, bookmark: &Bookmark) -> Result<(), DomainError>;

    /// Atomically count one access and stamp accessed_at (does NOT change updated_at)
    fn record_access(&self, id: i32) -> Result<(), DomainError>;

    /// Stamp verified_at, the only way to set it (does NOT change updated_at). A change of content
    /// or title clears it, by a database trigger.
    fn mark_verified(&self, id: i32) -> Result<(), DomainError>;

    /// Delete a bookmark by ID. A memory keeps its old state in history.
    fn delete(&self, id: i32) -> Result<bool, DomainError>;

    /// Delete a bookmark and all history stored under its ID. True if anything was removed. Their
    /// text stays in the database files until `scrub`.
    fn purge(&self, id: i32) -> Result<bool, DomainError>;

    /// Remove the traces of deleted text from the database files. Safe to run again after a failure.
    fn scrub(&self) -> Result<(), DomainError>;

    /// Add a bookmark again under its old ID, which it must carry. Only for a bookmark that was
    /// deleted, because history refers to it by that ID.
    fn reinsert(&self, bookmark: &Bookmark) -> Result<(), DomainError>;

    /// History entries, newest first, of one bookmark ID or of all bookmarks
    fn get_history(
        &self,
        bookmark_id: Option<i32>,
        limit: i64,
    ) -> Result<Vec<HistoryEntry>, DomainError>;

    /// One history entry by its own ID
    fn get_history_entry(&self, history_id: i32) -> Result<Option<HistoryEntry>, DomainError>;

    /// Get all unique tags with their frequency
    fn get_all_tags(&self) -> Result<Vec<(Tag, usize)>, DomainError>;

    /// Get tags related to a specific tag (co-occurring)
    fn get_related_tags(&self, tag: &Tag) -> Result<Vec<(Tag, usize)>, DomainError>;

    /// Get random bookmarks
    fn get_random(&self, count: usize) -> Result<Vec<Bookmark>, DomainError>;

    /// Get bookmarks ordered by access date
    fn get_by_access_date(
        &self,
        direction: SortDirection,
        limit: Option<usize>,
    ) -> Result<Vec<Bookmark>, DomainError>;

    /// The ID of the bookmark with this URL, if there is one
    fn exists_by_url(&self, url: &str) -> Result<Option<i32>, DomainError>;

    /// Get bookmarks that are marked as embeddable but don't have embeddings yet
    fn get_embeddable_without_embeddings(&self) -> Result<Vec<Bookmark>, DomainError>;

    /// FTS search returning results with rank positions preserved for RRF fusion.
    /// If `filter_ids` is Some, only returns results whose IDs are in the set.
    fn get_bookmarks_fts_ranked(
        &self,
        fts_query: &str,
        filter_ids: Option<&HashSet<i32>>,
    ) -> Result<Vec<RankedResult>, DomainError>;

    /// Reset all content hashes to NULL (clean slate for re-embedding)
    fn clear_all_content_hashes(&self) -> Result<(), DomainError>;
}
