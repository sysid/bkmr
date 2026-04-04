// bkmr/src/domain/repositories/vector_repository.rs
use crate::domain::error::DomainResult;
use std::collections::HashSet;

/// Repository trait for vector embedding storage and similarity search.
/// Backed by sqlite-vec virtual table in the infrastructure layer.
pub trait VectorRepository: std::fmt::Debug + Send + Sync {
    /// Create the vec_bookmarks virtual table if it does not exist.
    fn init_vec_table(&self, dimensions: usize) -> DomainResult<()>;

    /// Insert or replace an embedding for a bookmark.
    fn upsert_embedding(&self, bookmark_id: i32, embedding: &[f32]) -> DomainResult<()>;

    /// Delete the embedding for a bookmark.
    fn delete_embedding(&self, bookmark_id: i32) -> DomainResult<()>;

    /// Find the nearest neighbors to a query embedding.
    /// Returns (bookmark_id, distance) pairs ordered by distance ascending.
    fn search_nearest(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> DomainResult<Vec<(i32, f64)>>;

    /// Check whether any embeddings exist in the virtual table.
    fn has_embeddings(&self) -> DomainResult<bool>;

    /// Detect the dimension of existing embeddings, if any.
    /// Returns None if the table is empty.
    fn get_dimensions(&self) -> DomainResult<Option<usize>>;

    /// Delete all rows from the virtual table.
    fn clear_all(&self) -> DomainResult<()>;

    /// Get the set of bookmark IDs that have embeddings.
    fn get_embedded_ids(&self) -> DomainResult<std::collections::HashSet<i32>>;

    /// Find nearest neighbors, constrained to a set of allowed IDs.
    /// Over-fetches internally and post-filters to ensure enough results within the ID set.
    /// If `filter_ids` is None, behaves identically to `search_nearest`.
    fn search_nearest_filtered(
        &self,
        query_embedding: &[f32],
        limit: usize,
        filter_ids: Option<&HashSet<i32>>,
    ) -> DomainResult<Vec<(i32, f64)>>;
}
