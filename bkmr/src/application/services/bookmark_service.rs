// src/application/services/bookmark_service.rs
use crate::application::error::ApplicationResult;
use crate::domain::bookmark::Bookmark;
use crate::domain::repositories::query::{BookmarkQuery, SortDirection};
use crate::domain::search::{SemanticSearch, SemanticSearchResult};
use crate::domain::tag::Tag;
use std::collections::HashSet;
use std::fmt::Debug;

/// Service interface for bookmark-related operations
pub trait BookmarkService: Send + Sync + Debug {
    /// Add a new bookmark
    fn add_bookmark(
        &self,
        url: &str,
        title: Option<&str>,
        description: Option<&str>,
        tags: Option<&HashSet<Tag>>,
        fetch_metadata: bool,
    ) -> ApplicationResult<Bookmark>;

    /// Delete a bookmark by ID
    fn delete_bookmark(&self, id: i32) -> ApplicationResult<bool>;

    /// Get a bookmark by ID
    fn get_bookmark(&self, id: i32) -> ApplicationResult<Option<Bookmark>>;

    fn set_bookmark_embeddable(&self, id: i32, embeddable: bool) -> ApplicationResult<Bookmark>;

    /// Update a bookmark's title and description
    fn update_bookmark(
        &self,
        bookmark: Bookmark,
        force_embedding: bool,
    ) -> ApplicationResult<Bookmark>;

    /// Add tags to a bookmark
    fn add_tags_to_bookmark(&self, id: i32, tags: &HashSet<Tag>) -> ApplicationResult<Bookmark>;

    /// Remove tags from a bookmark
    fn remove_tags_from_bookmark(
        &self,
        id: i32,
        tags: &HashSet<Tag>,
    ) -> ApplicationResult<Bookmark>;

    /// Replace all tags on a bookmark
    fn replace_bookmark_tags(&self, id: i32, tags: &HashSet<Tag>) -> ApplicationResult<Bookmark>;

    fn search_bookmarks_by_text(&self, query: &str) -> ApplicationResult<Vec<Bookmark>>;
    // Add a convenience method to create a query for text search

    // Replace the complex search_bookmarks method with a simpler interface
    fn search_bookmarks(&self, query: &BookmarkQuery) -> ApplicationResult<Vec<Bookmark>>;

    /// Perform semantic search with the given parameters
    fn semantic_search(
        &self,
        search: &SemanticSearch,
    ) -> ApplicationResult<Vec<SemanticSearchResult>>;

    /// Get bookmark by URL
    fn get_bookmark_by_url(&self, url: &str) -> ApplicationResult<Option<Bookmark>>;

    /// Get all bookmarks
    fn get_all_bookmarks(
        &self,
        sort_direction: Option<SortDirection>,
        limit: Option<usize>,
    ) -> ApplicationResult<Vec<Bookmark>>;

    /// Get random bookmarks
    fn get_random_bookmarks(&self, count: usize) -> ApplicationResult<Vec<Bookmark>>;

    /// Get bookmarks for forced backfill (all embeddable bookmarks except those with _imported_ tag)
    fn get_bookmarks_for_forced_backfill(&self) -> ApplicationResult<Vec<Bookmark>>;

    /// Check if bookmarks need embedding backfilling
    fn get_bookmarks_without_embeddings(&self) -> ApplicationResult<Vec<Bookmark>>;

    /// Record that a bookmark was accessed
    fn record_bookmark_access(&self, id: i32) -> ApplicationResult<Bookmark>;

    /// Import bookmarks from a JSON file
    fn load_json_bookmarks(&self, path: &str, dry_run: bool) -> ApplicationResult<usize>;

    /// Load texts from NDJSON file and create embeddings for semantic search
    fn load_texts(&self, path: &str, dry_run: bool, force: bool) -> ApplicationResult<usize>;

    /// Import files from directories, parsing frontmatter metadata
    fn import_files(
        &self,
        paths: &[String],
        update: bool,
        delete_missing: bool,
        dry_run: bool,
        verbose: bool,
        base_path_name: Option<&str>,
    ) -> ApplicationResult<(usize, usize, usize)>; // Returns (added, updated, deleted)
}
