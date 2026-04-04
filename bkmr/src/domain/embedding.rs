// bkmr/src/domain/embedding.rs
use crate::domain::error::DomainResult;
use std::fmt::Debug;

/// Core trait for text embedding functionality.
///
/// Implementations must distinguish between document and query embeddings
/// because prefix-aware models (e.g., Nomic) use different prefixes for
/// each direction to improve retrieval quality.
pub trait Embedder: Send + Sync + Debug {
    /// Embed text intended for storage (document side).
    /// Implementations prepend the appropriate document prefix (e.g., "search_document: ").
    fn embed_document(&self, text: &str) -> DomainResult<Option<Vec<f32>>>;

    /// Embed text intended for search (query side).
    /// Implementations prepend the appropriate query prefix (e.g., "search_query: ").
    fn embed_query(&self, text: &str) -> DomainResult<Option<Vec<f32>>>;

    /// The number of dimensions this embedder produces.
    /// Used for dimension mismatch detection (FR-012).
    fn dimensions(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::embeddings::DummyEmbedding;

    #[test]
    fn given_dummy_embedder_when_embed_document_then_returns_none() {
        let embedder = DummyEmbedding;
        let result = embedder.embed_document("test text").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn given_dummy_embedder_when_embed_query_then_returns_none() {
        let embedder = DummyEmbedding;
        let result = embedder.embed_query("test query").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn given_dummy_embedder_when_dimensions_then_returns_zero() {
        let embedder = DummyEmbedding;
        assert_eq!(embedder.dimensions(), 0);
    }
}
