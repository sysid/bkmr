// src/domain/repositories/repository

use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainError;
use crate::domain::repositories::query::{BookmarkQuery, SortDirection};
use crate::domain::tag::Tag;
use crate::infrastructure::repositories::sqlite::connection::PooledConnection;
use crate::infrastructure::repositories::sqlite::error::{SqliteRepositoryError, SqliteResult};
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

    /// Update an existing bookmark
    fn update(&self, bookmark: &Bookmark) -> Result<(), DomainError>;

    /// Delete a bookmark by ID
    fn delete(&self, id: i32) -> Result<bool, DomainError>;

    /// Get all unique tags with their frequency
    fn get_all_tags(&self) -> Result<Vec<(Tag, usize)>, DomainError>;

    /// Get tags related to a specific tag (co-occurring)
    fn get_related_tags(&self, tag: &Tag) -> Result<Vec<(Tag, usize)>, DomainError>;

    /// Get random bookmarks
    fn get_random(&self, count: usize) -> Result<Vec<Bookmark>, DomainError>;

    /// Get bookmarks without embeddings
    fn get_without_embeddings(&self) -> Result<Vec<Bookmark>, DomainError>;

    /// Get bookmarks filtered by tags (all tags must match)
    fn get_by_all_tags(&self, tags: &HashSet<Tag>) -> Result<Vec<Bookmark>, DomainError>;

    /// Get bookmarks filtered by tags (any tag may match)
    fn get_by_any_tag(&self, tags: &HashSet<Tag>) -> Result<Vec<Bookmark>, DomainError>;

    /// Get bookmarks ordered by access date
    fn get_by_access_date(
        &self,
        direction: SortDirection,
        limit: Option<usize>,
    ) -> Result<Vec<Bookmark>, DomainError>;

    /// Search bookmarks by text
    fn search_by_text(&self, text: &str) -> Result<Vec<Bookmark>, DomainError>;

    fn get_bookmarks(&self, query: &str) -> SqliteResult<Vec<Bookmark>>;

    fn get_bookmarks_fts(&self, fts_query: &str) -> SqliteResult<Vec<i32>>;

    /// Check if bookmark exists by URL
    fn exists_by_url(&self, url: &str) -> Result<i32, DomainError>;

    /// Get bookmarks that are marked as embeddable but don't have embeddings yet
    fn get_embeddable_without_embeddings(&self) -> Result<Vec<Bookmark>, DomainError>;
    fn get_bookmarks_by_ids(&self, ids: &[i32]) -> Result<Vec<Bookmark>, DomainError>;
    fn get_all_bookmark_ids(
        &self,
        conn: &mut PooledConnection,
    ) -> Result<Vec<i32>, SqliteRepositoryError>;
}
