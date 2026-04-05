// src/infrastructure/repositories/null_vector_repository.rs
//! No-op VectorRepository for use in tests where embeddings are not exercised.

use crate::domain::error::DomainResult;
use crate::domain::repositories::vector_repository::VectorRepository;
use std::collections::HashSet;

/// A VectorRepository that does nothing and stores nothing.
/// Used with DummyEmbedding in tests where semantic search is not under test.
#[derive(Debug)]
pub struct NullVectorRepository;

impl VectorRepository for NullVectorRepository {
    fn init_vec_table(&self, _dimensions: usize) -> DomainResult<()> {
        Ok(())
    }

    fn upsert_embedding(&self, _bookmark_id: i32, _embedding: &[f32]) -> DomainResult<()> {
        Ok(())
    }

    fn delete_embedding(&self, _bookmark_id: i32) -> DomainResult<()> {
        Ok(())
    }

    fn search_nearest(
        &self,
        _query_embedding: &[f32],
        _limit: usize,
    ) -> DomainResult<Vec<(i32, f64)>> {
        Ok(Vec::new())
    }

    fn has_embeddings(&self) -> DomainResult<bool> {
        Ok(false)
    }

    fn get_dimensions(&self) -> DomainResult<Option<usize>> {
        Ok(None)
    }

    fn clear_all(&self) -> DomainResult<()> {
        Ok(())
    }

    fn get_embedded_ids(&self) -> DomainResult<HashSet<i32>> {
        Ok(HashSet::new())
    }

    fn search_nearest_filtered(
        &self,
        _query_embedding: &[f32],
        _limit: usize,
        _filter_ids: Option<&HashSet<i32>>,
    ) -> DomainResult<Vec<(i32, f64)>> {
        Ok(Vec::new())
    }
}
